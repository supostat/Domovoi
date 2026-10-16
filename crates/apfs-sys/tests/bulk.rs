mod temp_dir;

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use domovoi_apfs_sys::sys::{EF_NO_XATTRS, EF_SHARES_ALL_BLOCKS, UF_HIDDEN};
use domovoi_apfs_sys::{
    AttributeSet, BulkListing, EntryAttributes, ObjectKind, chflags, clone_at, copy_metadata, current_user_id,
    ftruncate, get_xattr, last_used_xattr, mkdir_at, mmap_readonly, open_at, pwrite_at, real_dev, rename_swap,
    set_last_used_xattr, set_nocache, set_xattr, stream_identity,
};
use temp_dir::TempDir;

const CONTENT_LENGTH: usize = 10_000;
const SPARSE_LENGTH: u64 = 10_000_000;

fn create_file(directory: BorrowedFd<'_>, name: &str, content: &[u8]) -> OwnedFd {
    let file = open_at(
        directory,
        OsStr::new(name),
        libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW,
        0o644,
    )
    .expect("file is created");
    pwrite_at(file.as_fd(), content, 0).expect("content is written");
    file
}

fn listing(directory: BorrowedFd<'_>, attributes: AttributeSet) -> BTreeMap<OsString, EntryAttributes> {
    BulkListing::new(directory, attributes)
        .map(|entry| {
            let entry = entry.expect("entry parses");
            (entry.name, entry.attributes.expect("entry has attributes"))
        })
        .collect()
}

#[test]
fn lists_names_kinds_sizes_and_identity_of_a_temp_tree() {
    let temp = TempDir::new("bulk");
    let root = temp.open();
    let content = vec![0x5A_u8; CONTENT_LENGTH];
    create_file(root.as_fd(), "file.txt", &content);
    mkdir_at(root.as_fd(), OsStr::new("subdir"), 0o755).expect("directory is created");
    std::os::unix::fs::symlink("file.txt", temp.path().join("link")).expect("symlink is created");
    let sparse = create_file(root.as_fd(), "sparse.bin", b"");
    ftruncate(sparse.as_fd(), SPARSE_LENGTH).expect("sparse file is extended");

    let entries = listing(root.as_fd(), AttributeSet::Extended);
    let names: Vec<_> = entries.keys().cloned().collect();
    assert_eq!(names, ["file.txt", "link", "sparse.bin", "subdir"].map(OsString::from));

    let file = &entries[OsStr::new("file.txt")];
    let metadata = fs::symlink_metadata(temp.path().join("file.txt")).expect("metadata is read");
    assert_eq!(file.kind, ObjectKind::File);
    assert_eq!(file.file_id, metadata.ino());
    assert_eq!(file.total_size, Some(CONTENT_LENGTH as u64));
    assert_eq!(file.alloc_size, Some(metadata.blocks() * 512));
    assert_eq!(file.modified, metadata.modified().expect("mtime is available"));
    assert_eq!(file.real_dev, Some(real_dev(temp.path()).expect("real dev is read")));
    assert!(file.added.is_some());
    assert_eq!(file.private_size, None);

    assert_eq!(entries[OsStr::new("subdir")].kind, ObjectKind::Directory);
    assert_eq!(entries[OsStr::new("subdir")].mount_status, Some(0));
    assert_eq!(entries[OsStr::new("link")].kind, ObjectKind::Symlink);
    assert_eq!(entries[OsStr::new("sparse.bin")].total_size, Some(SPARSE_LENGTH));
    assert_eq!(entries[OsStr::new("sparse.bin")].alloc_size, Some(0));
}

#[test]
fn reports_clones_and_hard_links_through_extended_attributes() {
    let temp = TempDir::new("identity");
    let root = temp.open();
    create_file(root.as_fd(), "original", &vec![1_u8; CONTENT_LENGTH]);
    clone_at(root.as_fd(), OsStr::new("original"), root.as_fd(), OsStr::new("clone")).expect("file is cloned");
    fs::hard_link(temp.path().join("original"), temp.path().join("hardlink")).expect("hard link is created");

    let entries = listing(root.as_fd(), AttributeSet::ExtendedWithPrivateSize);
    let original = &entries[OsStr::new("original")];
    let clone = &entries[OsStr::new("clone")];
    let hardlink = &entries[OsStr::new("hardlink")];

    assert_eq!(original.file_id, hardlink.file_id);
    assert_ne!(original.link_id, hardlink.link_id);
    assert_eq!(original.link_count, Some(2));
    assert_ne!(original.file_id, clone.file_id);
    assert_eq!(original.clone_id, clone.clone_id);
    assert_eq!(clone.clone_refcount, Some(2));
    assert!(
        clone
            .ext_flags
            .is_some_and(|flags| flags & u64::from(EF_SHARES_ALL_BLOCKS) != 0)
    );
    assert_eq!(clone.private_size, Some(0));

    let original_stream = stream_identity(&temp.path().join("original")).expect("stream identity is read");
    let clone_stream = stream_identity(&temp.path().join("clone")).expect("stream identity is read");
    assert_eq!(Some(original_stream.real_dev), original.real_dev);
    assert_eq!(Some(original_stream.clone_id), original.clone_id);
    assert_eq!(original_stream.clone_id, clone_stream.clone_id);
    assert_eq!(Some(clone_stream.ext_flags), clone.ext_flags);
}

