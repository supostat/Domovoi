use std::ffi::{CString, OsStr};
use std::io;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::time::SystemTime;

use domovoi_apfs_sys::{clone_at, ftruncate, mkdir_at, open_at, pwrite_at, set_last_used_xattr, set_xattr};

use crate::fill::Fill;
use crate::system_tool::run;
use crate::temp_tree::TempTree;

const WRITE_CHUNK: usize = 1 << 20;
const MINIMUM_COMPRESSED_SIZE: u64 = 64 * 1024;
const FILE_MODE: libc::mode_t = 0o644;
const DIRECTORY_MODE: libc::mode_t = 0o755;
const LONGEST_NAME_CHARACTER: char = '漢';
const LONGEST_NAME_UNITS: usize = 255;
const FINDER_INFO: &str = "com.apple.FinderInfo";
const FINDER_INFO_LENGTH: usize = 32;
const EXTENDED_FINDER_FLAGS_OFFSET: usize = 24;
const OBJECT_IS_BUSY: u16 = 0x0080;
const LOOP_PARTNER_SUFFIX: &str = ".partner";
const PNPM_STORE: &str = "pnpm-store";
const PNPM_PACKAGES: [(&str, &str, &[u8]); 3] = [
    ("left-pad@1.3.0", "index.js", b"module.exports = leftPad;\n"),
    ("is-odd@3.0.1", "index.js", b"module.exports = isOdd;\n"),
    ("chalk@5.3.0", "source/index.js", b"export default chalk;\n"),
];

#[derive(Debug)]
pub struct Directory<'tree> {
    tree: &'tree TempTree,
    descriptor: OwnedFd,
    path: PathBuf,
}

impl<'tree> Directory<'tree> {
    pub(crate) fn new(tree: &'tree TempTree, descriptor: OwnedFd, path: PathBuf) -> Self {
        Self { tree, descriptor, path }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn directory(&self, name: impl AsRef<OsStr>) -> io::Result<Directory<'tree>> {
        let name = name.as_ref();
        mkdir_at(self.descriptor.as_fd(), name, DIRECTORY_MODE)?;
        self.open_directory(name)
    }

    pub fn file(&self, name: impl AsRef<OsStr>, size: u64, content: Fill) -> io::Result<()> {
        let created = self.create(name.as_ref())?;
        let mut offset = 0;
        while offset < size {
            let length = usize::try_from((size - offset).min(WRITE_CHUNK as u64)).expect("chunk fits in usize");
            pwrite_at(created.as_fd(), &content.chunk(offset, length), offset)?;
            offset += length as u64;
        }
        Ok(())
    }

    pub fn hardlink(&self, existing: impl AsRef<OsStr>, name: impl AsRef<OsStr>) -> io::Result<()> {
        let existing = c_name(existing.as_ref())?;
        let name = c_name(name.as_ref())?;
        let directory = self.descriptor.as_raw_fd();
        check(unsafe { libc::linkat(directory, existing.as_ptr(), directory, name.as_ptr(), 0) })
    }

    pub fn clone_of(&self, existing: impl AsRef<OsStr>, name: impl AsRef<OsStr>) -> io::Result<()> {
        clone_at(
            self.descriptor.as_fd(),
            existing.as_ref(),
            self.descriptor.as_fd(),
            name.as_ref(),
        )
    }

    pub fn sparse(&self, name: impl AsRef<OsStr>, size: u64) -> io::Result<()> {
        let file = self.create(name.as_ref())?;
        ftruncate(file.as_fd(), size)
    }

    pub fn xattr(&self, name: impl AsRef<OsStr>, key: impl AsRef<OsStr>, value: &[u8]) -> io::Result<()> {
        let file = self.open_entry(name.as_ref())?;
        set_xattr(file.as_fd(), key.as_ref(), value)
    }

