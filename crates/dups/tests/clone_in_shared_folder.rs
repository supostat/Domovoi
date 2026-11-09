use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use domovoi_apfs_sys::stream_identity;
use domovoi_dups::{CloneJournal, ClonePlan, CloudFolders};
use domovoi_index::{Enrichment, Index, IndexRoot, NodeId};
use domovoi_rules::{CaseRule, Denylist};
use domovoi_scanner::{CancelToken, ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};

const SHARED: &str = "/Users/Shared";
const SIZE: usize = 2 * 1024 * 1024;

struct SharedCopy {
    path: PathBuf,
}

impl SharedCopy {
    fn write(contents: &[u8]) -> Self {
        let path = Path::new(SHARED).join(format!(".domovoi-clone-copy-{}.raw", process::id()));
        fs::write(&path, contents).expect("a copy is written straight into the shared folder");
        Self { path }
    }
}

impl Drop for SharedCopy {
    fn drop(&mut self) {
        fs::remove_file(&self.path).expect("the copy leaves the shared folder");
    }
}

fn node_at(index: &Index, path: &Path) -> NodeId {
    index
        .find(path)
        .unwrap_or_else(|| panic!("{} is indexed", path.display()))
}

#[test]
fn a_copy_in_a_folder_of_another_owner_is_reported_as_replaced() {
    let kept_tree = TempTree::under(Path::new(SHARED)).expect("a kept folder is created");
    kept_tree
        .root()
        .expect("the kept folder opens")
        .file("report.raw", SIZE as u64, Fill::Byte(7))
        .expect("the kept file is written");
    let kept = kept_tree.path().join("report.raw");
    let copy = SharedCopy::write(&vec![7_u8; SIZE]);
    let journal_tree = TempTree::new().expect("a journal folder is created");
    let report = scan(Path::new(SHARED), &ScanOptions::default()).expect("the shared folder scans");
    let index = Index::build(
        &report,
        IndexRoot::for_path(Path::new(SHARED)).expect("the root kind is known"),
        &Enrichment::default(),
    );
    let denylist = Denylist::new(CaseRule::Insensitive).expect("the denylist is prepared");
    let plan = ClonePlan::prepare(
        &index,
        node_at(&index, &kept),
        node_at(&index, &copy.path),
        &denylist,
        &CloudFolders::new(Vec::new()),
    )
    .expect("the copy can be replaced");
    let mut journal = CloneJournal::open(&journal_tree.path().join("clone-journal")).expect("the journal opens");

    let replaced = plan.execute(&mut journal, &CancelToken::default(), &|_| {});

    let copy_stream = stream_identity(&copy.path).expect("the copy is still there");
    let kept_stream = stream_identity(&kept).expect("the kept file is still there");
    assert_eq!(
        copy_stream.clone_id, kept_stream.clone_id,
        "the copy became a clone of the kept file"
    );
    let replaced = replaced.expect("a replacement that happened is reported as replaced");
    assert!(replaced.freed_bytes > 0, "{replaced:?}");
}
