use std::path::Path;

use domovoi_app::{RecommendationsModel, ShellFixture, TrayModel, strings};
use domovoi_rules::{ConfigOverrides, rule_table};

const STRINGS_SOURCE: &str = include_str!("../src/strings.rs");
const NO_BREAK_SPACE: char = '\u{A0}';
const GIGABYTE: u64 = 1_000_000_000;
const FORBIDDEN: [&str; 14] = [
    "Ура",
    "мусор",
    "смело",
    "Осторожно",
    "Опасно",
    "Ошибка доступа",
    "EPERM",
    "Неизвестное",
    "Готово",
    "летает",
    "Скачиваем",
    "Ой",
    " ой",
    "просто кэш",
];

fn literals(source: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut chars = source.chars();
    while let Some(character) = chars.next() {
        if character != '"' {
            continue;
        }
        let mut literal = String::new();
        while let Some(inner) = chars.next() {
            match inner {
                '\\' => {
                    if let Some(escaped) = chars.next() {
                        literal.push(escaped);
                    }
                }
                '"' => break,
                other => literal.push(other),
            }
        }
        found.push(literal);
    }
    found
}

fn spoken(text: &str) -> String {
    text.replace(NO_BREAK_SPACE, " ")
}

#[test]
fn interface_strings_and_rule_explanations_never_exclaim_or_joke() {
    let interface = literals(STRINGS_SOURCE);
    assert!(
        interface.len() > 100,
        "the strings module was read: {} literals",
        interface.len()
    );
    let explanations = rule_table(&ConfigOverrides::default())
        .into_iter()
        .map(|rule| rule.explanation.to_owned());
    for text in interface.into_iter().chain(explanations) {
        assert!(!text.contains('!'), "{text:?} exclaims");
        for phrase in FORBIDDEN {
            assert!(!text.contains(phrase), "{text:?} uses the forbidden phrase {phrase:?}");
        }
    }
}

#[test]
fn every_rule_of_the_table_has_a_title() {
    for rule in rule_table(&ConfigOverrides::default()) {
        assert!(strings::rule_title(rule.id).is_some(), "{} has no title", rule.id);
    }
}

#[test]
fn section_viii_phrases_are_how_the_interface_speaks() {
    let total = RecommendationsModel {
        safe: 87_800_000_000,
        recoverable: 26_100_000_000,
        check_yourself: 108_200_000_000,
        cards: Vec::new(),
    };
    assert_eq!(spoken(&total.total_line()), "Можно освободить 222,1 ГБ");
    assert_eq!(
        spoken(&TrayModel::moved_line(80_800_000_000)),
        "Перемещено в Корзину: 80,8 ГБ"
    );
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shell_state.json");
    let mut data = ShellFixture::read(&fixture)
        .and_then(|fixture| fixture.shell_data())
        .expect("the fixture reads");
    assert_eq!(data.hidden_notice(), None);
    data.hidden_folders = 3;
    assert_eq!(
        data.hidden_notice().map(|notice| spoken(&notice)).as_deref(),
        Some("macOS скрыла 3 папки. Дайте полный доступ к диску, чтобы учесть их")
    );
    assert_eq!(
        strings::SNAPSHOTS_RELEASE,
        "macOS удалит снапшоты сама, когда место понадобится"
    );
    let derived_data = rule_table(&ConfigOverrides::default())
        .into_iter()
        .find(|rule| rule.id == "xcode.derived-data")
        .expect("the DerivedData rule exists");
    assert!(derived_data.explanation.contains("Пересоздаются при следующей сборке"));
    assert!(total.total() == 222_100_000_000 && total.total() > 222 * GIGABYTE);
}
