use std::fs::{self, File};
use std::os::fd::AsFd;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use domovoi_apfs_sys::{chflags, link_id};
use domovoi_rules::{DeleteFailure, DeleteProgress, DeleteSubject, delete};
use domovoi_scanner::CancelToken;
use domovoi_testkit::{ImageVolume, Visibility, VolumeFormat};

const FOLDER_FILES: usize = 300;
const STORE_FOLDERS: usize = 10;
const CANCEL_AT: usize = 50;
const OUTSIDE_FILES: usize = 3;
const FILE_BYTES: usize = 4_096;
const MEASURED_FOLDER: u64 = 1_228_800;
const MEASURED_FILE: u64 = 4_096;

fn subject(location: &Path, measured_bytes: u64, indexed_files: usize) -> DeleteSubject {
    DeleteSubject {
        path: location.to_path_buf(),
        physical: location.to_path_buf(),
        link_id: link_id(location).expect("the item has a link id"),
        measured_bytes,
        indexed_files: indexed_files as u64,
        rule_id: None,
    }
}

fn folder(root: &Path, name: &str) -> PathBuf {
    let folder = root.join(name);
    fs::create_dir(&folder).expect("the folder is created");
    for number in 0..FOLDER_FILES {
        fs::write(folder.join(format!("file-{number}.js")), vec![1; FILE_BYTES]).expect("the file is written");
    }
    folder
}

fn store(root: &Path, name: &str) -> PathBuf {
    let store = root.join(name);
    for number in 0..FOLDER_FILES {
        let shard = store.join("files").join(format!("{:02}", number % STORE_FOLDERS));
        fs::create_dir_all(&shard).expect("the shard is created");
        fs::write(shard.join(format!("blob-{number}")), vec![5; FILE_BYTES]).expect("the blob is written");
    }
    store
}

fn file(root: &Path, name: &str) -> PathBuf {
    let file = root.join(name);
    fs::write(&file, vec![2; FILE_BYTES]).expect("the file is written");
    file
}

fn image() -> (ImageVolume, PathBuf) {
    let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("the image attaches");
    let root = fs::canonicalize(volume.mount_point()).expect("the mount point resolves");
    (volume, root)
}

fn files_under(path: &Path) -> usize {
    fs::read_dir(path)
        .expect("the folder is listed")
        .map(|entry| entry.expect("the entry is read").path())
        .map(|child| if child.is_dir() { files_under(&child) } else { 1 })
        .sum()
}

#[test]
fn a_delete_removes_the_subjects_and_counts_their_files() {
    let (_volume, root) = image();
    let subjects = vec![
        subject(&folder(&root, "cache"), MEASURED_FOLDER, FOLDER_FILES),
        subject(&file(&root, "installer.dmg"), MEASURED_FILE, 1),
    ];
    let steps = Mutex::new(Vec::new());

    let report = delete(subjects.clone(), &CancelToken::default(), &|step: DeleteProgress| {
        steps.lock().expect("the steps are not poisoned").push(step);
    });

    assert_eq!(report.deleted, subjects);
    assert!(report.untouched.is_empty() && report.interrupted.is_empty() && report.failed.is_empty());
    assert_eq!(report.freed_bytes, MEASURED_FOLDER + MEASURED_FILE);
    assert!(
        subjects
            .iter()
            .all(|subject| subject.physical.symlink_metadata().is_err()),
        "the disk no longer holds them"
    );
    let steps = steps.into_inner().expect("the steps are not poisoned");
    let last = steps
        .iter()
        .max_by_key(|step| step.files_done)
        .expect("the delete reports progress");
    assert_eq!(last.files_done, last.files_total, "progress reaches the total");
    assert_eq!(
        last.files_total,
        FOLDER_FILES + 1,
        "the files of the folder and the file"
    );
}

#[test]
fn progress_moves_file_by_file_inside_one_large_folder() {
    let (_volume, root) = image();
    let store = store(&root, "v3");
    let subjects = vec![subject(&store, MEASURED_FOLDER, FOLDER_FILES)];
    let steps = Mutex::new(Vec::new());

    let report = delete(subjects.clone(), &CancelToken::default(), &|step: DeleteProgress| {
        steps.lock().expect("the steps are not poisoned").push(step);
    });

    assert_eq!(report.deleted, subjects);
    assert!(
        store.symlink_metadata().is_err(),
        "the nested folders go with the store"
    );
    let inside: Vec<usize> = steps
        .into_inner()
        .expect("the steps are not poisoned")
        .iter()
        .filter(|step| step.subjects_done == 0)
        .map(|step| step.files_done)
        .collect();
    assert_eq!(
        inside,
        (1..=FOLDER_FILES).collect::<Vec<_>>(),
        "every file of the only child is a step before the store is done"
    );
}

#[test]
fn stopping_inside_a_large_folder_leaves_the_rest_on_disk() {
    let (_volume, root) = image();
    let store = store(&root, "v3");
    let subjects = vec![subject(&store, MEASURED_FOLDER, FOLDER_FILES)];
    let cancel = CancelToken::default();

    let report = delete(subjects.clone(), &cancel, &|step: DeleteProgress| {
        if step.files_done == CANCEL_AT {
            cancel.cancel();
        }
    });

    assert_eq!(report.interrupted, subjects);
    assert!(report.deleted.is_empty() && report.failed.is_empty() && report.untouched.is_empty());
    assert_eq!(report.freed_bytes, 0, "an interrupted item frees nothing in the report");
    assert_eq!(
        files_under(&store),
        FOLDER_FILES - CANCEL_AT,
        "the walk stops at the next file"
    );
}

