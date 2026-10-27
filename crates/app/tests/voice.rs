use std::path::Path;
use std::time::Duration;

use domovoi_app::{
    DeleteOutcome, DeleteStatus, MeasureProgress, RecommendationsModel, Screen, ShellFixture, TrayModel,
    scan_finished_line, strings,
};
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
    let outcome = DeleteOutcome {
        objects: 12,
        freed: 80_800_000_000,
        failures: Vec::new(),
    };
    assert_eq!(spoken(&outcome.line()), "Удалено: 12 объектов, освобождено 80,8 ГБ");
    let deleting = TrayModel::default().with_deleting(DeleteStatus {
        done: 1_200,
        total: 3_400,
    });
    assert_eq!(
        deleting.deleting_line().map(|line| spoken(&line)).as_deref(),
        Some("Удаляю: 1 200 из 3 400 элементов")
    );
    let measuring = TrayModel::default().with_progress(MeasureProgress {
        files: 182_340,
        bytes: 41_800_000_000,
    });
    assert_eq!(
        measuring.measuring_line().map(|line| spoken(&line)).as_deref(),
        Some("Уточняю размер: 182 340 файлов")
    );
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shell_state.json");
    let mut data = ShellFixture::read(&fixture)
        .and_then(|fixture| fixture.shell_data(Screen::Main))
        .expect("the fixture reads");
    assert_eq!(data.hidden_notice(), None);
    data.hidden_folders = 3;
    assert_eq!(
        data.hidden_notice().map(|notice| spoken(&notice)).as_deref(),
        Some("macOS скрыла 3 папки. Дайте полный доступ к диску, чтобы учесть их")
    );
    assert_eq!(
        spoken(&scan_finished_line(Duration::from_secs(130))),
        "Скан завершён за 2 мин 10 с"
    );
    assert_eq!(
        spoken(&scan_finished_line(Duration::from_secs(48))),
        "Скан завершён за 48 с"
    );
    assert_eq!(strings::UNRESPONSIVE_TITLE, "Файловая система не отвечает");
    assert!(
        strings::unresponsive_body("/Volumes/Backup").ends_with("Последняя папка: /Volumes/Backup"),
        "the stall notice names the folder it waited on"
    );
    assert!(
        strings::ACCESS_STEPS[1].concat().contains("перетащите иконку"),
        "the access steps offer dragging the icon into the list"
    );
    assert!(strings::ACCESS_DRAG_HINT.contains("перетащить"));
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
