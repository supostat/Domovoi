use std::path::PathBuf;

use domovoi_app::{DuplicateKey, DuplicatesModel, PreviewTargets, Session, SessionPaths};
use domovoi_scanner::CancelToken;
use domovoi_testkit::{Fill, TempTree};

const SIZE: u64 = 2 * 1024 * 1024;
const KEPT: (&str, &str) = ("Documents", "report.pdf");
const COPY: (&str, &str) = ("Downloads", "report copy.pdf");

#[test]
fn a_member_in_the_home_folder_previews_its_file_on_disk() {
    let home = std::env::home_dir().expect("the user has a home folder");
    let tree = TempTree::under(&home).expect("a tree is created in the home folder");
    let root = tree.root().expect("the root opens");
    for (folder, name) in [KEPT, COPY] {
        root.directory(folder)
            .expect("a folder is created")
            .file(name, SIZE, Fill::Noise(5))
            .expect("a file is written");
    }
    let state = TempTree::new().expect("a state folder is created");
    let mut session =
        Session::open(tree.path(), &SessionPaths::in_directory(state.path()), "0.0.0-test").expect("the session opens");
    session.scan(None, CancelToken::default()).expect("the tree scans");
    session
        .find_duplicates(None, CancelToken::default())
        .expect("duplicates are searched");
    let index = session.index().expect("the index stays");
    let model = DuplicatesModel::found(index, session.duplicates().expect("the search stores duplicates"), 2, 1);
    assert_eq!(model.groups().len(), 1, "the two reports form a group");
    let both_members = [DuplicateKey::parse("0.0"), DuplicateKey::parse("0.1")];

    let targets = PreviewTargets::from_duplicates(model.groups(), index, &both_members);

    let mut found = targets.paths.clone();
    let mut expected: Vec<PathBuf> = [KEPT, COPY]
        .iter()
        .map(|(folder, name)| tree.path().join(folder).join(name))
        .collect();
    found.sort();
    expected.sort();
    assert_eq!(
        found, expected,
        "Quick Look gets the files on disk, not the shown paths"
    );
    assert!(found.iter().all(|path| path.is_file()));
}
