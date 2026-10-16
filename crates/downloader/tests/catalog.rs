use std::fs;
use std::path::{Path, PathBuf};

use domovoi_downloader::{Catalog, ModelStore, Sha256Digest};
use domovoi_grammar::PromptFamily;
use domovoi_index::{Category, Enrichment, Index, IndexRoot, ModelEvent};
use domovoi_inference_proto::{ClassFootprint, MemoryBudget, ModelClass};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::TempTree;
use serde_json::Value;

const ALLOWED_LICENSES: [&str; 2] = ["Apache-2.0", "MIT"];
const GIBIBYTE: u64 = 1024 * 1024 * 1024;
const WORKING_SET_PERCENT: u64 = 74;

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn test_catalog() -> Catalog {
    Catalog::from_path(&assets().join("test/catalog.json")).expect("the test catalog parses")
}

#[test]
fn every_bundled_entry_is_permissive_chat_ml_and_pinned_by_size_and_sha256() {
    let catalog = Catalog::bundled();
    assert!(!catalog.entries.is_empty());
    for entry in &catalog.entries {
        assert!(ALLOWED_LICENSES.contains(&entry.license.as_str()), "{}", entry.id);
        assert!(entry.license_source.starts_with("https://"), "{}", entry.id);
        assert_eq!(entry.template.family, PromptFamily::ChatMl, "{}", entry.id);
        assert!(entry.size > 0, "{}", entry.id);
        assert_ne!(entry.sha256, Sha256Digest::from_bytes([0; 32]), "{}", entry.id);
        assert_eq!(
            entry.context_length,
            ClassFootprint::of(entry.class).context_length,
            "{}",
            entry.id
        );
    }
}

#[test]
fn each_class_has_exactly_one_primary_entry() {
    let catalog = Catalog::bundled();
    for class in ModelClass::ALL {
        let primaries = catalog
            .entries
            .iter()
            .filter(|entry| entry.class == class && entry.primary)
            .count();
        assert_eq!(primaries, 1, "{class:?}");
    }
}

#[test]
fn the_default_model_follows_the_memory_of_the_mac_and_its_gpu_budget() {
    let catalog = Catalog::bundled();
    let chosen =
        |memory: u64, budget: Option<MemoryBudget>| catalog.default_entry(memory, budget).map(|entry| entry.id.clone());
    let budget_of = |memory: u64| MemoryBudget {
        working_set_bytes: memory * WORKING_SET_PERCENT / 100,
    };
    assert_eq!(chosen(8 * GIBIBYTE, None).as_deref(), Some("qwen3.5-4b"));
    assert_eq!(chosen(16 * GIBIBYTE, None).as_deref(), Some("t-lite-it-2.1"));
    assert_eq!(chosen(24 * GIBIBYTE, None).as_deref(), Some("t-lite-it-2.1"));
    assert_eq!(chosen(36 * GIBIBYTE, None).as_deref(), Some("qwen3-14b"));
    for memory in [8, 16, 32].map(|gibibytes| gibibytes * GIBIBYTE) {
        let offered = chosen(memory, None);
        assert_eq!(chosen(memory, Some(budget_of(memory))), offered, "{memory}");
    }
    assert_eq!(
        chosen(32 * GIBIBYTE, Some(budget_of(8 * GIBIBYTE))).as_deref(),
        Some("qwen3.5-4b")
    );
    assert_eq!(chosen(8 * GIBIBYTE, Some(budget_of(GIBIBYTE))), None);
}

#[test]
fn an_installed_model_emits_the_event_that_marks_it_in_the_index() {
    let tree = TempTree::new().expect("a temp tree is created");
    let store = ModelStore::open(&tree.path().join("models")).expect("the model store opens");
    let entry = test_catalog().entries[0].clone();
    let event = store
        .adopt(&assets().join("test/tiny-random.gguf"), &entry)
        .expect("the tiny model is accepted by its hash");
    let installed = store.model_path(&entry);
    assert_eq!(
        event,
        ModelEvent::Downloaded {
            path: installed.clone()
        }
    );

    let report = scan(tree.path(), &ScanOptions::default()).expect("the tree scans");
    let root = IndexRoot::for_path(tree.path()).expect("the root kind is known");
    let mut enrichment = Enrichment::default();
    let mut index = Index::build(&report, root, &enrichment);
    let canonical = fs::canonicalize(&installed).expect("the model has a canonical path");
    index.apply_model_event(
        &ModelEvent::Downloaded {
            path: canonical.clone(),
        },
        &mut enrichment,
    );
    let node = index.find(&canonical).expect("the model is indexed");
    assert_eq!(index.node(node).category, Category::Model);
}

#[test]
fn the_test_catalog_offers_the_committed_tiny_model() {
    let catalog = test_catalog();
    assert_eq!(catalog.entries.len(), 1);
    let entry = &catalog.entries[0];
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(assets().join("test/manifest.json")).expect("the test model manifest reads"),
    )
    .expect("the manifest is JSON");
    let tiny = manifest["models"]
        .as_array()
        .expect("the manifest lists models")
        .iter()
        .find(|model| model["name"] == entry.file.as_str())
        .expect("the manifest describes the tiny model");
    assert_eq!(tiny["size"].as_u64(), Some(entry.size));
    assert_eq!(tiny["sha256"].as_str(), Some(entry.sha256.to_string().as_str()));
    assert!(
        Catalog::bundled().entry(&entry.id).is_none(),
        "test entries stay out of the bundled catalog"
    );
}
