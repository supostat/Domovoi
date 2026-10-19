use std::ffi::OsStr;
use std::fs::{self, File};
use std::os::fd::AsFd;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use domovoi_apfs_sys::{clone_at, get_xattr, link_id, private_size, set_xattr, stream_identity};
use domovoi_dups::{
    CloneJournal, ClonePlan, CloneRefusal, CloneReplaceError, CloudFolders, DuplicateOptions, HashingPool,
    PendingReplacement, TEMPORARY_PREFIX, find_duplicates,
};
use domovoi_index::{Enrichment, Index, IndexRoot, NodeId, Reclaimable};
use domovoi_rules::{CaseRule, Denylist, TrashJournal};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{ImageVolume, TempTree, Visibility, VolumeFormat};

const FILE_SIZE: usize = 256 * 1024;
const SPARSE_SIZE: u64 = 1024 * 1024;
const FINDER_INFO: &str = "com.apple.FinderInfo";
const FINDER_INFO_LENGTH: usize = 32;
const BUSY_FINDER_INFO_OFFSET: usize = 24;
const OBJECT_IS_BUSY: u16 = 0x0080;
const COPY_TAG: &str = "com.domovoi.test.copy";
const ORIGINAL_TAG: &str = "com.domovoi.test.original";

struct HoldingChild(Child);

impl Drop for HoldingChild {
    fn drop(&mut self) {
        self.0.kill().expect("holding child is killed");
        self.0.wait().expect("holding child is reaped");
    }
}

struct Fixture {
    volume: ImageVolume,
    index: Index,
    denylist: Denylist,
    cloud_folders: CloudFolders,
    trash_journal: TrashJournal,
    clone_journal: CloneJournal,
    _workspace: TempTree,
}

impl Fixture {
    fn new() -> Self {
        let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("image attaches");
        create_scenarios(volume.mount_point());
        let workspace = TempTree::new().expect("journal workspace is created");
        Self {
            index: index_of(volume.mount_point()),
            denylist: Denylist::new(CaseRule::Insensitive).expect("denylist is prepared"),
            cloud_folders: CloudFolders::new(Vec::new()),
            trash_journal: TrashJournal::open(&workspace.path().join("trash-journal")).expect("journal opens"),
            clone_journal: CloneJournal::open(&workspace.path().join("clone-journal")).expect("journal opens"),
            _workspace: workspace,
            volume,
        }
    }

    fn mount(&self) -> &Path {
        self.volume.mount_point()
    }

    fn node(&self, relative: &str) -> NodeId {
        let mut current = self.index.root();
        for name in Path::new(relative).components() {
            current = self
                .index
                .child_named(current, name.as_os_str())
                .unwrap_or_else(|| panic!("{relative} is indexed"));
        }
        current
    }

    fn prepare(&self, scenario: &str) -> Result<ClonePlan, CloneReplaceError> {
        ClonePlan::prepare(
            &self.index,
            self.node(&format!("{scenario}/original")),
            self.node(&format!("{scenario}/copy")),
            &self.denylist,
            &self.cloud_folders,
        )
    }

    fn refusal(&self, scenario: &str) -> CloneRefusal {
        match self.prepare(scenario) {
            Err(CloneReplaceError::Refused(refusal)) => refusal,
            other => panic!("expected a refusal for {scenario}, got {other:?}"),
        }
    }
}

