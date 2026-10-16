use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use domovoi_apfs_sys::current_user_id;
use domovoi_downloader::{
    Catalog, CatalogEntry, InstallError, InstalledModel, ModelStore, Orphan, OrphanReason, PartialDownload,
    Sha256Hasher, is_excluded_from_backup,
};
use domovoi_index::ModelEvent;
use domovoi_testkit::{ImageVolume, TempTree, Visibility, VolumeFormat};

const PARTIAL_BYTES: usize = 1000;

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn tiny_entry() -> CatalogEntry {
    Catalog::from_path(&assets().join("test/catalog.json"))
        .expect("the test catalog parses")
        .entries[0]
        .clone()
}

fn tiny_content() -> Vec<u8> {
    fs::read(assets().join("test/tiny-random.gguf")).expect("the tiny model reads")
}

fn damaged(content: &[u8]) -> Vec<u8> {
    let mut damaged = content.to_vec();
    let middle = damaged.len() / 2;
    damaged[middle] ^= 0xFF;
    damaged
}

#[test]
fn the_models_folder_is_excluded_from_time_machine_when_it_is_created() {
    let tree = TempTree::new().expect("a temp tree is created");
    let directory = tree.path().join("Application Support/Domovoi/models");
    let store = ModelStore::open(&directory).expect("the model store opens");
    assert!(is_excluded_from_backup(store.directory()).expect("the exclusion reads"));
    assert!(!is_excluded_from_backup(tree.path()).expect("the exclusion reads"));
    assert!(
        ModelStore::default_directory()
            .expect("the user has Application Support")
            .ends_with("Library/Application Support/Domovoi/models")
    );
}

#[test]
fn orphans_are_flagged_while_valid_models_and_partials_are_not() {
    let tree = TempTree::new().expect("a temp tree is created");
    let store = ModelStore::open(&tree.path().join("models")).expect("the model store opens");
    let tiny = tiny_entry();
    let second = CatalogEntry {
        id: "test.second".to_owned(),
        file: "second.gguf".to_owned(),
        ..tiny.clone()
    };
    let catalog = Catalog {
        entries: vec![tiny.clone(), second.clone()],
    };
    let content = tiny_content();
    fs::write(store.model_path(&tiny), &content).expect("the valid model is written");
    fs::write(store.model_path(&second), damaged(&content)).expect("the damaged model is written");
    fs::write(store.partial_path(&second), &content[..PARTIAL_BYTES]).expect("the partial is written");
    fs::write(store.directory().join("stray.gguf"), b"not from the catalog").expect("the stray file is written");
    fs::write(store.directory().join(".DS_Store"), b"finder").expect("the Finder file is written");

    let report = store.scan(&catalog).expect("the store scans");
    assert_eq!(
        report.installed,
        [InstalledModel {
            entry_id: tiny.id.clone(),
            path: store.model_path(&tiny),
        }]
    );
    assert_eq!(
        report.partials,
        [PartialDownload {
            entry_id: second.id.clone(),
            path: store.partial_path(&second),
            received: PARTIAL_BYTES as u64,
        }]
    );
    assert_eq!(
        report.orphans,
        [
            Orphan {
                path: store.model_path(&second),
                reason: OrphanReason::DigestMismatch,
            },
            Orphan {
                path: store.directory().join("stray.gguf"),
                reason: OrphanReason::NotInCatalog,
            },
        ]
    );
}

#[test]
fn a_file_placed_by_hand_is_accepted_only_by_its_sha256() {
    let tree = TempTree::new().expect("a temp tree is created");
    let store = ModelStore::open(&tree.path().join("models")).expect("the model store opens");
    let entry = tiny_entry();
    let content = tiny_content();

    let corrupted = tree.path().join("corrupted.gguf");
    fs::write(&corrupted, damaged(&content)).expect("the corrupted copy is written");
    assert!(matches!(
        store.adopt(&corrupted, &entry),
        Err(InstallError::DigestMismatch { .. })
    ));
    let truncated = tree.path().join("truncated.gguf");
    fs::write(&truncated, &content[..PARTIAL_BYTES]).expect("the truncated copy is written");
    assert!(matches!(
        store.adopt(&truncated, &entry),
        Err(InstallError::SizeMismatch { .. })
    ));
    assert!(!store.model_path(&entry).exists());

    let event = store
        .adopt(&assets().join("test/tiny-random.gguf"), &entry)
        .expect("the genuine file is accepted");
    assert_eq!(
        event,
        ModelEvent::Downloaded {
            path: store.model_path(&entry),
        }
    );
    assert_eq!(
        Sha256Hasher::digest_of_file(&store.model_path(&entry)).expect("the model hashes"),
        entry.sha256
    );
    assert!(!store.partial_path(&entry).exists());

    let in_place = store
        .adopt(&store.model_path(&entry), &entry)
        .expect("a file already in place is accepted");
    assert_eq!(in_place, event);
}

#[test]
fn removing_a_model_moves_it_to_the_trash_of_its_volume() {
    let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("the image attaches");
    let store = ModelStore::open(&volume.mount_point().join("models")).expect("the model store opens");
    let entry = tiny_entry();
    store
        .adopt(&assets().join("test/tiny-random.gguf"), &entry)
        .expect("the tiny model is installed");
    let installed = store.model_path(&entry);

    let outside = volume.mount_point().join("outside.gguf");
    fs::write(&outside, b"user data").expect("a file outside the store is written");
    match store.remove(&outside) {
        Err(InstallError::Io(error)) => assert_eq!(error.kind(), io::ErrorKind::InvalidInput),
        other => panic!("a file outside the store must not be removed, got {other:?}"),
    }
    assert!(outside.exists());

    let event = store.remove(&installed).expect("the model goes to the Trash");
    assert_eq!(
        event,
        ModelEvent::Removed {
            path: installed.clone()
        }
    );
    assert!(!installed.exists());
    let trash = volume
        .mount_point()
        .join(".Trashes")
        .join(current_user_id().to_string());
    assert!(trash.join(&entry.file).exists(), "the model is in the volume Trash");
}
