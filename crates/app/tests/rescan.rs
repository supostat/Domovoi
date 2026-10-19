use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use domovoi_app::{CacheOpening, Session, SessionError, SessionPaths};
use domovoi_scanner::{CancelToken, ScanError, current_event_id};
use domovoi_testkit::{ImageVolume, TempTree, Visibility, VolumeFormat};

const APP_VERSION: &str = "0.0.0-test";
const SEEN: &str = "Projects/engine/src/main.rs";
const FRESH: &str = "Projects/engine/fresh.bin";
const FILE_BYTES: usize = 64 * 1024;
const EVENT_TIMEOUT: Duration = Duration::from_secs(10);
const EVENT_POLL: Duration = Duration::from_millis(10);

struct ScannedVolume {
    _image: ImageVolume,
    _state: TempTree,
    root: PathBuf,
    paths: SessionPaths,
    session: Session,
}

fn write(path: &Path) {
    fs::create_dir_all(path.parent().expect("a file has a folder")).expect("the folder is created");
    fs::write(path, vec![7_u8; FILE_BYTES]).expect("the file is written");
}

fn scanned_volume() -> ScannedVolume {
    let image = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("the image attaches");
    let root = fs::canonicalize(image.mount_point()).expect("the mount point resolves");
    write(&root.join(SEEN));
    let state = TempTree::new().expect("a temp tree is created");
    let paths = SessionPaths::in_directory(state.path());
    let mut session = Session::open(&root, &paths, APP_VERSION).expect("the session opens");
    session.scan(None, CancelToken::default()).expect("the volume scans");
    ScannedVolume {
        _image: image,
        _state: state,
        root,
        paths,
        session,
    }
}

fn stopped() -> CancelToken {
    let cancel = CancelToken::default();
    cancel.cancel();
    cancel
}

fn wait_for_event_after(event_id: u64) {
    let deadline = Instant::now() + EVENT_TIMEOUT;
    while current_event_id() <= event_id {
        assert!(Instant::now() < deadline, "the file system moves past event {event_id}");
        thread::sleep(EVENT_POLL);
    }
}

#[test]
fn a_stopped_rescan_keeps_the_previous_index() {
    let mut volume = scanned_volume();
    let nodes = volume.session.index().expect("the volume is scanned").len();
    let last_event_id = volume.session.last_event_id();
    write(&volume.root.join(FRESH));

    let outcome = volume.session.scan(None, stopped());

    assert!(
        matches!(outcome, Err(SessionError::Scan(ScanError::Cancelled))),
        "the stop cancels the rescan: {outcome:?}"
    );
    let index = volume.session.index().expect("the previous index stays installed");
    assert_eq!(index.len(), nodes, "no partial index replaces the previous one");
    assert!(
        index.find(&volume.root.join(SEEN)).is_some(),
        "the previous contents stay"
    );
    assert!(
        index.find(&volume.root.join(FRESH)).is_none(),
        "the stopped rescan adds nothing"
    );
    assert_eq!(
        volume.session.last_event_id(),
        last_event_id,
        "events resume from the previous point"
    );
}

#[test]
fn a_full_rescan_finds_what_no_event_delivered_and_rewrites_the_cache() {
    let mut volume = scanned_volume();
    let before = volume.session.last_event_id();
    let fresh = volume.root.join(FRESH);
    write(&fresh);
    wait_for_event_after(before);

    volume
        .session
        .scan(None, CancelToken::default())
        .expect("the rescan completes");

    assert!(
        volume
            .session
            .index()
            .expect("the rescan installs an index")
            .find(&fresh)
            .is_some(),
        "the rescan finds the new file"
    );
    assert!(volume.session.last_event_id() > before, "events resume from the rescan");
    let mut reopened = Session::open(&volume.root, &volume.paths, APP_VERSION).expect("the session reopens");
    assert_eq!(
        reopened.open_cached().expect("the cache reads"),
        CacheOpening::Opened,
        "the rescan rewrote a valid cache"
    );
    assert!(
        reopened
            .index()
            .expect("the cache holds an index")
            .find(&fresh)
            .is_some(),
        "the cache holds the rescanned contents"
    );
    assert_eq!(reopened.last_event_id(), volume.session.last_event_id());
}