#[test]
fn a_link_inside_an_item_goes_without_its_target() {
    let (_volume, root) = image();
    let outside = root.join("shared");
    fs::create_dir(&outside).expect("the outside folder is created");
    for number in 0..OUTSIDE_FILES {
        file(&outside, &format!("kept-{number}"));
    }
    let outside_file = file(&root, "kept.txt");
    let project = root.join("project");
    fs::create_dir(&project).expect("the project is created");
    file(&project, "build.log");
    symlink(&outside, project.join("shared")).expect("the folder link is created");
    symlink(&outside_file, project.join("note")).expect("the file link is created");
    let subjects = vec![subject(&project, MEASURED_FILE, 3)];

    let report = delete(subjects.clone(), &CancelToken::default(), &|_| {});

    assert_eq!(report.deleted, subjects);
    assert!(project.symlink_metadata().is_err());
    assert_eq!(
        files_under(&outside),
        OUTSIDE_FILES,
        "the linked folder keeps its files"
    );
    assert!(outside_file.exists(), "the linked file stays");
}

#[test]
fn a_locked_file_keeps_its_folder_while_its_neighbours_go() {
    let (_volume, root) = image();
    let folder = folder(&root, "cache");
    let locked = folder.join("file-7.js");
    let handle = File::open(&locked).expect("the locked file opens");
    chflags(handle.as_fd(), libc::UF_IMMUTABLE).expect("the file is locked");
    let subjects = vec![subject(&folder, MEASURED_FOLDER, FOLDER_FILES)];

    let report = delete(subjects.clone(), &CancelToken::default(), &|_| {});

    assert_eq!(report.failed, [(subjects[0].clone(), DeleteFailure::NoPermission)]);
    assert_eq!(files_under(&folder), 1, "only the locked file stays");
    assert!(locked.exists());
    chflags(handle.as_fd(), 0).expect("the file is unlocked");
}

#[test]
fn a_delete_leaves_a_replaced_item_and_reports_what_is_gone() {
    let (_volume, root) = image();
    let subjects = vec![
        subject(&file(&root, "replaced.bin"), MEASURED_FILE, 1),
        subject(&file(&root, "gone.bin"), MEASURED_FILE, 1),
    ];
    let replaced = &subjects[0].physical;
    fs::remove_file(replaced).expect("the file is removed");
    fs::write(replaced, b"someone else's file").expect("another file takes its place");
    fs::remove_file(&subjects[1].physical).expect("the file went away elsewhere");

    let report = delete(subjects.clone(), &CancelToken::default(), &|_| {});

    assert!(report.deleted.is_empty());
    assert_eq!(report.freed_bytes, 0);
    assert_eq!(
        report.failed,
        [
            (subjects[0].clone(), DeleteFailure::Replaced),
            (subjects[1].clone(), DeleteFailure::Missing),
        ]
    );
    assert_eq!(
        fs::read(replaced).expect("the replacement stays"),
        b"someone else's file"
    );
}

#[test]
fn a_cancelled_delete_keeps_the_untouched_subjects() {
    let (_volume, root) = image();
    let subjects = vec![
        subject(&folder(&root, "first"), MEASURED_FOLDER, FOLDER_FILES),
        subject(&folder(&root, "second"), MEASURED_FOLDER, FOLDER_FILES),
    ];
    let cancel = CancelToken::default();

    let report = delete(subjects.clone(), &cancel, &|step: DeleteProgress| {
        if step.subjects_done == 1 {
            cancel.cancel();
        }
    });

    assert_eq!(report.deleted, [subjects[0].clone()]);
    assert_eq!(report.untouched, [subjects[1].clone()]);
    assert_eq!(report.freed_bytes, MEASURED_FOLDER);
    assert!(subjects[0].physical.symlink_metadata().is_err());
    assert_eq!(
        fs::read_dir(&subjects[1].physical)
            .expect("the untouched folder stays")
            .count(),
        FOLDER_FILES,
        "nothing of the untouched subject is removed"
    );
}

#[test]
fn a_read_only_volume_fails_per_subject_without_stopping_the_batch() {
    let (mut volume, root) = image();
    let subjects = vec![
        subject(&file(&root, "first"), MEASURED_FILE, 1),
        subject(&folder(&root, "second"), MEASURED_FOLDER, FOLDER_FILES),
    ];
    volume.detach().expect("the image detaches");
    volume.attach_read_only().expect("the image reattaches read-only");

    let report = delete(subjects.clone(), &CancelToken::default(), &|_| {});

    assert!(report.deleted.is_empty());
    assert_eq!(report.failed.len(), 2, "{report:?}");
    for ((subject, failure), expected) in report.failed.iter().zip(&subjects) {
        assert_eq!(subject, expected);
        assert_eq!(*failure, DeleteFailure::ReadOnlyVolume, "{}", subject.path.display());
    }
}
