use std::path::Path;

use domovoi_downloader::Catalog;
use domovoi_grammar::{ChatMessage, PromptFamily, ResponseSchema, system_prompt};

const QUESTION: &str = "Что занимает больше всего места?";
const EMPTY_THINKING: &str = "<think>\n\n</think>\n\n";

#[test]
fn every_catalog_entry_resolves_to_a_chat_ml_template_of_the_grammar_crate() {
    let test_catalog =
        Catalog::from_path(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/test/catalog.json"))
            .expect("the test catalog parses");
    let schema = ResponseSchema::answers_and_tool_calls();
    let system = system_prompt("Ты — Домовой.", &schema);
    for entry in Catalog::bundled().entries.iter().chain(&test_catalog.entries) {
        assert_eq!(entry.template.family, PromptFamily::ChatMl, "{}", entry.id);
        let prompt = entry.template.render(&system, &[ChatMessage::user(QUESTION)]);
        assert!(prompt.starts_with("<|im_start|>system\n"), "{}", entry.id);
        assert!(
            prompt.contains(&format!("<|im_start|>user\n{QUESTION}<|im_end|>\n")),
            "{}",
            entry.id
        );
        let answer_prefix = if entry.template.empty_think_before_answer {
            format!("<|im_start|>assistant\n{EMPTY_THINKING}")
        } else {
            "<|im_start|>assistant\n".to_owned()
        };
        assert!(prompt.ends_with(&answer_prefix), "{}", entry.id);
    }
}
