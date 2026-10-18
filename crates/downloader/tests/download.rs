use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use domovoi_apfs_sys::current_user_id;
use domovoi_downloader::{
    Catalog, CatalogEntry, DownloadHandle, DownloadOutcome, DownloadProgress, Downloader, InstallError, ModelSource,
    ModelStore, Sha256Hasher,
};
use domovoi_index::ModelEvent;
use domovoi_testkit::{
    CDN_ETAG, HfServer, ImageVolume, ServerBehaviour, TempTree, Visibility, VolumeFormat, test_timeout,
};

const THROTTLED_CHUNK: usize = 4096;
const THROTTLE_DELAY: Duration = Duration::from_millis(15);
const PAUSE_AFTER: u64 = 16 * 1024;
const UNTHROTTLED_CHUNK: usize = 64 * 1024;
const IMPOSSIBLE_SIZE: u64 = 1 << 60;

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn tiny_entry() -> CatalogEntry {
    Catalog::from_path(&assets().join("test/catalog.json"))
        .expect("the test catalog parses")
        .entry("test.tiny-random")
        .expect("the test catalog lists the tiny model")
        .clone()
}

fn honest_behaviour(entry: &CatalogEntry) -> ServerBehaviour {
    ServerBehaviour {
        content: fs::read(assets().join("test/tiny-random.gguf")).expect("the tiny model reads"),
        revision: entry.revision.clone(),
        linked_etag: entry.sha256.to_string(),
        expired_links: 0,
        ignore_ranges: false,
        chunk: UNTHROTTLED_CHUNK,
        chunk_delay: Duration::ZERO,
    }
}

fn throttled(behaviour: ServerBehaviour) -> ServerBehaviour {
    ServerBehaviour {
        chunk: THROTTLED_CHUNK,
        chunk_delay: THROTTLE_DELAY,
        ..behaviour
    }
}

fn store_in(tree: &TempTree) -> ModelStore {
    ModelStore::open(&tree.path().join("models")).expect("the model store opens")
}

fn start(server: &HfServer, store: &ModelStore, entry: &CatalogEntry) -> (DownloadHandle, Receiver<DownloadProgress>) {
    let (progress, updates) = mpsc::channel();
    let downloader = Downloader::new(ModelSource::mirror(server.base()), store.clone());
    let handle = downloader.start(entry, move |update| {
        let _ = progress.send(update);
    });
    (handle, updates)
}

fn wait_for_progress(updates: &Receiver<DownloadProgress>, at_least: u64) {
    let deadline = Instant::now() + test_timeout();
    while Instant::now() < deadline {
        if let Ok(update) = updates.recv_timeout(Duration::from_millis(20))
            && update.received >= at_least
        {
            return;
        }
    }
    panic!("the download never reached {at_least} bytes");
}

fn assert_installed(outcome: DownloadOutcome, store: &ModelStore, entry: &CatalogEntry) {
    match outcome {
        DownloadOutcome::Installed(ModelEvent::Downloaded { path }) => {
            assert_eq!(path, store.model_path(entry));
            assert_eq!(
                Sha256Hasher::digest_of_file(&path).expect("the model hashes"),
                entry.sha256
            );
            assert!(!store.partial_path(entry).exists(), "the partial file was renamed");
        }
        other => panic!("expected an installed model, got {other:?}"),
    }
}

fn paused_halfway(server: &HfServer, store: &ModelStore, entry: &CatalogEntry) -> u64 {
    let (handle, updates) = start(server, store, entry);
    wait_for_progress(&updates, PAUSE_AFTER);
    handle.pause();
    assert!(matches!(handle.wait(), DownloadOutcome::Paused));
    let kept = fs::metadata(store.partial_path(entry))
        .expect("the partial file stays")
        .len();
    assert!(kept >= PAUSE_AFTER && kept < entry.size, "{kept} bytes were kept");
    kept
}

#[test]
fn an_expired_link_is_refused_and_a_fresh_one_completes_the_download() {
    let entry = tiny_entry();
    let server = HfServer::start(ServerBehaviour {
        expired_links: 1,
        ..honest_behaviour(&entry)
    });
    let tree = TempTree::new().expect("a temp tree is created");
    let store = store_in(&tree);
    let (handle, _) = start(&server, &store, &entry);
    assert_installed(handle.wait(), &store, &entry);
    assert_eq!(server.resolves(), 2);
    let statuses: Vec<u16> = server.transfers().iter().map(|request| request.status).collect();
    assert_eq!(statuses, [403, 200]);
}

