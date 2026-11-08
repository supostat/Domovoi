use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use domovoi_app::{Provenance, RecommendationsModel, Session, SessionPaths, TrayModel};
use domovoi_scanner::{CancelToken, VolumeEvent};
use domovoi_testkit::{Fill, TempTree};

const PROJECT: &str = "condoit";
const EVENT_ID: u64 = 1;

struct Workspace {
    tree: TempTree,
    _state: TempTree,
    session: Session,
}

impl Workspace {
    fn scanned() -> Self {
        let tree = TempTree::new().expect("a temp tree is created");
        let root = tree.root().expect("the root opens");
        let project = root.directory(PROJECT).expect("a project is created");
        project
            .file("Package.swift", 64, Fill::Byte(1))
            .expect("a manifest is written");
        project
            .directory(".build")
            .expect("a build folder is created")
            .file("build.o", 200_000, Fill::Noise(2))
            .expect("build output is written");
        root.directory("notes")
            .expect("a folder is created")
            .file("todo.txt", 64, Fill::Byte(3))
            .expect("a file is written");
        let state = TempTree::new().expect("a state folder is created");
        let mut session = Session::open(tree.path(), &SessionPaths::in_directory(state.path()), "0.0.0-test")
            .expect("the session opens");
        session.scan(None, CancelToken::default()).expect("the tree scans");
        Self {
            tree,
            _state: state,
            session,
        }
    }

    fn mark_the_project(&mut self) {
        let index = self.session.index().expect("the tree is indexed");
        let model = RecommendationsModel::build(
            index,
            self.session.rules(),
            self.session.recommendations().expect("a scan builds recommendations"),
            SystemTime::now(),
        );
        let card = model
            .cards
            .iter()
            .find(|card| card.provenance == Provenance::Projects)
            .expect("the build folder makes a projects card");
        let row = card
            .rows
            .iter()
            .find(|row| row.name == PROJECT)
            .expect("the project has a row");
        for target in &row.targets {
            self.session.add_target_to_tray(target);
        }
    }

    fn change_on_disk(&mut self) {
        let notes: PathBuf = self.tree.path().join("notes");
        fs::write(notes.join("fresh.txt"), b"a change next to the project").expect("a file appears");
        self.session
            .apply_events(&[VolumeEvent::Changed {
                path: notes,
                rescan_subtree: false,
                id: EVENT_ID,
            }])
            .expect("the change applies");
    }

    fn tray_titles(&self) -> Vec<String> {
        let index = self.session.index().expect("the tree is indexed");
        TrayModel::estimated(index, self.session.tray())
            .rows
            .into_iter()
            .map(|row| row.title)
            .collect()
    }
}

#[test]
fn a_marked_project_keeps_its_row_after_a_change_on_disk() {
    let mut workspace = Workspace::scanned();
    workspace.mark_the_project();
    assert_eq!(
        workspace.tray_titles(),
        [PROJECT],
        "the project goes to the tray as its own row"
    );
    workspace.change_on_disk();
    assert_eq!(
        workspace.tray_titles(),
        [PROJECT],
        "a change elsewhere on the volume keeps the project row in the tray"
    );
}
