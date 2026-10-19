use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use domovoi_apfs_sys::{EntryAttributes, ObjectKind};
use domovoi_index::SafetyLevel;
use domovoi_index::{Enrichment, Index, IndexRoot};
use domovoi_rules::{
    CaseRule, ConfigOverrides, Denylist, InstalledSimulatorRuntimes, LevelTotals, RuleSet, rule_table,
};
use domovoi_scanner::{Node, NodeState, ScanReport};

pub const FIXTURE_HOME: &str = "/Users/fixture";
const MODEL_FILE: &str = "Library/Application Support/Domovoi/models/Qwen3.5-4B-Q4_K_M.gguf";
const SECONDS_PER_DAY: u64 = 86_400;
const FILES: [(&str, u64, u64); 31] = [
    (
        "Library/Developer/Xcode/DerivedData/Domovoi-abcdef/Build/Intermediates.noindex/data.bin",
        12_400_000_000,
        2,
    ),
    (
        "Library/Developer/Xcode/DerivedData/Widget-ghijkl/Build/Products/Widget.app/Widget",
        6_100_000_000,
        40,
    ),
    (
        "Library/Developer/Xcode/Archives/2024-03-01/Domovoi.xcarchive/Products.bin",
        3_100_000_000,
        590,
    ),
    (
        "Library/Developer/CoreSimulator/Devices/4F1C2A9E-1111-4C3B-9D2E-7A1B2C3D4E5F/data/Containers.bin",
        8_200_000_000,
        120,
    ),
    (
        "Library/Developer/CoreSimulator/Caches/dyld/dyld.bin",
        2_900_000_000,
        30,
    ),
    ("Library/Caches/Homebrew/downloads/node.tar.gz", 2_400_000_000, 15),
    ("Library/Caches/com.apple.dt.Xcode/cache.db", 1_100_000_000, 1),
    ("Library/Caches/Google/Chrome/Default/Cache/data.bin", 900_000_000, 0),
    ("Library/pnpm/store/v10/files/00/data.bin", 300_000_000, 7),
    (
        "Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw",
        40_000_000_000,
        3,
    ),
    (
        "Library/Application Support/MobileSync/Backup/00008110-001A2B3C4D5E/Manifest.db",
        25_300_000_000,
        210,
    ),
    (
        "Library/Application Support/com.tinyapp.helper/state.bin",
        1_800_000_000,
        700,
    ),
    (MODEL_FILE, 2_740_000_000, 5),
    (".npm/_cacache/content.bin", 1_600_000_000, 20),
    (".cargo/registry/cache/crates.bin", 2_200_000_000, 10),
    (".ollama/models/blobs/sha256-6a0746a1ec1a", 4_700_000_000, 90),
    ("Downloads/Xcode_16.dmg", 4_500_000_000, 60),
    ("Downloads/Docker.dmg", 620_000_000, 200),
    ("Downloads/report.pdf", 2_000_000, 8),
    ("Projects/engine/Cargo.toml", 2_000, 1),
    ("Projects/engine/src/main.rs", 40_000, 1),
    ("Projects/engine/target/debug/engine.bin", 7_500_000_000, 1),
    ("Projects/site/package.json", 2_000, 30),
    ("Projects/site/node_modules/react/index.js", 900_000_000, 30),
    ("Projects/old-site/package.json", 2_000, 420),
    ("Projects/old-site/node_modules/jquery/jquery.js", 500_000_000, 420),
    ("Documents/Taxes/2023.pdf", 5_000_000, 300),
    ("Documents/Thesis/thesis.docx", 1_200_000_000, 800),
    ("Pictures/Photos Library.photoslibrary/database.bin", 61_000_000_000, 2),
    ("Movies/Holiday.mov", 14_000_000_000, 400),
    ("Music/Music/Media.localized/library.bin", 9_000_000_000, 50),
];

#[derive(Debug)]
pub struct FixtureIndex {
    pub index: Index,
    pub rules: RuleSet,
    pub denylist: Denylist,
    pub totals: LevelTotals,
}

