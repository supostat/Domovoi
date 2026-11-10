use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use domovoi_app::{Session, SessionPaths};
use domovoi_scanner::{CancelToken, VolumeEvent, current_event_id};
use domovoi_testkit::TempTree;

const APP_VERSION: &str = "0.0.0-test";
const FIRST_TARGET: &str = "/Users/igor/builds/target";
const MOVED_TARGET: &str = "/Users/igor/elsewhere/target";
const CARGO_CONFIG: &str = ".cargo/config.toml";

fn configure_cargo_target(home: &Path, target: &str) {
    let config = home.join(CARGO_CONFIG);
    fs::create_dir_all(config.parent().expect("a file has a folder")).expect("the folder is created");
    fs::write(&config, format!("[build]\ntarget-dir = \"{target}\"\n")).expect("the config is written");
}

fn cargo_target_paths(session: &Session) -> Vec<String> {
    session
        .rules()
        .rules()
        .iter()
        .find(|rule| rule.id == "cargo.target")
        .expect("the table has cargo.target")
        .paths()
        .to_vec()
}

fn cargo_target_card(session: &Session) -> BTreeSet<PathBuf> {
    let index = session.index().expect("the volume is scanned");
    session
        .recommendations()
        .expect("a scanned volume has recommendations")
        .cards
        .iter()
        .filter(|card| session.rules().rule(card.rule).id == "cargo.target")
        .flat_map(|card| card.items.iter().map(|item| index.path(item.node)))
        .collect()
}

fn build_folder(path: &Path) {
    fs::create_dir_all(path.join("debug")).expect("the build folder is created");
    fs::write(path.join("debug/app"), vec![7_u8; 4096]).expect("a build product is written");
}

struct ConfiguredVolume {
    _volume: TempTree,
    _state: TempTree,
    home: PathBuf,
    moved: PathBuf,
    session: Session,
}

impl ConfiguredVolume {
    fn scanned() -> Self {
        let volume = TempTree::new().expect("a temp volume is created");
        let state = TempTree::new().expect("a temp state is created");
        let root = fs::canonicalize(volume.path()).expect("the volume path resolves");
        let home = root.join("home");
        let first = root.join("builds/first/target");
        let moved = root.join("builds/moved/target");
        build_folder(&first);
        build_folder(&moved);
        configure_cargo_target(&home, &first.to_string_lossy());
        let paths = SessionPaths::in_directory(state.path()).with_home(&home);
        let mut session = Session::open(&root, &paths, APP_VERSION).expect("the session opens");
        session.scan(None, CancelToken::default()).expect("the volume scans");
        assert_eq!(cargo_target_card(&session), BTreeSet::from([first]));
        Self {
            _volume: volume,
            _state: state,
            home,
            moved,
            session,
        }
    }

    fn move_target_and_report(&mut self, changed_folder: PathBuf) {
        configure_cargo_target(&self.home, &self.moved.to_string_lossy());
        self.session
            .apply_events(&[VolumeEvent::Changed {
                path: changed_folder,
                rescan_subtree: false,
                id: current_event_id(),
            }])
            .expect("the change applies");
    }
}

#[test]
fn a_rescan_takes_the_cargo_target_changed_while_the_session_was_open() {
    let volume = TempTree::new().expect("a temp volume is created");
    let state = TempTree::new().expect("a temp state is created");
    let home = TempTree::new().expect("a temp home is created");
    configure_cargo_target(home.path(), FIRST_TARGET);
    let paths = SessionPaths::in_directory(state.path()).with_home(home.path());
    let mut session = Session::open(volume.path(), &paths, APP_VERSION).expect("the session opens");
    session.scan(None, CancelToken::default()).expect("the volume scans");

    configure_cargo_target(home.path(), MOVED_TARGET);
    session
        .scan(None, CancelToken::default())
        .expect("the volume scans again");

    let targets = cargo_target_paths(&session);
    assert!(
        targets.iter().any(|path| path == MOVED_TARGET) && !targets.iter().any(|path| path == FIRST_TARGET),
        "{targets:?} follows the target-dir changed without a restart"
    );
}

#[test]
fn an_edited_cargo_config_moves_the_target_card_without_a_rescan() {
    let mut volume = ConfiguredVolume::scanned();
    let cargo_folder = volume.home.join(".cargo");

    volume.move_target_and_report(cargo_folder);

    assert_eq!(
        cargo_target_card(&volume.session),
        BTreeSet::from([volume.moved.clone()]),
        "the card follows the edited config without a rescan"
    );
}

#[test]
fn a_new_folder_inside_a_source_folder_counts_as_its_change() {
    let mut volume = ConfiguredVolume::scanned();
    let fresh_folder = volume.home.join(".cargo/fresh");
    fs::create_dir(&fresh_folder).expect("a folder appears after the scan");

    volume.move_target_and_report(fresh_folder);

    assert_eq!(
        cargo_target_card(&volume.session),
        BTreeSet::from([volume.moved.clone()]),
        "a folder that appears inside a source folder, like a mounted runtime, rebuilds the rules"
    );
}
