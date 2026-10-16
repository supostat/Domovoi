use std::cell::Cell;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::fd::AsFd;
use std::os::macos::fs::MetadataExt as _;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::time::SystemTime;

use block2::StackBlock;
use domovoi_apfs_sys::sys::{
    EF_IS_PURGEABLE, EF_IS_SYNC_ROOT, SF_APPEND, SF_DATALESS, SF_FIRMLINK, SF_IMMUTABLE, SF_NOUNLINK, SF_RESTRICTED,
    UF_APPEND, UF_DATAVAULT, UF_IMMUTABLE,
};
use domovoi_apfs_sys::{
    clone_at, container_capacity, copy_metadata, current_user_id, gen_count, get_xattr, link_id, pids_with_open,
    rename_swap, set_nocache, stream_identity, user_access, volume_capabilities,
};
use domovoi_index::{Index, NodeId};
use domovoi_rules::{Denylist, TrashJournal};
use objc2_app_kit::NSWorkspace;
use objc2_foundation::{NSFileCoordinator, NSFileCoordinatorWritingOptions, NSString, NSURL, NSUUID};

use crate::clone_journal::CloneJournal;
use crate::clone_refusal::CloneRefusal;
use crate::clone_replace_error::CloneReplaceError;
use crate::clone_replaced::CloneReplaced;
use crate::cloud_folders::CloudFolders;
use crate::content_digest::{open_without_cache, same_contents};
use crate::mount_point::mount_point_of;
use crate::pending_replacement::{PendingReplacement, TEMPORARY_PREFIX};

const PROTECTED_FLAGS: u32 = UF_IMMUTABLE
    | UF_APPEND
    | UF_DATAVAULT
    | SF_IMMUTABLE
    | SF_APPEND
    | SF_RESTRICTED
    | SF_NOUNLINK
    | SF_FIRMLINK
    | SF_DATALESS;
const WRITE_ACCESS: u32 = libc::W_OK.unsigned_abs();
const FINDER_INFO: &str = "com.apple.FinderInfo";
const EXTENDED_FINDER_FLAGS_OFFSET: usize = 24;
const OBJECT_IS_BUSY: u16 = 0x0080;
const LOW_SPACE_DIVISOR: u64 = 20;

#[derive(Debug)]
pub struct ClonePlan {
    pub low_space: bool,
    original: PathBuf,
    copy: PathBuf,
    copy_device: u64,
    copy_inode: u64,
    copy_size: u64,
    copy_modified: SystemTime,
    copy_generation: u32,
    directory_modified: SystemTime,
    pending: PendingReplacement,
}

