use std::fs;

use domovoi_app::{Session, SessionPaths};
use domovoi_dups::CloneJournal;
use domovoi_scanner::CancelToken;
use domovoi_testkit::TempTree;

const APP_VERSION: &str = "0.0.0-test";
const BROKEN_JOURNALS: [&[u8]; 2] = [b"a journal cut short by a crash", b""];

#[test]
fn a_broken_clone_journal_is_set_aside_and_the_disk_opens() {
    for broken in BROKEN_JOURNALS {
        let disk = TempTree::new().expect("a disk folder is created");
        fs::write(disk.path().join("report.raw"), vec![7_u8; 4096]).expect("a file is written");
        let state = TempTree::new().expect("a state folder is created");
        let paths = SessionPaths::in_directory(state.path());
        fs::write(&paths.clone_journal, broken).expect("the broken journal is written");
        let shown = String::from_utf8_lossy(broken);

        let mut session = Session::open(disk.path(), &paths, APP_VERSION)
            .unwrap_or_else(|error| panic!("the journal {shown:?} does not keep the disk closed: {error}"));

        session
            .scan(None, CancelToken::default())
            .expect("the disk scans after the broken journal");
        assert_eq!(
            fs::read(state.path().join("clone-journal.broken")).expect("the broken journal is set aside"),
            broken,
            "the journal {shown:?} is kept as it was"
        );
        let journal = CloneJournal::open(&paths.clone_journal).expect("the journal reads again");
        assert!(journal.pending().is_empty(), "{journal:?}");
    }
}
