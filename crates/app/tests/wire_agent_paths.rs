use std::fs::{self, File};
use std::path::{Path, PathBuf};

use domovoi_agent::{AgentAnswer, AgentLimits};
use domovoi_app::{Session, SessionPaths};
use domovoi_downloader::{Catalog, CatalogEntry};
use domovoi_inference::{CancelFlag, FakeBackend, InferenceBackend};
use domovoi_scanner::CancelToken;
use domovoi_testkit::{ImageVolume, TempTree, Visibility, VolumeFormat};
use serde_json::{Value, json};

const HOME: &str = "Users/tester";
const STORE: &str = "Library/pnpm/store";
const STORE_FILE: &str = "v10/files/00/data.bin";
const STORE_BYTES: usize = 1024 * 1024;
const TARGET_RULE: &str = "cargo.target";
const PROJECT: &str = "Projects/engine";
const QUESTION: &str = "Что насчёт хранилища pnpm?";
const STEPS: u32 = 4;

fn say(response: &Value) -> String {
    format!("token {response}")
}

fn tool_call(name: &str, arguments: &Value) -> String {
    say(&json!({"kind": "tool_call", "name": name, "arguments": arguments}))
}

fn populate(home: &Path) {
    let store = home.join(STORE).join(STORE_FILE);
    fs::create_dir_all(store.parent().expect("the store file has a folder")).expect("the store is created");
    fs::write(&store, vec![3_u8; STORE_BYTES]).expect("the store file is written");
    let project = home.join(PROJECT);
    fs::create_dir_all(project.join("target/debug")).expect("the project is created");
    fs::write(project.join("Cargo.toml"), "[package]\nname = \"engine\"\n").expect("the manifest is written");
    fs::write(project.join("target/debug/engine"), vec![5_u8; STORE_BYTES]).expect("the build output is written");
}

fn catalog_entry() -> CatalogEntry {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/test/catalog.json");
    Catalog::from_path(&path).expect("the test catalog parses").entries[0].clone()
}

fn ask(session: &Session, entry: &CatalogEntry, weights: &Path) -> AgentAnswer {
    let mut backend = FakeBackend::replying([
        tool_call("rule_explain", &json!({"id": TARGET_RULE})),
        tool_call("query_index", &json!({"path_prefix": "~/Library/pnpm/store"})),
        tool_call("query_index", &json!({"path_prefix": "/Users/igor/.pnpm"})),
        say(&json!({
            "kind": "final",
            "answer": "Хранилище pnpm лежит в домашней папке.",
            "hypotheses": [{"path": "~/Library/pnpm/store", "guess": "Похоже на хранилище пакетов pnpm", "confidence": 0.9}]
        })),
    ]);
    backend
        .load(
            File::open(weights).expect("the weights open").into(),
            entry.class,
            entry.context_length,
            &CancelFlag::default(),
        )
        .expect("the fake model loads");
    session
        .ask(
            &mut backend,
            entry,
            AgentLimits {
                max_steps: STEPS,
                ..AgentLimits::for_context_length(entry.context_length)
            },
            QUESTION,
            &CancelFlag::default(),
            &mut |_| {},
        )
        .expect("the agent answers through the session")
}

fn listed(result: &Value, field: &str) -> Vec<PathBuf> {
    result[field]
        .as_array()
        .unwrap_or_else(|| panic!("the result lists {field}: {result}"))
        .iter()
        .map(|item| PathBuf::from(item["path"].as_str().expect("an item has a path")))
        .collect()
}

#[test]
fn home_relative_paths_and_rule_findings_reach_the_answer_through_the_session() {
    let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("the image attaches");
    let mount_point = fs::canonicalize(volume.mount_point()).expect("the mount point resolves");
    let home = mount_point.join(HOME);
    populate(&home);
    let state = TempTree::new().expect("a temp tree is created");
    let paths = SessionPaths::in_directory(state.path()).with_home(&home);
    let mut session = Session::open(&mount_point, &paths, "0.0.0-test").expect("the session opens");
    session.scan(None, CancelToken::default()).expect("the volume scans");
    let store = home.join(STORE);
    assert!(
        session.index().expect("the volume is scanned").find(&store).is_some(),
        "home is written in the index's own form"
    );
    let weights = state.path().join("weights.gguf");
    fs::write(&weights, b"GGUF wire").expect("the weights are written");

    let answer = ask(&session, &catalog_entry(), &weights);

    let target = home.join(PROJECT).join("target");
    let rule = &answer.steps[0].result;
    assert_eq!(rule["paths"], json!(["**/target|Cargo.toml"]));
    assert_eq!(listed(rule, "found"), std::slice::from_ref(&target));
    assert_eq!(listed(&answer.steps[1].result, "items"), std::slice::from_ref(&store));
    let missing = &answer.steps[2].result;
    assert_eq!(missing["total"], 0);
    assert_eq!(missing["prefix_found"], false);
    assert_eq!(answer.hypotheses.len(), 1);
    assert_eq!(answer.hypotheses[0].path, store);
    assert!(
        answer
            .actions
            .iter()
            .any(|action| action.rule_id == TARGET_RULE && action.path == target),
        "the rule's finding becomes an action: {:?}",
        answer.actions
    );
}
