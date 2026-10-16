use std::env;
use std::time::{Duration, Instant};

use domovoi_scanner::{NodeState, ScanOptions, scan};

const MOBILE_DOCUMENTS: &str = "Library/Mobile Documents";
const SCAN_BUDGET: Duration = Duration::from_secs(2);

#[test]
#[ignore = "nightly self-hosted runner with iCloud Drive"]
fn mobile_documents_scan_without_materializing_dataless_files() {
    let home = env::home_dir().expect("home directory is known");
    let mobile_documents = home.join(MOBILE_DOCUMENTS);
    assert!(mobile_documents.is_dir(), "iCloud Drive is not set up on this host");
    let started = Instant::now();
    let report = scan(&mobile_documents, &ScanOptions::default()).expect("iCloud Drive scans");
    let elapsed = started.elapsed();
    assert!(elapsed <= SCAN_BUDGET, "iCloud Drive scan took {elapsed:?}");
    assert_ne!(report.root.state, NodeState::InCloud);
}
