mod synthetic_tree;

use std::fs::{self, File};
use std::os::fd::AsFd;
use std::path::Path;
use std::process::{Child, Command};

use domovoi_apfs_sys::chflags;
use domovoi_apfs_sys::sys::UF_IMMUTABLE;
use domovoi_index::{Enrichment, Index, IndexRoot, NodeId};
use domovoi_rules::{CaseRule, DenyReason, Denylist};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};
use synthetic_tree::{data_volume_index, directory, file, merge, path_tree};

fn temp_index(tree: &TempTree) -> Index {
    let report = scan(tree.path(), &ScanOptions::default()).expect("temp tree scans");
    Index::build(
        &report,
        IndexRoot::for_path(tree.path()).expect("root kind"),
        &Enrichment::default(),
    )
}

fn at(index: &Index, relative: &str) -> NodeId {
    Path::new(relative).components().fold(index.root(), |node, name| {
        index
            .child_named(node, name.as_os_str())
            .unwrap_or_else(|| panic!("{relative} is indexed"))
    })
}

fn set_flags(path: &Path, flags: u32) {
    let file = File::open(path).expect("file opens");
    chflags(file.as_fd(), flags).expect("flags are set");
}

struct RunningChild(Child);

impl Drop for RunningChild {
    fn drop(&mut self) {
        if let Err(error) = self.0.kill().and_then(|()| self.0.wait().map(drop)) {
            eprintln!("bundle process was not stopped: {error}");
        }
    }
}

#[test]
fn immutable_items_and_their_folders_are_denied() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    let folder = root.directory("archive").expect("directory is created");
    folder.file("locked", 64, Fill::Byte(1)).expect("file is written");
    root.file("free", 64, Fill::Byte(2)).expect("file is written");
    let locked = tree.path().join("archive/locked");
    set_flags(&locked, UF_IMMUTABLE);
    let index = temp_index(&tree);
    let denylist = Denylist::new(CaseRule::Insensitive).expect("denylist is prepared");
    let verdicts = (
        denylist.verdict(&index, at(&index, "archive/locked")),
        denylist.verdict(&index, at(&index, "archive")),
        denylist.verdict(&index, at(&index, "free")),
    );
    set_flags(&locked, 0);
    assert_eq!(verdicts.0, Some(DenyReason::ProtectedFlag("UF_IMMUTABLE")));
    assert_eq!(verdicts.1, Some(DenyReason::ProtectedFlag("UF_IMMUTABLE")));
    assert_eq!(verdicts.2, None);
}

#[test]
fn bundles_of_running_processes_are_denied_until_they_exit() {
    let tree = TempTree::new().expect("temp tree is created");
    let executable_directory = tree.path().join("Fake.app/Contents/MacOS");
    fs::create_dir_all(&executable_directory).expect("bundle is created");
    let executable = executable_directory.join("Fake");
    let source = tree.path().join("fake.c");
    fs::write(
        &source,
        "#include <unistd.h>\nint main(void) { sleep(30); return 0; }\n",
    )
    .expect("source is written");
    let compiled = Command::new("/usr/bin/cc")
        .arg("-o")
        .arg(&executable)
        .arg(&source)
        .status()
        .expect("cc runs");
    assert!(compiled.success(), "the bundle executable compiles");
    let index = temp_index(&tree);
    let bundle = at(&index, "Fake.app");
    let mut denylist = Denylist::new(CaseRule::Insensitive).expect("denylist is prepared");
    let child = RunningChild(
        Command::new(&executable)
            .arg("30")
            .spawn()
            .expect("bundle process starts"),
    );
    denylist.refresh().expect("running bundles are listed");
    let canonical_bundle = fs::canonicalize(tree.path().join("Fake.app")).expect("bundle canonicalizes");
    assert_eq!(
        denylist.verdict(&index, bundle),
        Some(DenyReason::RunningApplication(canonical_bundle))
    );
    drop(child);
    denylist.refresh().expect("running bundles are listed");
    assert_eq!(denylist.verdict(&index, bundle), None);
}

#[test]
fn personal_and_system_data_is_denied_by_location() {
    let index = data_volume_index(vec![directory(
        "Users",
        vec![directory(
            "igor",
            merge(vec![
                path_tree("Library/Keychains", file("login.keychain-db")),
                path_tree("Library/Containers/com.apple.Notes", file("notes.db")),
                path_tree("Library/Group Containers/UBF8T346G9.com.apple.notes", file("store")),
                path_tree(
                    "Library/Containers/com.example.App/Data/Library/Preferences",
                    file("app.plist"),
                ),
                path_tree("Pictures/Photos Library.photoslibrary", file("database")),
                path_tree(".Trash", file("old")),
                path_tree("Library/Caches/com.example.viewer", file("blob")),
            ]),
        )],
    )]);
    let denylist = Denylist::new(CaseRule::Insensitive).expect("denylist is prepared");
    let reason = |relative: &str| denylist.verdict(&index, at(&index, relative));
    assert!(matches!(
        reason("Users/igor/Library/Keychains"),
        Some(DenyReason::ProtectedData(_))
    ));
    assert!(matches!(
        reason("Users/igor/Library/Containers/com.apple.Notes"),
        Some(DenyReason::ProtectedData(_))
    ));
    assert!(matches!(
        reason("Users/igor/Library/Group Containers/UBF8T346G9.com.apple.notes"),
        Some(DenyReason::ProtectedData(_))
    ));
    assert!(matches!(
        reason("Users/igor/Library/Containers/com.example.App/Data/Library/Preferences/app.plist"),
        Some(DenyReason::ProtectedData(_))
    ));
    assert!(matches!(
        reason("Users/igor/Pictures/Photos Library.photoslibrary"),
        Some(DenyReason::ProtectedData(_))
    ));
    assert_eq!(reason("Users/igor/.Trash/old"), Some(DenyReason::Trash));
    assert_eq!(reason("Users/igor/Library/Caches/com.example.viewer"), None);
}
