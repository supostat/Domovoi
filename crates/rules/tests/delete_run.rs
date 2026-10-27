use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use domovoi_apfs_sys::link_id;
use domovoi_rules::{DeleteFailure, DeleteProgress, DeleteSubject, delete};
use domovoi_scanner::CancelToken;
use domovoi_testkit::{ImageVolume, Visibility, VolumeFormat};

const FOLDER_FILES: usize = 300;
const FILE_BYTES: usize = 4_096;
const MEASURED_FOLDER: u64 = 1_228_800;
const MEASURED_FILE: u64 = 4_096;

fn subject(location: &Path, measured_bytes: u64) -> DeleteSubject {
    DeleteSubject {
        path: location.to_path_buf(),
        physical: location.to_path_buf(),
        link_id: link_id(location).expect("the item has a link id"),
        measured_bytes,
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

#[test]
fn a_delete_removes_the_subjects_and_sums_the_measured_bytes() {
    let (_volume, root) = image();
    let subjects = vec![
        subject(&folder(&root, "cache"), MEASURED_FOLDER),
        subject(&file(&root, "installer.dmg"), MEASURED_FILE),
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
        .max_by_key(|step| step.done)
        .expect("the delete reports progress");
    assert_eq!(last.done, last.total, "progress reaches the total");
    assert_eq!(
        last.total,
        FOLDER_FILES + 2,
        "the folder's children, the folder and the file"
    );
}

#[test]
fn a_delete_leaves_a_replaced_item_and_reports_what_is_gone() {
    let (_volume, root) = image();
    let subjects = vec![
        subject(&file(&root, "replaced.bin"), MEASURED_FILE),
        subject(&file(&root, "gone.bin"), MEASURED_FILE),
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
        subject(&folder(&root, "first"), MEASURED_FOLDER),
        subject(&folder(&root, "second"), MEASURED_FOLDER),
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
        subject(&file(&root, "first"), MEASURED_FILE),
        subject(&file(&root, "second"), MEASURED_FILE),
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
