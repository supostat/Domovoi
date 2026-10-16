use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, UNIX_EPOCH};

use domovoi_apfs_sys::sys::{EF_NO_XATTRS, UF_COMPRESSED};
use domovoi_apfs_sys::{AttributeSet, BulkListing, EntryAttributes, last_used_xattr, real_dev, volume_capabilities};
use domovoi_testkit::{Fill, ImageVolume, TempTree, Visibility, VolumeFormat};

const PATH_MAX: usize = 1024;
const SPARSE_SIZE: u64 = 50_000_000;
const COMPRESSED_SIZE: u64 = 128 * 1024;

fn listing(directory: &Path) -> BTreeMap<OsString, EntryAttributes> {
    let handle = OwnedFd::from(File::open(directory).expect("directory opens"));
    BulkListing::new(handle.as_fd(), AttributeSet::Extended)
        .map(|entry| {
            let entry = entry.expect("entry parses");
            (entry.name, entry.attributes.expect("entry has attributes"))
        })
        .collect()
}

fn xattr_hex(path: &Path, key: &str) -> String {
    let output = Command::new("/usr/bin/xattr")
        .args(["-px", key])
        .arg(path)
        .output()
        .expect("xattr runs");
    assert!(output.status.success(), "xattr -px {key} failed");
    String::from_utf8(output.stdout)
        .expect("xattr prints ASCII")
        .split_whitespace()
        .collect()
}

#[test]
fn builds_files_links_clones_sparse_and_compressed_data() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.file("noise.bin", 100_000, Fill::Noise(7))
        .expect("noise file is written");
    root.file("noise-twin.bin", 100_000, Fill::Noise(7))
        .expect("twin file is written");
    root.hardlink("noise.bin", "noise-link").expect("hard link is created");
    root.clone_of("noise.bin", "noise-clone").expect("clone is created");
    root.sparse("sparse.bin", SPARSE_SIZE).expect("sparse file is created");
    root.compressed("packed.txt", COMPRESSED_SIZE)
        .expect("compressed file is created");

    let path = |name: &str| tree.path().join(name);
    assert_eq!(
        fs::read(path("noise.bin")).expect("noise is read"),
        fs::read(path("noise-twin.bin")).expect("twin is read")
    );
    let original = fs::metadata(path("noise.bin")).expect("metadata");
    assert_eq!(original.len(), 100_000);
    assert_eq!(original.nlink(), 2);
    assert_eq!(
        fs::metadata(path("noise-link")).expect("metadata").ino(),
        original.ino()
    );
    let sparse = fs::metadata(path("sparse.bin")).expect("metadata");
    assert_eq!((sparse.len(), sparse.blocks()), (SPARSE_SIZE, 0));

    let entries = listing(tree.path());
    assert_eq!(
        entries[OsStr::new("noise.bin")].clone_id,
        entries[OsStr::new("noise-clone")].clone_id
    );
    let packed = &entries[OsStr::new("packed.txt")];
    assert_ne!(packed.flags & UF_COMPRESSED, 0);
    assert_eq!(packed.total_size, Some(COMPRESSED_SIZE));
    assert!(root.compressed("small.txt", 1024).is_err());
}

#[test]
fn builds_metadata_permissions_loops_and_deep_chains() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    let path = |name: &str| tree.path().join(name);
    root.file("marked", 16, Fill::Byte(1)).expect("file is written");
    root.xattr("marked", "com.domovoi.test", b"value")
        .expect("xattr is set");
    let opened_at = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    root.last_used("marked", opened_at).expect("last used date is set");
    root.file("busy", 16, Fill::Byte(2)).expect("file is written");
    root.busy_bit("busy").expect("busy bit is set");
    root.directory("locked").expect("directory is created");
    root.mode("locked", 0o000).expect("mode is set");
    root.symlink_loop("loop").expect("symlink loop is created");
    let deepest = root.deep(3).expect("deep chain is created");
    deepest
        .file("bottom", 8, Fill::Byte(3))
        .expect("file at the bottom is written");

    let marked = File::open(path("marked")).expect("marked file opens");
    assert_eq!(last_used_xattr(marked.as_fd()).expect("date is read"), Some(opened_at));
    assert_eq!(xattr_hex(&path("marked"), "com.domovoi.test"), "76616C7565");
    let finder_info = xattr_hex(&path("busy"), "com.apple.FinderInfo");
    assert_eq!(&finder_info[48..52], "0080");
    let entries = listing(tree.path());
    assert_eq!(
        entries[OsStr::new("marked")]
            .ext_flags
            .map(|flags| flags & u64::from(EF_NO_XATTRS)),
        Some(0)
    );
    assert_eq!(
        fs::symlink_metadata(path("locked"))
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777,
        0
    );
    let loop_error = File::open(path("loop")).expect_err("symlink loop does not resolve");
    assert_eq!(loop_error.raw_os_error(), Some(libc::ELOOP));
    assert!(deepest.path().as_os_str().len() > PATH_MAX);
    let too_long = fs::metadata(deepest.path()).expect_err("std paths stop at PATH_MAX");
    assert_eq!(too_long.raw_os_error(), Some(libc::ENAMETOOLONG));
}

#[test]
fn builds_a_pnpm_store_with_cloned_project_files() {
    let tree = TempTree::new().expect("temp tree is created");
    tree.root()
        .expect("root opens")
        .pnpm_store(2)
        .expect("pnpm store is created");
    let store = listing(&tree.path().join("pnpm-store"));
    for project in ["project-0", "project-1"] {
        let package = tree
            .path()
            .join(project)
            .join("node_modules/.pnpm/left-pad@1.3.0/node_modules/left-pad");
        let files = listing(&package);
        assert_eq!(
            files[OsStr::new("index.js")].clone_id,
            store[OsStr::new("content-0")].clone_id,
            "{project}"
        );
    }
    assert_eq!(store[OsStr::new("content-0")].clone_refcount, Some(3));
}

#[test]
fn attaches_an_image_volume_with_its_own_device_and_detaches_it() {
    let mut volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("image is attached");
    let mount_point = volume.mount_point().to_path_buf();
    assert_ne!(
        real_dev(&mount_point).expect("mount real dev"),
        real_dev(volume.workspace()).expect("workspace real dev")
    );
    assert!(!volume_capabilities(&mount_point).expect("capabilities").case_sensitive);
    fs::write(mount_point.join("owned"), b"data").expect("volume root is writable by its owner");
    volume.detach().expect("image detaches");
    assert_eq!(
        fs::metadata(&mount_point).expect("mount directory stays").dev(),
        fs::metadata(volume.workspace()).expect("workspace").dev()
    );
}
