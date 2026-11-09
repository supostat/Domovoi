use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use domovoi_app::{CloneJob, DuplicatesModel, Session, SessionPaths};
use domovoi_dups::TEMPORARY_PREFIX;
use domovoi_index::NodeId;
use domovoi_scanner::CancelToken;
use domovoi_testkit::{Fill, TempTree, test_timeout};

const MEBIBYTE: u64 = 1024 * 1024;
const LARGE_COPY: u64 = 1024 * MEBIBYTE;
const SMALL_COPY: u64 = 2 * MEBIBYTE;
const COPIES_IN_CLONE_MODE: usize = 8;
const CALLER_BUDGET: Duration = Duration::from_millis(100);
const POLL: Duration = Duration::from_millis(5);

struct Searched {
    session: Session,
    tree: TempTree,
    state: TempTree,
}

impl Searched {
    fn first_pair(&self) -> (NodeId, NodeId) {
        let group = &self.session.duplicates().expect("the search stores duplicates").groups[0];
        (group.keep, group.copies[0].node)
    }
}

fn searched(copies: usize, size: u64) -> Searched {
    let tree = TempTree::new().expect("a temp tree is created");
    let root = tree.root().expect("the root opens");
    for folder in 0..=copies {
        root.directory(format!("folder-{folder}"))
            .expect("a folder is created")
            .file("movie.mov", size, Fill::Byte(7))
            .expect("a file is written");
    }
    let state = TempTree::new().expect("a state folder is created");
    let mut session =
        Session::open(tree.path(), &SessionPaths::in_directory(state.path()), "0.0.0-test").expect("the session opens");
    session.scan(None, CancelToken::default()).expect("the tree scans");
    session
        .find_duplicates(None, CancelToken::default())
        .expect("duplicates are searched");
    Searched { session, tree, state }
}

fn temporaries_in(folder: &Path) -> Vec<String> {
    fs::read_dir(folder)
        .expect("the folder lists")
        .map(|entry| {
            entry
                .expect("an entry reads")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.starts_with(TEMPORARY_PREFIX))
        .collect()
}

#[test]
fn a_clone_job_returns_at_once_reports_progress_and_stops_without_touching_the_copy() {
    let searched = searched(1, LARGE_COPY);
    let (keep, copy) = searched.first_pair();
    let copy_path = searched.session.index().expect("the index stays").path(copy);
    let copy_inode = fs::metadata(&copy_path).expect("the copy exists").ino();
    let Searched {
        session,
        tree: _tree,
        state: _state,
    } = searched;

    let started = Instant::now();
    let mut job = CloneJob::start(session, vec![(keep, copy)]);
    let held = started.elapsed();
    assert!(
        held < CALLER_BUDGET,
        "starting the replacement held the caller for {held:?}"
    );
    assert_eq!(job.total, LARGE_COPY, "the job knows how much it compares");

    let deadline = Instant::now() + test_timeout();
    while !(job.absorb_progress() && job.done > 0) {
        assert!(!job.is_finished(), "the replacement ended before it reported progress");
        assert!(
            Instant::now() < deadline,
            "the replacement reported no progress in time"
        );
        thread::sleep(POLL);
    }
    job.stop();
    let (_session, outcome) = job.finish();
    let outcome = outcome.expect("the index stays consistent");

    assert!(outcome.stopped, "{outcome:?}");
    assert_eq!(outcome.freed, 0, "a stopped replacement frees nothing");
    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    assert_eq!(
        fs::metadata(&copy_path).expect("the copy stays").ino(),
        copy_inode,
        "the copy keeps its own body"
    );
    let folder = copy_path.parent().expect("a copy sits in a folder");
    assert!(temporaries_in(folder).is_empty(), "{:?}", temporaries_in(folder));
}

#[test]
fn switching_to_clone_mode_does_not_hold_the_caller() {
    let searched = searched(COPIES_IN_CLONE_MODE, SMALL_COPY);
    let session = &searched.session;
    let index = session.index().expect("the index stays");
    let mut model = DuplicatesModel::found(index, session.duplicates().expect("the search stores duplicates"), 0, 0);
    assert_eq!(model.groups()[0].copies.len(), COPIES_IN_CLONE_MODE);

    let started = Instant::now();
    model.assess_clones(|keep, copy| session.clone_assessment(keep, copy));
    let held = started.elapsed();

    assert!(
        held < CALLER_BUDGET,
        "assessing {COPIES_IN_CLONE_MODE} copies for clones held the caller for {held:?}"
    );
}
