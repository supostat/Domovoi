use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use domovoi_app::{MeasureProgress, TrayMeasureJob};
use domovoi_index::{Enrichment, Index, IndexRoot, Reclaimable};
use domovoi_scanner::{ScanError, ScanOptions, scan};
use domovoi_testkit::{ImageVolume, Visibility, VolumeFormat, test_timeout};

const FOLDERS: usize = 40;
const FILES_PER_FOLDER: usize = 50;
const FILE_BYTES: usize = 4_096;
const LOOSE_BYTES: usize = 300_000;
const POLL: Duration = Duration::from_millis(20);

fn populate(root: &Path) {
    for folder in 0..FOLDERS {
        let directory = root.join("build").join(format!("unit-{folder}"));
        fs::create_dir_all(&directory).expect("a folder is created");
        for file in 0..FILES_PER_FOLDER {
            let content: Vec<u8> = (0..FILE_BYTES)
                .map(|position| (position + folder * 7 + file) as u8)
                .collect();
            fs::write(directory.join(format!("object-{file}.o")), content).expect("a file is written");
        }
    }
    fs::write(root.join("installer.dmg"), vec![9_u8; LOOSE_BYTES]).expect("a loose file is written");
}

fn index_of(root: &Path) -> Index {
    let report = scan(root, &ScanOptions::default()).expect("the volume scans");
    Index::build(
        &report,
        IndexRoot::for_path(root).expect("root kind"),
        &Enrichment::default(),
    )
}

fn indexed(index: &Index, path: &Path) -> Reclaimable {
    let node = index.find(path).expect("the item is indexed");
    index.reclaimable(node).expect("the index measures the item")
}

fn wait(job: &mut TrayMeasureJob) -> Vec<MeasureProgress> {
    let deadline = Instant::now() + test_timeout();
    let mut seen = Vec::new();
    while !job.is_finished() {
        assert!(Instant::now() < deadline, "the measurement did not finish in time");
        seen.push(job.progress());
        thread::sleep(POLL);
    }
    seen.push(job.progress());
    seen
}

#[test]
fn the_tray_is_measured_in_the_background_with_progress_and_can_be_cancelled() {
    let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("the image attaches");
    let root = fs::canonicalize(volume.mount_point()).expect("the mount point resolves");
    populate(&root);
    let index = index_of(&root);
    let items: Vec<PathBuf> = vec![root.join("build"), root.join("installer.dmg"), root.join("gone")];

    let mut job = TrayMeasureJob::start(items.clone());
    let seen = wait(&mut job);
    let measured = job.finish().expect("the measurement completes");
    assert_eq!(
        measured.sizes,
        [
            indexed(&index, &items[0]),
            indexed(&index, &items[1]),
            Reclaimable::default(),
        ]
    );
    assert_eq!(measured.walked.files, (FOLDERS * FILES_PER_FOLDER + 1) as u64);
    assert!(measured.walked.bytes >= (FOLDERS * FILES_PER_FOLDER * FILE_BYTES + LOOSE_BYTES) as u64);
    assert!(seen.windows(2).all(|pair| pair[0].files <= pair[1].files));
    assert!(
        seen.iter().all(|progress| progress.files <= measured.walked.files),
        "progress never runs ahead of the walk"
    );

    let job = TrayMeasureJob::start(items);
    job.cancel();
    assert!(matches!(job.finish(), Err(ScanError::Cancelled)));
}