#[test]
fn reads_extended_attributes_and_the_current_user() {
    let temp = TempDir::new("xattr");
    let root = temp.open();
    let file = create_file(root.as_fd(), "tagged", b"tagged");
    let key = OsStr::new("com.domovoi.test");
    assert_eq!(
        get_xattr(file.as_fd(), key).expect("missing attribute is not an error"),
        None
    );
    set_xattr(file.as_fd(), key, b"value").expect("attribute is written");
    assert_eq!(
        get_xattr(file.as_fd(), key).expect("attribute is read"),
        Some(b"value".to_vec())
    );
    let owner = fs::symlink_metadata(temp.path().join("tagged"))
        .expect("metadata")
        .uid();
    assert_eq!(current_user_id(), owner);
}

#[test]
fn keeps_a_name_of_255_utf16_units() {
    let temp = TempDir::new("long-name");
    let root = temp.open();
    let name = "Ж".repeat(255);
    mkdir_at(root.as_fd(), OsStr::new(&name), 0o755).expect("directory with a long name is created");
    let entries = listing(root.as_fd(), AttributeSet::Extended);
    assert_eq!(entries.keys().cloned().collect::<Vec<_>>(), vec![OsString::from(name)]);
}

#[test]
fn writes_metadata_that_the_listing_and_readers_see() {
    let temp = TempDir::new("metadata");
    let root = temp.open();
    let file = create_file(root.as_fd(), "marked", b"payload");
    let opened_at = UNIX_EPOCH + Duration::new(1_760_000_000, 123_456_789);
    set_last_used_xattr(file.as_fd(), opened_at).expect("last used date is written");
    set_xattr(file.as_fd(), OsStr::new("com.domovoi.test"), b"value").expect("xattr is written");
    chflags(file.as_fd(), UF_HIDDEN).expect("flags are set");
    set_nocache(file.as_fd()).expect("cache is disabled");

    assert_eq!(
        last_used_xattr(file.as_fd()).expect("last used date is read"),
        Some(opened_at)
    );
    let marked = &listing(root.as_fd(), AttributeSet::Extended)[OsStr::new("marked")];
    assert_ne!(marked.flags & UF_HIDDEN, 0);
    assert!(
        marked
            .ext_flags
            .is_some_and(|flags| flags & u64::from(EF_NO_XATTRS) == 0)
    );
    assert_eq!(
        &*mmap_readonly(&temp.path().join("marked")).expect("file is mapped"),
        b"payload"
    );

    let plain = create_file(root.as_fd(), "plain", b"");
    assert_eq!(
        last_used_xattr(plain.as_fd()).expect("missing date is not an error"),
        None
    );
    assert!(
        mmap_readonly(&temp.path().join("plain"))
            .expect("empty file is mapped")
            .is_empty()
    );
}

#[test]
fn copies_metadata_and_swaps_entries() {
    let temp = TempDir::new("swap");
    let root = temp.open();
    let source = create_file(root.as_fd(), "source", b"source");
    let target = create_file(root.as_fd(), "target", b"target");
    fs::set_permissions(temp.path().join("source"), fs::Permissions::from_mode(0o600)).expect("mode is set");
    let accessed = SystemTime::now();
    set_last_used_xattr(source.as_fd(), accessed).expect("last used date is written");
    copy_metadata(source.as_fd(), target.as_fd()).expect("metadata is copied");
    let target_metadata = fs::symlink_metadata(temp.path().join("target")).expect("metadata is read");
    assert_eq!(target_metadata.mode() & 0o777, 0o600);
    assert_eq!(
        last_used_xattr(target.as_fd()).expect("copied date is read"),
        Some(accessed)
    );

    let source_id = fs::symlink_metadata(temp.path().join("source"))
        .expect("metadata")
        .ino();
    let target_id = target_metadata.ino();
    rename_swap(root.as_fd(), OsStr::new("source"), root.as_fd(), OsStr::new("target")).expect("entries are swapped");
    assert_eq!(
        fs::symlink_metadata(temp.path().join("source"))
            .expect("metadata")
            .ino(),
        target_id
    );
    assert_eq!(
        fs::symlink_metadata(temp.path().join("target"))
            .expect("metadata")
            .ino(),
        source_id
    );
}
