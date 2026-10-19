use std::path::PathBuf;

use domovoi_rules::{ConfigOverrides, Rule, rule_table};

const PNPM_STORE: &str = "cache.pnpm-store";
const CARGO_TARGET: &str = "cargo.target";
const CUSTOM_STORE: &str = "/Volumes/Work/pnpm-store";

fn rule<'table>(table: &'table [Rule], id: &str) -> &'table Rule {
    table
        .iter()
        .find(|rule| rule.id == id)
        .unwrap_or_else(|| panic!("the table has {id}"))
}

#[test]
fn a_rule_tells_its_paths_as_the_table_writes_them() {
    let table = rule_table(&ConfigOverrides::default());
    assert_eq!(rule(&table, PNPM_STORE).paths(), ["~/Library/pnpm/store"]);
    assert_eq!(rule(&table, CARGO_TARGET).paths(), ["**/target|Cargo.toml"]);
    assert!(
        table.iter().all(|rule| !rule.paths().is_empty()),
        "every rule names where it applies"
    );
}

#[test]
fn an_override_adds_its_absolute_path_after_the_standard_one() {
    let table = rule_table(&ConfigOverrides {
        pnpm_store: Some(PathBuf::from(CUSTOM_STORE)),
        cargo_target: None,
    });
    assert_eq!(rule(&table, PNPM_STORE).paths(), ["~/Library/pnpm/store", CUSTOM_STORE]);
}
