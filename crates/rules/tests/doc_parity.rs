use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use domovoi_index::SafetyLevel;
use domovoi_rules::{ConfigOverrides, rule_table};

const DOCUMENT: &str = "../../docs/DOMOVOI.md";
const SECTION: &str = "### 4.4";
const STARTING_IDS: usize = 10;

fn level_of(word: &str) -> SafetyLevel {
    match word {
        "Безопасно" => SafetyLevel::Safe,
        "Восстановимо" => SafetyLevel::Recoverable,
        "Проверьте сами" => SafetyLevel::CheckYourself,
        other => panic!("unknown level in the document: {other}"),
    }
}

fn documented_rules() -> BTreeMap<String, SafetyLevel> {
    let document = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(DOCUMENT)).expect("document reads");
    let section = document.split_once(SECTION).expect("section 4.4 exists").1;
    let section = section.split("\n### ").next().expect("section has a body");
    section
        .lines()
        .filter(|line| line.starts_with("| `"))
        .map(|line| {
            let columns: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
            let id = columns[0].trim_matches('`').to_owned();
            (id, level_of(columns.last().expect("row has a level")))
        })
        .collect()
}

#[test]
fn starting_rules_match_the_document_by_id_and_level() {
    let documented = documented_rules();
    assert_eq!(documented.len(), STARTING_IDS);
    let table = rule_table(&ConfigOverrides::default());
    for (id, level) in &documented {
        let rule = table
            .iter()
            .find(|rule| rule.id == id)
            .unwrap_or_else(|| panic!("{id} is missing"));
        assert_eq!(rule.level, *level, "{id}");
    }
}
