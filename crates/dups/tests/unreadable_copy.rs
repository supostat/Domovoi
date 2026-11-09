use std::fs::{self, Permissions};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use domovoi_dups::{
    CloudFolders, DuplicateGroup, DuplicateOptions, HashingPool, SkipReason, SkippedFile, find_duplicates,
};
use domovoi_index::{Enrichment, Index, IndexRoot, NodeId};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};

const HDIUTIL: &str = "/usr/bin/hdiutil";
const HFS_IMAGE_SIZE: &str = "16m";
const SIZE: u64 = 2 * 1024 * 1024;
const KEPT: &str = "Documents/report.raw";
const READABLE_COPY: &str = "Backup/report.raw";
const UNREADABLE_COPY: &str = "Locked/report.raw";
const CLOSED: u32 = 0o000;
const OPEN: u32 = 0o755;

struct ClosedFolder {
    path: PathBuf,
}

impl ClosedFolder {
    fn close(path: PathBuf) -> Self {
        fs::set_permissions(&path, Permissions::from_mode(CLOSED)).expect("the folder is closed");
        Self { path }
    }
}

impl Drop for ClosedFolder {
    fn drop(&mut self) {
        fs::set_permissions(&self.path, Permissions::from_mode(OPEN)).expect("the folder is opened again");
    }
}

fn volume() -> TempTree {
    let tree = TempTree::new().expect("a temp tree is created");
    let root = tree.root().expect("the root opens");
    for relative in [KEPT, READABLE_COPY, UNREADABLE_COPY] {
        let path = Path::new(relative);
        root.directory(path.parent().expect("a file sits in a folder"))
            .expect("a folder is created")
            .file(path.file_name().expect("a file has a name"), SIZE, Fill::Noise(3))
            .expect("a file is written");
    }
    tree
}

fn index_of(root: &Path) -> Index {
    let report = scan(root, &ScanOptions::default()).expect("the tree scans");
    Index::build(
        &report,
        IndexRoot::for_path(root).expect("the root kind is known"),
        &Enrichment::default(),
    )
}

fn node_at(index: &Index, relative: &str) -> NodeId {
    index
        .find(&index.path(index.root()).join(relative))
        .unwrap_or_else(|| panic!("{relative} is indexed"))
}

#[test]
fn a_copy_whose_metadata_cannot_be_read_does_not_fail_its_group() {
    let tree = volume();
    let index = index_of(tree.path());
    let members = [KEPT, READABLE_COPY, UNREADABLE_COPY].map(|relative| node_at(&index, relative));
    let _closed = ClosedFolder::close(tree.path().join("Locked"));

    let mut skipped = Vec::new();
    let group = DuplicateGroup::of_members(&index, SIZE, &members, &mut skipped)
        .expect("one unreadable copy does not fail the group");

    assert_eq!(group.keep, node_at(&index, KEPT));
    let copies: Vec<(NodeId, bool)> = group
        .copies
        .iter()
        .map(|copy| (copy.node, copy.is_removable()))
        .collect();
    assert_eq!(
        copies,
        [(node_at(&index, READABLE_COPY), true)],
        "the readable copy stays, the unreadable one leaves the group"
    );
    assert_eq!(
        skipped,
        [SkippedFile {
            node: node_at(&index, UNREADABLE_COPY),
            reason: SkipReason::Unreadable(libc::EACCES),
        }]
    );
}

struct HfsVolume {
    mount_point: PathBuf,
    _workspace: TempTree,
}

impl HfsVolume {
    fn attach() -> Self {
        let workspace = TempTree::new().expect("a workspace is created");
        let image = workspace.path().join("volume.dmg");
        let mount_point = workspace.path().join("mount");
        fs::create_dir(&mount_point).expect("the mount point is created");
        let created = Command::new(HDIUTIL)
            .args([
                "create",
                "-quiet",
                "-size",
                HFS_IMAGE_SIZE,
                "-fs",
                "HFS+",
                "-volname",
                "Plain",
            ])
            .arg(&image)
            .status()
            .expect("hdiutil runs");
        assert!(created.success(), "an HFS+ image is created");
        let attached = Command::new(HDIUTIL)
            .args(["attach", "-quiet", "-nobrowse", "-mountpoint"])
            .arg(&mount_point)
            .arg(&image)
            .status()
            .expect("hdiutil runs");
        assert!(attached.success(), "the HFS+ image attaches");
        Self {
            mount_point,
            _workspace: workspace,
        }
    }
}

impl Drop for HfsVolume {
    fn drop(&mut self) {
        let detached = Command::new(HDIUTIL)
            .args(["detach", "-quiet", "-force"])
            .arg(&self.mount_point)
            .status()
            .expect("hdiutil runs");
        assert!(detached.success(), "the HFS+ image detaches");
    }
}

#[test]
fn copies_on_a_volume_without_private_size_are_found_with_their_allocated_size() {
    let volume = HfsVolume::attach();
    for folder in ["Documents", "Backup"] {
        fs::create_dir(volume.mount_point.join(folder)).expect("a folder is created");
        fs::write(
            volume.mount_point.join(folder).join("report.raw"),
            vec![7_u8; SIZE as usize],
        )
        .expect("a file is written");
    }
    let index = index_of(&volume.mount_point);
    let pool = HashingPool::new(None).expect("the hashing pool starts");

    let duplicates = find_duplicates(&index, &pool, &DuplicateOptions::new(CloudFolders::new(Vec::new())))
        .expect("a volume without PRIVATESIZE does not fail the search");

    let [group] = duplicates.groups.as_slice() else {
        panic!("one group is found: {:?}", duplicates.groups);
    };
    assert_eq!(
        group.reclaimable.bytes,
        index.node(node_at(&index, READABLE_COPY)).alloc_size,
        "a copy on a volume without clones frees its allocated size"
    );
}
