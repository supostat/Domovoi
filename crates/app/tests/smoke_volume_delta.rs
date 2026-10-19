use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use domovoi_app::{EventEffect, EventQueue, EventRequest, PendingDelta, Session, SessionError, SessionPaths};
use domovoi_index::{DirectoryListing, Reclaimable};
use domovoi_scanner::{CancelToken, ScanOptions, VolumeEvent};
use domovoi_testkit::{ImageVolume, TempTree, Visibility, VolumeFormat, test_timeout};

const MEGABYTE: usize = 1024 * 1024;
const QUIET: Duration = Duration::from_millis(600);
const POLL: Duration = Duration::from_millis(50);
const NEW_DIRECTORIES: u64 = 2;

type CardsView = BTreeMap<&'static str, (BTreeMap<PathBuf, Reclaimable>, BTreeSet<PathBuf>, Reclaimable)>;

fn write(path: &Path, content: &[u8]) {
    fs::create_dir_all(path.parent().expect("a fixture file has a folder")).expect("the fixture folder is created");
    fs::write(path, content).expect("the fixture file is written");
}

fn populate(root: &Path) {
    write(&root.join("Projects/engine/src/main.rs"), b"fn main() {}\n");
    write(
        &root.join("Projects/engine/target/debug/engine"),
        &vec![7; 2 * MEGABYTE],
    );
    write(&root.join("Projects/site/package.json"), b"{}\n");
    write(
        &root.join("Projects/site/node_modules/left-pad/index.js"),
        &vec![3; MEGABYTE],
    );
    write(&root.join("Media/old/clip.mov"), &vec![5; MEGABYTE]);
    write(&root.join("Notes/todo.txt"), b"todo\n");
}

fn read_in_another_thread(pending: &PendingDelta) -> Vec<DirectoryListing> {
    let pending = pending.clone();
    thread::spawn(move || pending.read(&ScanOptions::default()))
        .join()
        .expect("the reading thread does not panic")
        .expect("the targets read")
}

fn next_batch(events: &Receiver<VolumeEvent>, deadline: Instant) -> Vec<VolumeEvent> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    let first = match events.recv_timeout(remaining) {
        Ok(event) => event,
        Err(RecvTimeoutError::Timeout) => return Vec::new(),
        Err(RecvTimeoutError::Disconnected) => panic!("the volume event stream stopped"),
    };
    let mut batch = vec![first];
    while let Ok(event) = events.recv_timeout(QUIET) {
        batch.push(event);
    }
    batch
}

fn apply_until(session: &mut Session, events: &Receiver<VolumeEvent>, settled: impl Fn(&Session) -> bool) -> u64 {
    let deadline = Instant::now() + test_timeout();
    let mut descended = 0;
    while !settled(session) {
        assert!(Instant::now() < deadline, "the changes did not reach the index in time");
        let batch = next_batch(events, deadline);
        if batch.is_empty() {
            continue;
        }
        let EventRequest::Delta(pending) = session.delta_request(&batch) else {
            panic!("the volume asked for a full rescan: {batch:?}");
        };
        let listings = read_in_another_thread(&pending);
        descended += listings.iter().map(|listing| listing.descended).sum::<u64>();
        session.apply_listings(&pending, listings).expect("the delta applies");
    }
    descended
}

fn batch_touching(events: &Receiver<VolumeEvent>, folder: &Path) -> Vec<VolumeEvent> {
    let deadline = Instant::now() + test_timeout();
    let mut batch = Vec::new();
    while !batch
        .iter()
        .any(|event| matches!(event, VolumeEvent::Changed { path, .. } if path.starts_with(folder)))
    {
        assert!(
            Instant::now() < deadline,
            "no event reached {} in time",
            folder.display()
        );
        batch.extend(next_batch(events, deadline));
    }
    batch
}

fn trash(session: &mut Session, path: &Path) {
    let node = session
        .index()
        .expect("the volume is scanned")
        .find(path)
        .expect("the folder is indexed");
    session.add_to_tray(node, None);
    let outcomes = session.move_tray_to_trash().expect("the tray moves to the Trash");
    assert!(outcomes.iter().all(|outcome| outcome.result.is_ok()), "{outcomes:?}");
}

fn indexed(session: &Session, path: &Path) -> bool {
    session.index().expect("the volume is scanned").find(path).is_some()
}

fn cards(session: &Session) -> (CardsView, u64) {
    let index = session.index().expect("the volume is scanned");
    let recommendations = session.recommendations().expect("a scan builds recommendations");
    let view = recommendations
        .cards
        .iter()
        .map(|card| {
            let items = card
                .items
                .iter()
                .map(|item| (index.path(item.node), item.reclaimable))
                .collect();
            let blocked = card.blocked.iter().map(|(node, _)| index.path(*node)).collect();
            (session.rules().rule(card.rule).id, (items, blocked, card.reclaimable))
        })
        .collect();
    (view, recommendations.totals.total())
}