fn content(seed: u8) -> Vec<u8> {
    (0..FILE_SIZE)
        .map(|position| (position as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

fn write_pair(directory: &Path, seed: u8) {
    fs::create_dir(directory).expect("scenario directory is created");
    fs::write(directory.join("original"), content(seed)).expect("original is written");
    fs::write(directory.join("copy"), content(seed)).expect("copy is written");
}

fn clone_file(directory: &Path, source: &str, target: &str) {
    let handle = File::open(directory).expect("directory opens");
    clone_at(handle.as_fd(), OsStr::new(source), handle.as_fd(), OsStr::new(target)).expect("file is cloned");
}

fn tag(path: &Path, key: &str, value: &[u8]) {
    set_xattr(File::open(path).expect("file opens").as_fd(), OsStr::new(key), value)
        .expect("extended attribute is set");
}

fn create_scenarios(mount: &Path) {
    write_pair(&mount.join("hard"), 1);
    fs::hard_link(mount.join("hard/copy"), mount.join("hard/copy-link")).expect("hard link is created");
    fs::create_dir(mount.join("clones")).expect("directory is created");
    fs::write(mount.join("clones/original"), content(2)).expect("original is written");
    clone_file(&mount.join("clones"), "original", "copy");
    fs::create_dir(mount.join("sparse")).expect("directory is created");
    for name in ["original", "copy"] {
        File::create(mount.join("sparse").join(name))
            .expect("sparse file is created")
            .set_len(SPARSE_SIZE)
            .expect("sparse file is extended");
    }
    write_pair(&mount.join("busy"), 3);
    let mut finder_info = [0_u8; FINDER_INFO_LENGTH];
    finder_info[BUSY_FINDER_INFO_OFFSET..BUSY_FINDER_INFO_OFFSET + 2].copy_from_slice(&OBJECT_IS_BUSY.to_be_bytes());
    tag(&mount.join("busy/copy"), FINDER_INFO, &finder_info);
    write_pair(&mount.join("open"), 4);
    write_pair(&mount.join("success"), 5);
    tag(&mount.join("success/copy"), COPY_TAG, b"copy");
    tag(&mount.join("success/original"), ORIGINAL_TAG, b"original");
}

fn index_of(root: &Path) -> Index {
    let report = scan(root, &ScanOptions::default()).expect("image scans");
    Index::build(
        &report,
        IndexRoot::for_path(root).expect("root kind is known"),
        &Enrichment::default(),
    )
}

fn unsafe_pairs_are_refused(fixture: &Fixture) {
    assert_eq!(fixture.refusal("hard"), CloneRefusal::HardLinked);
    assert_eq!(fixture.refusal("busy"), CloneRefusal::Busy);
    let holder = HoldingChild(
        Command::new("/bin/sleep")
            .arg("30")
            .stdin(Stdio::from(
                File::open(fixture.mount().join("open/copy")).expect("copy opens"),
            ))
            .spawn()
            .expect("holding child starts"),
    );
    let holder_pid = libc::pid_t::try_from(holder.0.id()).expect("pid fits in pid_t");
    match fixture.refusal("open") {
        CloneRefusal::OpenElsewhere(holders) => assert!(holders.contains(&holder_pid), "{holders:?}"),
        other => panic!("expected an open-file refusal, got {other:?}"),
    }
}

fn existing_clones_have_no_gain(fixture: &Fixture) {
    let pool = HashingPool::new(None).expect("hashing pool starts");
    let duplicates = find_duplicates(
        &fixture.index,
        &pool,
        &DuplicateOptions {
            minimum_size: 1,
            ..DuplicateOptions::new(fixture.cloud_folders.clone())
        },
    )
    .expect("search completes");
    let clones = duplicates
        .groups
        .iter()
        .find(|group| group.keep == fixture.node("clones/original"))
        .expect("clones form a group");
    assert!(clones.copies.iter().all(|copy| copy.shares_blocks));
    assert_eq!(clones.reclaimable, Reclaimable::default());
    assert_eq!(fixture.refusal("clones"), CloneRefusal::AlreadyClones);
}

fn sparse_copies_are_replaced(fixture: &mut Fixture) {
    fixture
        .prepare("sparse")
        .expect("sparse pair is replaceable")
        .execute(&mut fixture.trash_journal, &mut fixture.clone_journal)
        .expect("sparse copy is replaced");
    let copy = fixture.mount().join("sparse/copy");
    assert_eq!(fs::metadata(&copy).expect("metadata").len(), SPARSE_SIZE);
    assert!(
        fs::read(&copy)
            .expect("sparse copy is read")
            .iter()
            .all(|byte| *byte == 0)
    );
}

fn a_replaced_copy_shares_blocks_and_its_old_body_comes_back(fixture: &mut Fixture) {
    let directory = fixture.mount().join("success");
    let copy = directory.join("copy");
    let original = directory.join("original");
    let directory_modified = fs::metadata(&directory).expect("metadata").modified().expect("mtime");
    let plan = fixture.prepare("success").expect("independent pair is replaceable");
    assert!(!plan.low_space);
    let replaced = plan
        .execute(&mut fixture.trash_journal, &mut fixture.clone_journal)
        .expect("copy is replaced with a clone");

    assert!(
        replaced.trashed.starts_with(fixture.mount().join(".Trashes")),
        "{}",
        replaced.trashed.display()
    );
    assert_named_like_copy(&replaced.trashed);
    assert_eq!(private_size(&copy).expect("private size"), 0);
    assert_eq!(
        stream_identity(&copy).expect("copy stream").clone_id,
        stream_identity(&original).expect("original stream").clone_id
    );
    assert_eq!(fs::read(&copy).expect("copy is read"), content(5));
    let copy_file = File::open(&copy).expect("copy opens");
    assert_eq!(
        get_xattr(copy_file.as_fd(), OsStr::new(COPY_TAG)).expect("tag is read"),
        Some(b"copy".to_vec())
    );
    assert_eq!(
        get_xattr(copy_file.as_fd(), OsStr::new(ORIGINAL_TAG)).expect("tag is read"),
        None
    );
    assert_eq!(
        fs::metadata(&directory).expect("metadata").modified().expect("mtime"),
        directory_modified
    );
    assert!(fixture.clone_journal.pending().is_empty());
    let last = fixture.trash_journal.entries().len() - 1;
    assert_eq!(
        fixture.trash_journal.entries()[last].original,
        fixture.index.path(fixture.node("success/copy")).to_string_lossy()
    );
    assert_eq!(
        Path::new(&fixture.trash_journal.entries()[last].trashed),
        replaced.trashed
    );
    let restored = fixture.trash_journal.restore(last).expect("old body comes back");
    assert_eq!(restored, directory.join("copy 2"));
    assert_eq!(fs::read(&restored).expect("old body is read"), content(5));
}

fn assert_named_like_copy(trashed: &Path) {
    let name = trashed
        .file_name()
        .and_then(|name| name.to_str())
        .expect("a trashed item has a UTF-8 name");
    let numbered = name
        .strip_prefix("copy ")
        .is_some_and(|number| !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit()));
    assert!(
        !name.starts_with(TEMPORARY_PREFIX) && (name == "copy" || numbered),
        "the old body lies in the Trash as {name}"
    );
}

fn pending(copy: &Path, temporary: &Path, link_id: u64) -> PendingReplacement {
    PendingReplacement {
        copy: copy.to_string_lossy().into_owned(),
        copy_physical: copy.to_string_lossy().into_owned(),
        temporary: temporary.to_string_lossy().into_owned(),
        link_id,
        alloc_size: FILE_SIZE as u64,
        flags: 0,
        clone_id: 0,
    }
}

fn stranded_temporaries_are_swept_into_the_trash(fixture: &mut Fixture) {
    let directory = fixture.mount().join("stranded");
    fs::create_dir(&directory).expect("directory is created");
    fs::write(directory.join("original"), content(6)).expect("original is written");
    let stray_clone_name = format!("{TEMPORARY_PREFIX}clone");
    clone_file(&directory, "original", &stray_clone_name);
    let stray_clone = directory.join(stray_clone_name);
    let old_body = directory.join(format!("{TEMPORARY_PREFIX}body"));
    fs::write(&old_body, content(7)).expect("old body is written");
    let copy = directory.join("copy");
    fixture
        .clone_journal
        .begin(pending(&copy, &stray_clone, 0))
        .expect("stray clone is journaled");
    fixture
        .clone_journal
        .begin(pending(&copy, &old_body, link_id(&old_body).expect("link id")))
        .expect("old body is journaled");
    let journaled_before = fixture.trash_journal.entries().len();

    let outcomes = fixture
        .clone_journal
        .sweep(&mut fixture.trash_journal)
        .expect("sweep runs");

    assert_eq!(outcomes.len(), 2);
    assert!(outcomes.iter().all(|outcome| outcome.result.is_ok()), "{outcomes:?}");
    assert!(!stray_clone.exists() && !old_body.exists());
    assert!(fixture.clone_journal.pending().is_empty());
    assert_eq!(fixture.trash_journal.entries().len(), journaled_before + 1);
    assert_eq!(
        PathBuf::from(&fixture.trash_journal.entries()[journaled_before].original_physical),
        copy
    );
    assert_named_like_copy(Path::new(&fixture.trash_journal.entries()[journaled_before].trashed));
}

#[test]
fn clone_replacement_checks_preconditions_swaps_atomically_and_stays_reversible() {
    let mut fixture = Fixture::new();
    unsafe_pairs_are_refused(&fixture);
    existing_clones_have_no_gain(&fixture);
    sparse_copies_are_replaced(&mut fixture);
    a_replaced_copy_shares_blocks_and_its_old_body_comes_back(&mut fixture);
    stranded_temporaries_are_swept_into_the_trash(&mut fixture);
}
