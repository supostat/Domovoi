use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, Instant};

use domovoi_app::{ApplyJob, EventEffect, EventQueue, Session, SessionPaths};
use domovoi_scanner::{CancelToken, VolumeEvent};
use domovoi_testkit::{ImageVolume, TempTree, Visibility, VolumeFormat, test_timeout};

const HEAVY_NODES: usize = 2;
const WAITING_FILES: usize = 8;
const LATER_FILES: usize = 4;
const HELD_TICKS: usize = 6;
const POLL: Duration = Duration::from_millis(100);
const FILE_BYTES: usize = 8_192;

fn write(path: &Path) {
    fs::create_dir_all(path.parent().expect("a fixture file has a folder")).expect("the fixture folder is created");
    fs::write(path, vec![4; FILE_BYTES]).expect("the fixture file is written");
}

fn indexed(session: &Session, path: &Path) -> bool {
    session.index().expect("the volume is scanned").find(path).is_some()
}

fn all_indexed(session: &Session, paths: &[PathBuf]) -> bool {
    paths.iter().all(|path| indexed(session, path))
}

fn apply_held(queue: &mut EventQueue, session: Session) -> Session {
    let batch = queue.take_heavy().expect("the heavy batch waits for its session");
    let (session, applied) = ApplyJob::start(session, batch).finish();
    queue.settle(applied).expect("the batch applies");
    session
}

fn advance(queue: &mut EventQueue, events: &Receiver<VolumeEvent>, session: &mut Session) -> EventEffect {
    let effect = queue.advance(events.try_iter(), session).expect("the queue advances");
    assert_ne!(
        effect,
        EventEffect::RescanRequired,
        "the volume asked for a full rescan"
    );
    effect
}

#[test]
fn a_heavy_batch_held_while_the_tray_is_open_loses_nothing() {
    let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Browsable).expect("the image attaches");
    let root = fs::canonicalize(volume.mount_point()).expect("the mount point resolves");
    write(&root.join("Notes/todo.txt"));
    let state = TempTree::new().expect("a temp tree is created");
    let mut session =
        Session::open(&root, &SessionPaths::in_directory(state.path()), "0.0.0-test").expect("the session opens");
    session.scan(None, CancelToken::default()).expect("the volume scans");
    let (_stream, events) = session.watch().expect("the volume is watched");
    let mut queue = EventQueue::new(HEAVY_NODES);
    let deadline = Instant::now() + test_timeout();

    for file in 0..WAITING_FILES {
        write(&root.join(format!("Waiting.partial/file-{file}")));
    }
    fs::rename(root.join("Waiting.partial"), root.join("Waiting")).expect("the folder appears at once");
    let waiting: Vec<PathBuf> = (0..WAITING_FILES)
        .map(|file| root.join(format!("Waiting/file-{file}")))
        .collect();
    while advance(&mut queue, &events, &mut session) != EventEffect::Heavy {
        assert!(Instant::now() < deadline, "no heavy batch arrived in time");
        thread::sleep(POLL);
    }

    let later: Vec<PathBuf> = (0..LATER_FILES)
        .map(|file| root.join(format!("Later/note-{file}")))
        .collect();
    for path in &later {
        write(path);
    }
    for _ in 0..HELD_TICKS {
        thread::sleep(POLL);
        assert_eq!(
            advance(&mut queue, &events, &mut session),
            EventEffect::Heavy,
            "the held batch keeps waiting while new events queue"
        );
        assert!(
            !waiting.iter().any(|path| indexed(&session, path)),
            "nothing of the held batch reaches the index while the tray is open"
        );
    }

    session = apply_held(&mut queue, session);
    assert!(all_indexed(&session, &waiting), "the held batch applies in full");
    while !all_indexed(&session, &later) {
        assert!(
            Instant::now() < deadline,
            "the events queued during the wait did not reach the index"
        );
        if advance(&mut queue, &events, &mut session) == EventEffect::Heavy {
            session = apply_held(&mut queue, session);
        }
        thread::sleep(POLL);
    }
    assert!(all_indexed(&session, &waiting), "the later batches keep the held one");
}