#[test]
fn a_paused_download_resumes_from_disk_with_range_and_if_range() {
    let entry = tiny_entry();
    let server = HfServer::start(throttled(honest_behaviour(&entry)));
    let tree = TempTree::new().expect("a temp tree is created");
    let store = store_in(&tree);
    let kept = paused_halfway(&server, &store, &entry);

    let (handle, _) = start(&server, &store, &entry);
    assert_installed(handle.wait(), &store, &entry);
    let resumed = server.transfers().last().cloned().expect("the transfer resumed");
    assert_eq!(resumed.range, Some(format!("bytes={kept}-")));
    assert_eq!(resumed.if_range.as_deref(), Some(CDN_ETAG));
    assert_eq!(resumed.status, 206);
}

#[test]
fn a_source_that_ignores_the_range_restarts_the_file_from_the_beginning() {
    let entry = tiny_entry();
    let server = HfServer::start(ServerBehaviour {
        ignore_ranges: true,
        ..throttled(honest_behaviour(&entry))
    });
    let tree = TempTree::new().expect("a temp tree is created");
    let store = store_in(&tree);
    paused_halfway(&server, &store, &entry);

    let (handle, _) = start(&server, &store, &entry);
    assert_installed(handle.wait(), &store, &entry);
    let resumed = server.transfers().last().cloned().expect("the transfer resumed");
    assert!(resumed.range.is_some());
    assert_eq!(resumed.status, 200);
}

#[test]
fn a_corrupted_transfer_is_refused_by_its_sha256() {
    let entry = tiny_entry();
    let mut behaviour = honest_behaviour(&entry);
    let middle = behaviour.content.len() / 2;
    behaviour.content[middle] ^= 0xFF;
    let server = HfServer::start(behaviour);
    let tree = TempTree::new().expect("a temp tree is created");
    let store = store_in(&tree);
    let (handle, _) = start(&server, &store, &entry);
    match handle.wait() {
        DownloadOutcome::Failed(InstallError::DigestMismatch { expected, received }) => {
            assert_eq!(expected, entry.sha256);
            assert_ne!(received, entry.sha256);
        }
        other => panic!("expected a digest mismatch, got {other:?}"),
    }
    assert!(!store.model_path(&entry).exists());
    assert_eq!(
        fs::metadata(store.partial_path(&entry))
            .expect("the partial file stays")
            .len(),
        0,
        "the corrupted bytes are not kept for a resume"
    );
}

#[test]
fn a_source_announcing_another_sha256_or_revision_is_refused_before_the_transfer() {
    let entry = tiny_entry();
    let tree = TempTree::new().expect("a temp tree is created");
    let store = store_in(&tree);

    let other_digest = HfServer::start(ServerBehaviour {
        linked_etag: "0".repeat(64),
        ..honest_behaviour(&entry)
    });
    let (handle, _) = start(&other_digest, &store, &entry);
    assert!(matches!(
        handle.wait(),
        DownloadOutcome::Failed(InstallError::DigestMismatch { .. })
    ));
    assert!(other_digest.transfers().is_empty());

    let other_revision = HfServer::start(ServerBehaviour {
        revision: "f".repeat(40),
        ..honest_behaviour(&entry)
    });
    let (handle, _) = start(&other_revision, &store, &entry);
    assert!(matches!(
        handle.wait(),
        DownloadOutcome::Failed(InstallError::RevisionMismatch { .. })
    ));
    assert!(other_revision.transfers().is_empty());
}

#[test]
fn missing_space_is_refused_before_any_request() {
    let entry = CatalogEntry {
        size: IMPOSSIBLE_SIZE,
        ..tiny_entry()
    };
    let server = HfServer::start(honest_behaviour(&entry));
    let tree = TempTree::new().expect("a temp tree is created");
    let store = store_in(&tree);
    let (handle, _) = start(&server, &store, &entry);
    match handle.wait() {
        DownloadOutcome::Failed(InstallError::InsufficientSpace { required, available }) => {
            assert!(required > available);
        }
        other => panic!("expected a refusal for space, got {other:?}"),
    }
    assert!(server.requests().is_empty());
    assert!(!store.partial_path(&entry).exists());
}

#[test]
fn a_cancelled_download_moves_its_partial_file_to_the_trash() {
    let entry = tiny_entry();
    let server = HfServer::start(throttled(honest_behaviour(&entry)));
    let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("the image attaches");
    let store = ModelStore::open(&volume.mount_point().join("models")).expect("the model store opens");
    let (handle, updates) = start(&server, &store, &entry);
    wait_for_progress(&updates, PAUSE_AFTER);
    handle.cancel();
    assert!(matches!(handle.wait(), DownloadOutcome::Cancelled));
    assert!(!store.partial_path(&entry).exists());
    let trash = volume
        .mount_point()
        .join(".Trashes")
        .join(current_user_id().to_string());
    let trashed: Vec<String> = fs::read_dir(&trash)
        .expect("the volume has a Trash for the user")
        .map(|item| {
            item.expect("a trashed item reads")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(
        trashed.iter().any(|name| name.starts_with("tiny-random.gguf.partial")),
        "{trashed:?}"
    );
}