impl ClonePlan {
    pub fn prepare(
        index: &Index,
        keep: NodeId,
        copy: NodeId,
        denylist: &Denylist,
        cloud_folders: &CloudFolders,
    ) -> Result<Self, CloneReplaceError> {
        let original = index.physical_path(keep);
        let copy_path = index.physical_path(copy);
        let directory = parent_of(&copy_path)?;
        let original_metadata = fs::symlink_metadata(&original)?;
        let copy_metadata = fs::symlink_metadata(&copy_path)?;
        for (path, metadata) in [(&original, &original_metadata), (&copy_path, &copy_metadata)] {
            if !metadata.file_type().is_file() {
                return Err(CloneRefusal::NotRegularFile(path.clone()).into());
            }
        }
        let original_stream = stream_identity(&original)?;
        let copy_stream = stream_identity(&copy_path)?;
        if original_stream.real_dev != copy_stream.real_dev {
            return Err(CloneRefusal::OtherVolume.into());
        }
        let volume = mount_point_of(&copy_path)?;
        let capabilities = volume_capabilities(&volume)?;
        if !capabilities.clone {
            return Err(CloneRefusal::VolumeCannotClone.into());
        }
        if !capabilities.rename_swap {
            return Err(CloneRefusal::VolumeCannotSwap.into());
        }
        if copy_metadata.nlink() != 1 {
            return Err(CloneRefusal::HardLinked.into());
        }
        if copy_metadata.uid() != current_user_id() {
            return Err(CloneRefusal::NotOwned.into());
        }
        for path in [&copy_path, &directory] {
            if user_access(path)? & WRITE_ACCESS == 0 {
                return Err(CloneRefusal::NotWritable(path.clone()).into());
            }
        }
        let protected = (copy_metadata.st_flags() & PROTECTED_FLAGS) | (original_metadata.st_flags() & SF_DATALESS);
        if protected != 0 {
            return Err(CloneRefusal::ProtectedFlags(protected).into());
        }
        if copy_stream.ext_flags & u64::from(EF_IS_PURGEABLE) != 0 {
            return Err(CloneRefusal::Purgeable.into());
        }
        if is_busy(&copy_path)? {
            return Err(CloneRefusal::Busy.into());
        }
        if let Some(reason) = denylist.verdict(index, copy) {
            return Err(CloneRefusal::Denied(reason).into());
        }
        if let Some(package) = enclosing_package(&directory)? {
            return Err(CloneRefusal::InsidePackage(package).into());
        }
        let canonical = index.path(copy);
        if cloud_folders.contains(&canonical) || inside_sync_root(index, copy) {
            return Err(CloneRefusal::SyncedFolder.into());
        }
        let holders = other_processes_holding(&copy_path)?;
        if !holders.is_empty() {
            return Err(CloneRefusal::OpenElsewhere(holders).into());
        }
        if original_stream.clone_id == copy_stream.clone_id {
            return Err(CloneRefusal::AlreadyClones.into());
        }
        let capacity = container_capacity(&volume)?;
        let temporary = directory.join(format!("{TEMPORARY_PREFIX}{}", NSUUID::UUID().UUIDString()));
        let pending = PendingReplacement {
            copy: canonical.to_string_lossy().into_owned(),
            copy_physical: copy_path.to_string_lossy().into_owned(),
            temporary: temporary.to_string_lossy().into_owned(),
            link_id: link_id(&copy_path)?,
            alloc_size: index.node(copy).alloc_size,
            flags: copy_metadata.st_flags(),
            clone_id: copy_stream.clone_id,
        };
        Ok(Self {
            low_space: capacity.available.saturating_mul(LOW_SPACE_DIVISOR) < capacity.size,
            original,
            copy_device: copy_metadata.dev(),
            copy_inode: copy_metadata.ino(),
            copy_size: copy_metadata.len(),
            copy_modified: copy_metadata.modified()?,
            copy_generation: gen_count(&copy_path)?,
            directory_modified: fs::metadata(&directory)?.modified()?,
            copy: copy_path,
            pending,
        })
    }

    pub fn execute(
        self,
        trash_journal: &mut TrashJournal,
        clone_journal: &mut CloneJournal,
    ) -> Result<CloneReplaced, CloneReplaceError> {
        let temporary = PathBuf::from(&self.pending.temporary);
        let directory = File::open(parent_of(&self.copy)?)?;
        let original_directory = File::open(parent_of(&self.original)?)?;
        let temporary_name = file_name_of(&temporary)?;
        clone_journal.begin(self.pending.clone())?;
        let replaced = clone_at(
            original_directory.as_fd(),
            file_name_of(&self.original)?,
            directory.as_fd(),
            temporary_name,
        )
        .map_err(CloneReplaceError::from)
        .and_then(|()| self.prepare_clone(&temporary))
        .and_then(|()| self.swap_coordinated(&directory, temporary_name));
        if let Err(error) = replaced {
            clone_journal.settle(&temporary, trash_journal)?;
            return Err(error);
        }
        let trashed = match trash_journal.trash(&self.pending.old_body())? {
            Ok(trashed) => trashed,
            Err(failure) => {
                return Err(CloneReplaceError::OldBodyNotTrashed {
                    location: temporary,
                    failure,
                });
            }
        };
        directory.set_modified(self.directory_modified)?;
        clone_journal.settle(&temporary, trash_journal)?;
        Ok(CloneReplaced { trashed })
    }

