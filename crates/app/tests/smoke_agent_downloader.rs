use std::fs::{self, File};
use std::path::{Path, PathBuf};

use domovoi_app::{AssistantSettings, ChatEntry, ChatModel, ModelCardState, ModelLibrary, Session, SessionPaths};
use domovoi_downloader::{Catalog, CatalogEntry, ModelSource, ModelStore};
use domovoi_index::{Category, ModelEvent};
use domovoi_inference::{CancelFlag, FakeBackend, InferenceBackend};
use domovoi_scanner::CancelToken;
use domovoi_testkit::{ImageVolume, TempTree, Visibility, VolumeFormat};
use serde_json::json;

const MEGABYTE: usize = 1024 * 1024;
const BUILD_OUTPUT: usize = 2 * MEGABYTE;
const MODELS: &str = "Models";
const QUESTION: &str = "Что из разработки можно убрать?";
const ANSWER: &str = "Результаты сборки Rust пересоздаются, их можно убрать.";
const CONTEXT_FOR_BYTE_TOKENS: u32 = 16_384;

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn noise(size: usize, seed: u8) -> Vec<u8> {
    (0..size)
        .map(|position| (position as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

fn populate(root: &Path) {
    let project = root.join("Projects/engine");
    fs::create_dir_all(project.join("target/debug")).expect("the project tree is created");
    fs::write(project.join("Cargo.toml"), "[package]\nname = \"engine\"\n").expect("the manifest is written");
    fs::write(project.join("target/debug/engine"), noise(BUILD_OUTPUT, 5)).expect("the build output is written");
}

fn say(response: &serde_json::Value) -> String {
    format!("token {response}")
}

fn chat_over_fake_model(session: &Session, entry: &CatalogEntry, model_path: &Path) -> ChatModel {
    let mut backend = FakeBackend::replying([
        say(&json!({"kind": "tool_call", "name": "query_index", "arguments": {"level": "safe", "limit": 10}})),
        say(&json!({"kind": "final", "answer": ANSWER, "hypotheses": []})),
    ]);
    backend
        .load(
            File::open(model_path).expect("the installed model opens").into(),
            entry.class,
            entry.context_length,
            &CancelFlag::default(),
        )
        .expect("the installed weights load");
    let byte_tokenized = CatalogEntry {
        context_length: CONTEXT_FOR_BYTE_TOKENS,
        ..entry.clone()
    };
    let mut chat = ChatModel::default();
    chat.ask(QUESTION);
    let answer = session
        .ask(
            &mut backend,
            &byte_tokenized,
            QUESTION,
            &CancelFlag::default(),
            &mut |step| chat.record_step(step),
        )
        .expect("the agent answers through the session");
    chat.record_answer(&answer);
    chat
}

fn assert_chat_shows_the_call_and_the_action(chat: &ChatModel) {
    match &chat.entries[1] {
        ChatEntry::ToolCall { tool, arguments, found } => {
            assert_eq!(tool, "query_index");
            assert!(arguments.contains("level: \"safe\""), "{arguments}");
            assert!(found.is_some_and(|count| count > 0), "the call found nodes: {found:?}");
        }
        other => panic!("the tool call is visible in the chat, found {other:?}"),
    }
    let answer_entry = chat.entries.last().expect("the answer is recorded");
    match answer_entry {
        ChatEntry::Answer { text, items, targets } => {
            assert_eq!(text, ANSWER);
            assert!(
                items.iter().any(|item| item.title == "Каталоги target/ (Rust)"),
                "{items:?}"
            );
            assert!(
                targets
                    .iter()
                    .any(|target| target.path.ends_with("Projects/engine/target"))
            );
        }
        other => panic!("the answer closes the chat, found {other:?}"),
    }
    assert!(
        answer_entry.answer_total() >= BUILD_OUTPUT as u64,
        "the action button sums the build output"
    );
}

#[test]
fn a_session_runs_the_real_agent_over_a_model_installed_from_the_real_catalog() {
    let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("the image attaches");
    let mount_point = fs::canonicalize(volume.mount_point()).expect("the mount point resolves");
    populate(&mount_point);
    let catalog = Catalog::from_path(&assets().join("test/catalog.json")).expect("the test catalog parses");
    let entry = catalog.entries[0].clone();
    let store = ModelStore::open(&mount_point.join(MODELS)).expect("the model store opens");
    store
        .adopt(&assets().join("test/tiny-random.gguf"), &entry)
        .expect("a file matching the catalog is adopted");

    let state = TempTree::new().expect("a temp tree is created");
    let mut session = Session::open(&mount_point, &SessionPaths::in_directory(state.path()), "0.0.0-test")
        .expect("the session opens");
    session.scan(None, CancelToken::default()).expect("the volume scans");
    let mut library = ModelLibrary::new(store.clone(), catalog, ModelSource::hugging_face());
    let installed = library.finish_scan().expect("the model folder scans");
    let model_path = store.model_path(&entry);
    assert_eq!(
        installed,
        [ModelEvent::Downloaded {
            path: model_path.clone()
        }]
    );
    for event in &installed {
        session.apply_model_event(event).expect("the event reaches the index");
    }
    let category_of_model = |session: &Session| {
        let index = session.index().expect("the volume is scanned");
        index
            .node(index.find(&model_path).expect("the model is indexed"))
            .category
    };
    assert_eq!(category_of_model(&session), Category::Model);

    let settings = AssistantSettings {
        enabled: true,
        active_entry: Some(entry.id.clone()),
        ..AssistantSettings::default()
    };
    let screen = library.screen_model(&settings);
    assert_eq!(screen.cards.len(), 1);
    assert_eq!(screen.cards[0].state, ModelCardState::Active);
    assert_eq!(
        screen.active().map(|card| card.entry_id.as_str()),
        Some(entry.id.as_str())
    );

    let chat = chat_over_fake_model(&session, &entry, &model_path);
    assert_chat_shows_the_call_and_the_action(&chat);

    let removed = library
        .remove(0)
        .expect("the installed model goes to the Trash")
        .expect("an installed model is removed");
    assert_eq!(
        removed,
        ModelEvent::Removed {
            path: model_path.clone()
        }
    );
    session
        .apply_model_event(&removed)
        .expect("the removal reaches the index");
    assert_eq!(
        library.screen_model(&settings).cards[0].state,
        ModelCardState::Available
    );
    assert!(!model_path.exists());
}
