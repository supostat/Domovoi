use std::fs::{self, File};
use std::path::Path;
use std::time::{Duration, SystemTime};

use domovoi_app::{Session, SessionPaths};
use domovoi_scanner::{CancelToken, VolumeEvent};
use domovoi_testkit::{Fill, TempTree};

const SIZE: u64 = 2 * 1024 * 1024;
const EDITED: &str = "Downloads/report copy.pdf";
const EVENT_ID: u64 = 1;
const LATER: Duration = Duration::from_secs(60);

struct Workspace {
    tree: TempTree,
    _state: TempTree,
    session: Session,
}

impl Workspace {
    fn searched() -> Self {
        let tree = TempTree::new().expect("a temp tree is created");
        let root = tree.root().expect("the root opens");
        for (folder, name) in [("Documents", "report.pdf"), ("Downloads", "report copy.pdf")] {
            root.directory(folder)
                .expect("a folder is created")
                .file(name, SIZE, Fill::Noise(5))
                .expect("a file is written");
        }
        root.directory("notes")
            .expect("a folder is created")
            .file("todo.txt", 64, Fill::Byte(1))
            .expect("a file is written");
        let state = TempTree::new().expect("a state folder is created");
        let mut session = Session::open(tree.path(), &SessionPaths::in_directory(state.path()), "0.0.0-test")
            .expect("the session opens");
        session.scan(None, CancelToken::default()).expect("the tree scans");
        session
            .find_duplicates(None, CancelToken::default())
            .expect("duplicates are searched");
        assert_eq!(groups(&session), 1, "the two reports form a group");
        Self {
            tree,
            _state: state,
            session,
        }
    }

    fn change(&mut self, relative: &str, contents: &[u8]) {
        let path = self.tree.path().join(relative);
        fs::write(&path, contents).expect("the file is written");
        File::options()
            .write(true)
            .open(&path)
            .and_then(|file| file.set_modified(SystemTime::now() + LATER))
            .expect("the change happens a minute after the search");
        let folder = path.parent().map(Path::to_path_buf).expect("a file sits in a folder");
        self.session
            .apply_events(&[VolumeEvent::Changed {
                path: folder,
                rescan_subtree: false,
                id: EVENT_ID,
            }])
            .expect("the change applies");
    }
}

fn groups(session: &Session) -> usize {
    session.duplicates().expect("a search was run").groups.len()
}

#[test]
fn a_member_edited_after_the_search_leaves_its_group() {
    let mut workspace = Workspace::searched();
    let edited = vec![9_u8; usize::try_from(SIZE).expect("the size fits")];

    workspace.change(EDITED, &edited);

    assert_eq!(
        groups(&workspace.session),
        0,
        "an edited file is no longer known to equal the other"
    );
}

#[test]
fn a_change_elsewhere_keeps_the_group() {
    let mut workspace = Workspace::searched();

    workspace.change("notes/fresh.txt", b"a change next to the duplicates");

    assert_eq!(groups(&workspace.session), 1);
}