    fn prepare_clone(&self, temporary: &Path) -> Result<(), CloneReplaceError> {
        let copy_file = open_without_cache(&self.copy)?;
        let clone_file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(temporary)?;
        set_nocache(clone_file.as_fd())?;
        copy_metadata(copy_file.as_fd(), clone_file.as_fd())?;
        let opened = copy_file.metadata()?;
        let unchanged = opened.dev() == self.copy_device
            && opened.ino() == self.copy_inode
            && opened.len() == self.copy_size
            && opened.modified()? == self.copy_modified
            && self.copy_is_untouched()?
            && clone_file.metadata()?.len() == self.copy_size
            && same_contents(&copy_file, &clone_file, self.copy_size)?;
        if unchanged {
            Ok(())
        } else {
            Err(CloneRefusal::ContentChanged.into())
        }
    }

    fn swap_coordinated(&self, directory: &File, temporary_name: &OsStr) -> Result<(), CloneReplaceError> {
        let url = NSURL::fileURLWithPath(&NSString::from_str(path_text(&self.copy)?));
        let outcome = Cell::new(None);
        let accessor = StackBlock::new(|_: NonNull<NSURL>| {
            outcome.set(Some(self.swap_if_untouched(directory, temporary_name)));
        });
        let mut coordination_error = None;
        NSFileCoordinator::new().coordinateWritingItemAtURL_options_error_byAccessor(
            &url,
            NSFileCoordinatorWritingOptions::ForReplacing,
            Some(&mut coordination_error),
            &accessor,
        );
        if let Some(error) = coordination_error {
            return Err(io::Error::other(error.localizedDescription().to_string()).into());
        }
        outcome
            .into_inner()
            .expect("the coordinator runs the accessor when it reports no error")
    }

    fn swap_if_untouched(&self, directory: &File, temporary_name: &OsStr) -> Result<(), CloneReplaceError> {
        if !self.copy_is_untouched()? {
            return Err(CloneRefusal::ContentChanged.into());
        }
        rename_swap(
            directory.as_fd(),
            temporary_name,
            directory.as_fd(),
            file_name_of(&self.copy)?,
        )?;
        Ok(())
    }

    fn copy_is_untouched(&self) -> io::Result<bool> {
        Ok(
            fs::symlink_metadata(&self.copy)?.ino() == self.copy_inode
                && gen_count(&self.copy)? == self.copy_generation,
        )
    }
}

fn is_busy(path: &Path) -> io::Result<bool> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    Ok(
        get_xattr(file.as_fd(), OsStr::new(FINDER_INFO))?.is_some_and(|finder_info| {
            finder_info
                .get(EXTENDED_FINDER_FLAGS_OFFSET..EXTENDED_FINDER_FLAGS_OFFSET + 2)
                .is_some_and(|flags| u16::from_be_bytes([flags[0], flags[1]]) & OBJECT_IS_BUSY != 0)
        }),
    )
}

fn enclosing_package(directory: &Path) -> io::Result<Option<PathBuf>> {
    let workspace = NSWorkspace::sharedWorkspace();
    for ancestor in directory.ancestors() {
        if workspace.isFilePackageAtPath(&NSString::from_str(path_text(ancestor)?)) {
            return Ok(Some(ancestor.to_path_buf()));
        }
    }
    Ok(None)
}

fn inside_sync_root(index: &Index, node: NodeId) -> bool {
    let mut current = index.parent(node);
    while let Some(ancestor) = current {
        if index.node(ancestor).ext_flags & u64::from(EF_IS_SYNC_ROOT) != 0 {
            return true;
        }
        current = index.parent(ancestor);
    }
    false
}

fn other_processes_holding(path: &Path) -> io::Result<Vec<libc::pid_t>> {
    let own = libc::pid_t::try_from(std::process::id()).expect("a process id fits in pid_t");
    Ok(pids_with_open(path)?.into_iter().filter(|pid| *pid != own).collect())
}

fn parent_of(path: &Path) -> io::Result<PathBuf> {
    path.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "a file path has no directory"))
}

fn file_name_of(path: &Path) -> io::Result<&OsStr> {
    path.file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "a file path has no name"))
}

fn path_text(path: &Path) -> io::Result<&str> {
    path.to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path is not UTF-8"))
}