    pub fn compressed(&self, name: impl AsRef<OsStr>, size: u64) -> io::Result<()> {
        if size < MINIMUM_COMPRESSED_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("decmpfs compression needs at least {MINIMUM_COMPRESSED_SIZE} bytes"),
            ));
        }
        let name = name.as_ref();
        let mut source_name = name.to_os_string();
        source_name.push(".uncompressed");
        self.file(&source_name, size, Fill::Byte(b'a'))?;
        let source = self.path.join(&source_name);
        run(
            "/usr/bin/ditto",
            [
                source.as_os_str(),
                OsStr::new("--hfsCompression"),
                self.path.join(name).as_os_str(),
            ],
        )?;
        std::fs::remove_file(source)
    }

    pub fn mode(&self, name: impl AsRef<OsStr>, mode: libc::mode_t) -> io::Result<()> {
        let name = name.as_ref();
        let c_name = c_name(name)?;
        check(unsafe { libc::fchmodat(self.descriptor.as_raw_fd(), c_name.as_ptr(), mode, 0) })?;
        self.tree.remember_restricted(self.path.join(name));
        Ok(())
    }

    pub fn symlink_loop(&self, name: impl AsRef<OsStr>) -> io::Result<()> {
        let name = name.as_ref();
        let mut partner = name.to_os_string();
        partner.push(LOOP_PARTNER_SUFFIX);
        self.symlink(name, &partner)?;
        self.symlink(&partner, name)
    }

    pub fn deep(&self, levels: usize) -> io::Result<Directory<'tree>> {
        let name: String = std::iter::repeat_n(LONGEST_NAME_CHARACTER, LONGEST_NAME_UNITS).collect();
        let mut current = self.directory(&name)?;
        for _ in 1..levels {
            current = current.directory(&name)?;
        }
        Ok(current)
    }

    pub fn busy_bit(&self, name: impl AsRef<OsStr>) -> io::Result<()> {
        let mut finder_info = [0_u8; FINDER_INFO_LENGTH];
        finder_info[EXTENDED_FINDER_FLAGS_OFFSET..EXTENDED_FINDER_FLAGS_OFFSET + 2]
            .copy_from_slice(&OBJECT_IS_BUSY.to_be_bytes());
        self.xattr(name, FINDER_INFO, &finder_info)
    }

    pub fn last_used(&self, name: impl AsRef<OsStr>, time: SystemTime) -> io::Result<()> {
        let file = self.open_entry(name.as_ref())?;
        set_last_used_xattr(file.as_fd(), time)
    }

    pub fn pnpm_store(&self, projects: usize) -> io::Result<()> {
        let store = self.directory(PNPM_STORE)?;
        for (index, (_, _, content)) in PNPM_PACKAGES.iter().enumerate() {
            store.write(&format!("content-{index}"), content)?;
        }
        for project_index in 0..projects {
            let project = self.directory(format!("project-{project_index}"))?;
            let virtual_store = project.directory("node_modules")?.directory(".pnpm")?;
            for (index, (package, file, _)) in PNPM_PACKAGES.iter().enumerate() {
                let package_name = package.split('@').next().expect("package has a name");
                let mut target = virtual_store
                    .directory(package)?
                    .directory("node_modules")?
                    .directory(package_name)?;
                let mut segments = file.split('/').peekable();
                let mut file_name = "";
                while let Some(segment) = segments.next() {
                    if segments.peek().is_some() {
                        target = target.directory(segment)?;
                    } else {
                        file_name = segment;
                    }
                }
                clone_at(
                    store.descriptor.as_fd(),
                    OsStr::new(&format!("content-{index}")),
                    target.descriptor.as_fd(),
                    OsStr::new(file_name),
                )?;
            }
        }
        Ok(())
    }

    fn write(&self, name: &str, content: &[u8]) -> io::Result<()> {
        let file = self.create(OsStr::new(name))?;
        pwrite_at(file.as_fd(), content, 0)
    }

    fn symlink(&self, name: &OsStr, target: &OsStr) -> io::Result<()> {
        let name = c_name(name)?;
        let target = c_name(target)?;
        check(unsafe { libc::symlinkat(target.as_ptr(), self.descriptor.as_raw_fd(), name.as_ptr()) })
    }

    fn create(&self, name: &OsStr) -> io::Result<OwnedFd> {
        open_at(
            self.descriptor.as_fd(),
            name,
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW,
            FILE_MODE,
        )
    }

    fn open_entry(&self, name: &OsStr) -> io::Result<OwnedFd> {
        open_at(self.descriptor.as_fd(), name, libc::O_RDONLY | libc::O_NOFOLLOW, 0)
    }

    fn open_directory(&self, name: &OsStr) -> io::Result<Directory<'tree>> {
        let descriptor = open_at(
            self.descriptor.as_fd(),
            name,
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW,
            0,
        )?;
        Ok(Directory::new(self.tree, descriptor, self.path.join(name)))
    }
}

fn c_name(name: &OsStr) -> io::Result<CString> {
    CString::new(name.as_bytes()).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "name contains NUL"))
}

fn check(result: libc::c_int) -> io::Result<()> {
    if result == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