#[derive(Default)]
struct Folder {
    files: Vec<(String, u64, u64)>,
    folders: BTreeMap<String, Folder>,
}

impl FixtureIndex {
    pub fn build() -> Self {
        let now = SystemTime::now();
        let mut home = Folder::default();
        for (path, bytes, age_days) in FILES {
            let mut components: Vec<&str> = path.split('/').collect();
            let name = components.pop().expect("a fixture path names a file");
            let folder = components.into_iter().fold(&mut home, |folder, component| {
                folder.folders.entry(component.to_owned()).or_default()
            });
            folder.files.push((name.to_owned(), bytes, age_days));
        }
        let mut next_file_id = 1;
        let report = ScanReport {
            root: node_of(FIXTURE_HOME, &home, now, &mut next_file_id),
            files: FILES.len() as u64,
            bytes: FILES.iter().map(|(_, bytes, _)| bytes).sum(),
        };
        let enrichment = Enrichment {
            model_paths: vec![Path::new(FIXTURE_HOME).join(MODEL_FILE)],
            ..Enrichment::default()
        };
        let index = Index::build(
            &report,
            IndexRoot::Plain {
                path: FIXTURE_HOME.to_owned(),
            },
            &enrichment,
        );
        let rules = RuleSet::new(
            rule_table(&ConfigOverrides::default()),
            CaseRule::Insensitive,
            Box::new(InstalledSimulatorRuntimes::discover()),
        );
        let denylist = Denylist::new(CaseRule::Insensitive).expect("the denylist reads the system state");
        let totals = level_totals(&index, &rules);
        Self {
            index,
            rules,
            denylist,
            totals,
        }
    }

    pub fn path(relative: &str) -> PathBuf {
        Path::new(FIXTURE_HOME).join(relative)
    }
}

fn node_of(name: &str, folder: &Folder, now: SystemTime, next_file_id: &mut u64) -> Node {
    let mut children: Vec<Node> = folder
        .folders
        .iter()
        .map(|(name, child)| node_of(name, child, now, next_file_id))
        .collect();
    for (file, bytes, age_days) in &folder.files {
        *next_file_id += 1;
        let modified = now - Duration::from_secs(age_days * SECONDS_PER_DAY);
        children.push(Node {
            name: OsString::from(file),
            attributes: Some(attributes(ObjectKind::File, *next_file_id, *bytes, modified)),
            last_used: None,
            state: NodeState::Leaf,
            children: Vec::new(),
        });
    }
    *next_file_id += 1;
    Node {
        name: OsString::from(name),
        attributes: Some(attributes(ObjectKind::Directory, *next_file_id, 0, now)),
        last_used: None,
        state: NodeState::Listed,
        children,
    }
}

fn attributes(kind: ObjectKind, file_id: u64, bytes: u64, modified: SystemTime) -> EntryAttributes {
    let is_file = kind == ObjectKind::File;
    EntryAttributes {
        kind,
        flags: 0,
        file_id,
        created: modified,
        modified,
        added: Some(modified),
        mount_status: (!is_file).then_some(0),
        link_count: is_file.then_some(1),
        total_size: is_file.then_some(bytes),
        alloc_size: is_file.then_some(bytes),
        data_alloc_size: is_file.then_some(bytes),
        private_size: None,
        link_id: Some(file_id),
        real_dev: Some(1),
        clone_id: Some(file_id),
        ext_flags: Some(0),
        clone_refcount: is_file.then_some(1),
    }
}

fn level_totals(index: &Index, rules: &RuleSet) -> LevelTotals {
    let mut totals = LevelTotals::default();
    for claim in rules.claims(index) {
        let rule = rules.rule(claim.rule);
        if rule.is_info_only() {
            continue;
        }
        let bytes = index.node(claim.node).allocated;
        match rule.level {
            SafetyLevel::Safe => totals.safe += bytes,
            SafetyLevel::Recoverable => totals.recoverable += bytes,
            SafetyLevel::CheckYourself => totals.check_yourself += bytes,
        }
    }
    totals
}
