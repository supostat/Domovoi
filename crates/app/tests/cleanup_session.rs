use std::fs;
use std::path::Path;

use domovoi_app::{Session, SessionPaths};
use domovoi_scanner::CancelToken;
use domovoi_testkit::TempTree;

const APP_VERSION: &str = "0.0.0-test";
const CONFIGURED_TARGET: &str = "/Users/igor/builds/target";

fn write(path: &Path, content: &[u8]) {
    fs::create_dir_all(path.parent().expect("a file has a folder")).expect("the folder is created");
    fs::write(path, content).expect("the file is written");
}

#[test]
fn the_session_takes_the_cargo_target_from_the_home_config() {
    let volume = TempTree::new().expect("a temp volume is created");
    let state = TempTree::new().expect("a temp state is created");
    let home = TempTree::new().expect("a temp home is created");
    write(
        &home.path().join(".cargo/config.toml"),
        format!("[build]\ntarget-dir = \"{CONFIGURED_TARGET}\"\n").as_bytes(),
    );
    let paths = SessionPaths::in_directory(state.path()).with_home(home.path());
    let session = Session::open(volume.path(), &paths, APP_VERSION).expect("the session opens");
    let cargo_target = session
        .rules()
        .rules()
        .iter()
        .find(|rule| rule.id == "cargo.target")
        .expect("the table has cargo.target");
    assert!(
        cargo_target.paths().iter().any(|path| path == CONFIGURED_TARGET),
        "{:?} names the configured target-dir",
        cargo_target.paths()
    );
}

#[test]
fn a_scan_asks_spotlight_for_screen_captures_and_finishes() {
    let volume = TempTree::new().expect("a temp volume is created");
    write(&volume.path().join("Desktop/note.txt"), b"note");
    let state = TempTree::new().expect("a temp state is created");
    let paths = SessionPaths::in_directory(state.path());
    let mut session = Session::open(volume.path(), &paths, APP_VERSION).expect("the session opens");
    session
        .scan(None, CancelToken::default())
        .expect("the volume scans with the real Spotlight query");
    assert!(session.index().is_some());
    assert!(session.recommendations().is_some());
}
