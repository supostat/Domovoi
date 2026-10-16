use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use domovoi_grammar::{ChatMessage, ChatTemplate};
use serde::Deserialize;

const DIALOG_MARKER_START: &str = "<<<dialog ";
const DIALOG_MARKER_END: &str = ">>>\n";
const GOLDEN_SUFFIX: &str = ".golden.txt";

#[derive(Deserialize)]
struct Catalog {
    entries: Vec<CatalogEntry>,
}

#[derive(Deserialize)]
struct CatalogEntry {
    repo: String,
    revision: String,
    template: ChatTemplate,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Dialogs {
    dialogs: Vec<Dialog>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Dialog {
    name: String,
    messages: Vec<DialogMessage>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DialogMessage {
    role: String,
    content: String,
}

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn templates_directory() -> PathBuf {
    assets().join("chat-templates")
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> T {
    serde_json::from_str(&fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display())))
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn rendered(template: ChatTemplate, dialogs: &[Dialog]) -> String {
    let mut text = String::new();
    for dialog in dialogs {
        let (system, rest) = dialog
            .messages
            .split_first()
            .expect("a dialog starts with a system message");
        assert_eq!(system.role, "system", "{}", dialog.name);
        let messages: Vec<ChatMessage> = rest
            .iter()
            .map(|message| match message.role.as_str() {
                "user" => ChatMessage::user(&message.content),
                "assistant" => ChatMessage::assistant(&message.content),
                "tool" => ChatMessage::tool(&message.content),
                other => panic!("{}: unexpected role {other}", dialog.name),
            })
            .collect();
        text.push_str(DIALOG_MARKER_START);
        text.push_str(&dialog.name);
        text.push_str(DIALOG_MARKER_END);
        text.push_str(&template.render(&system.content, &messages));
    }
    text
}

fn golden_files(directory: &Path) -> BTreeSet<PathBuf> {
    let mut found = BTreeSet::new();
    for entry in fs::read_dir(directory).expect("the templates directory lists") {
        let path = entry.expect("a directory entry reads").path();
        if path.is_dir() {
            found.extend(golden_files(&path));
        } else if path.to_string_lossy().ends_with(GOLDEN_SUFFIX) {
            found.insert(path);
        }
    }
    found
}

#[test]
fn rust_template_matches_the_golden_prompt_of_every_catalog_entry() {
    let catalog: Catalog = read_json(&assets().join("catalog.json"));
    let dialogs: Dialogs = read_json(&templates_directory().join("dialogs.json"));
    assert!(!catalog.entries.is_empty());
    let mut expected_goldens = BTreeSet::new();
    for entry in &catalog.entries {
        let golden = templates_directory().join(format!("{}@{}{GOLDEN_SUFFIX}", entry.repo, entry.revision));
        let expected = fs::read_to_string(&golden).unwrap_or_else(|error| {
            panic!(
                "{}: {error}; regenerate golden prompts with research/render_templates.py",
                golden.display()
            )
        });
        assert!(
            rendered(entry.template, &dialogs.dialogs) == expected,
            "{}@{} differs from {}",
            entry.repo,
            entry.revision,
            golden.display()
        );
        expected_goldens.insert(golden);
    }
    assert_eq!(
        golden_files(&templates_directory()),
        expected_goldens,
        "golden prompts must belong to catalog revisions"
    );
}
