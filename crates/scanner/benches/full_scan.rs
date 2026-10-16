use std::env;
use std::process::Command;
use std::time::Instant;

use domovoi_apfs_sys::{AttributeSet, data_volume};
use domovoi_scanner::{ScanOptions, scan};

const PURGE_VARIABLE: &str = "DOMOVOI_BENCH_PURGE";
const VARIANTS: [(&str, AttributeSet); 3] = [
    ("base", AttributeSet::Base),
    ("+cmnext", AttributeSet::Extended),
    ("+privatesize", AttributeSet::ExtendedWithPrivateSize),
];

fn main() {
    let data = data_volume().expect("the root data volume is mounted");
    let purge = env::var(PURGE_VARIABLE).is_ok_and(|value| value == "1");
    for (label, attributes) in VARIANTS {
        let first_pass = if purge {
            let status = Command::new("/usr/sbin/purge").status().expect("purge runs");
            assert!(status.success(), "purge needs root: {status}");
            "cold"
        } else {
            "first"
        };
        for pass in [first_pass, "warm"] {
            let options = ScanOptions {
                attributes,
                ..ScanOptions::default()
            };
            let started = Instant::now();
            let report = scan(&data, &options).expect("the data volume scans");
            println!(
                "full_scan/{label}/{pass}: {:.1} s, {} files, {} bytes",
                started.elapsed().as_secs_f64(),
                report.files,
                report.bytes
            );
        }
    }
}