fn assert_cards_match_a_full_build(session: &mut Session) {
    let incremental = cards(session);
    session.refresh_recommendations();
    assert_eq!(incremental, cards(session));
}

#[test]
fn volume_events_reach_the_index_through_background_reads() {
    let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Browsable).expect("the image attaches");
    let root = fs::canonicalize(volume.mount_point()).expect("the mount point resolves");
    populate(&root);
    let state = TempTree::new().expect("a temp tree is created");
    let mut session =
        Session::open(&root, &SessionPaths::in_directory(state.path()), "0.0.0-test").expect("the session opens");
    session.scan(None, CancelToken::default()).expect("the volume scans");
    assert!(!cards(&session).0.contains_key("cargo.target"));
    let (_stream, events) = session.watch().expect("the volume is watched");

    write(&root.join("fresh.txt"), &vec![1; MEGABYTE / 4]);
    write(&root.join("Projects/site/assets/icons/a.png"), &vec![2; MEGABYTE / 8]);
    write(&root.join("Projects/site/assets/icons/b.png"), &vec![4; MEGABYTE / 8]);
    write(
        &root.join("Projects/engine/Cargo.toml"),
        b"[package]\nname = \"engine\"\n",
    );
    fs::remove_dir_all(root.join("Media/old")).expect("a folder is removed");
    let descended = apply_until(&mut session, &events, |session| {
        indexed(session, &root.join("fresh.txt"))
            && indexed(session, &root.join("Projects/site/assets/icons/b.png"))
            && indexed(session, &root.join("Projects/engine/Cargo.toml"))
            && !indexed(session, &root.join("Media/old"))
    });
    assert_eq!(descended, NEW_DIRECTORIES, "only the new folders are read in depth");
    assert!(cards(&session).0.contains_key("cargo.target"));
    assert_cards_match_a_full_build(&mut session);

    let modules = root.join("Projects/site/node_modules");
    write(&root.join("Projects/site/README.md"), b"site\n");
    let batch = batch_touching(&events, &root.join("Projects/site"));
    let EventRequest::Delta(pending) = session.delta_request(&batch) else {
        panic!("the volume asked for a full rescan: {batch:?}");
    };
    let listings = read_in_another_thread(&pending);
    trash(&mut session, &modules);
    assert!(matches!(
        session.apply_listings(&pending, listings),
        Err(SessionError::Stale(_))
    ));
    assert!(!indexed(&session, &modules));
    assert_eq!(
        session.apply_events(&batch).expect("the queued events apply again"),
        EventEffect::Applied
    );
    assert!(indexed(&session, &root.join("Projects/site/README.md")));
    assert!(!indexed(&session, &modules));
    assert!(!cards(&session).0.contains_key("dev.deps"));
    assert_cards_match_a_full_build(&mut session);
}

#[test]
fn the_event_queue_reads_in_the_background_and_retries_a_stale_delta() {
    let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Browsable).expect("the image attaches");
    let root = fs::canonicalize(volume.mount_point()).expect("the mount point resolves");
    populate(&root);
    let state = TempTree::new().expect("a temp tree is created");
    let mut session =
        Session::open(&root, &SessionPaths::in_directory(state.path()), "0.0.0-test").expect("the session opens");
    session.scan(None, CancelToken::default()).expect("the volume scans");
    let (_stream, events) = session.watch().expect("the volume is watched");
    let mut queue = EventQueue::default();

    let modules = root.join("Projects/site/node_modules");
    write(&root.join("Projects/site/README.md"), b"site\n");
    let batch = batch_touching(&events, &root.join("Projects/site"));
    assert_eq!(
        queue.advance(batch, &mut session).expect("the queue requests a delta"),
        EventEffect::Ignored
    );
    assert!(queue.is_reading(), "the read runs while the main thread goes on");
    trash(&mut session, &modules);
    let deadline = Instant::now() + test_timeout();
    while !queue.is_idle() {
        assert!(Instant::now() < deadline, "the queue did not drain in time");
        queue
            .advance(events.try_iter(), &mut session)
            .expect("the queue advances");
        thread::sleep(POLL);
    }
    assert!(indexed(&session, &root.join("Projects/site/README.md")));
    assert!(!indexed(&session, &modules));
    assert!(!cards(&session).0.contains_key("dev.deps"));
    assert_cards_match_a_full_build(&mut session);
}
