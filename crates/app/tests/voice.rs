use std::path::Path;
use std::time::Duration;

use domovoi_app::{
    DeleteOutcome, DeleteStatus, DropRefusal, DropVerdict, MeasureProgress, RecommendationsModel, Screen, ShellFixture,
    TrayModel, TraySummary, scan_finished_line, strings,
};
use domovoi_rules::{ConfigOverrides, DenyReason, rule_table};

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
fn cleanup_cards_name_what_the_person_sees_in_finder() {
    let titles = [
        ("logs.crash-reports", "Отчёты о сбоях"),
        ("logs.user", "Журналы программ"),
        ("mail.downloads", "Вложения из Почты"),
        ("trash.user", "Корзина"),
        ("screenshots", "Снимки экрана"),
    ];
    for (rule, title) in titles {
        assert_eq!(strings::rule_title(rule), Some(title), "{rule}");
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
        files_done: 1_200,
        files_total: 3_400,
    });
    assert_eq!(
        deleting.deleting_line().map(|line| spoken(&line)).as_deref(),
        Some("Удаляю: 1 200 из 3 400 файлов")
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
    data.hidden_folders = 0;
    data.other_user_homes = 2;
    assert_eq!(
        data.hidden_notice().map(|notice| spoken(&notice)).as_deref(),
        Some("2 домашние папки других пользователей: их размер виден только администратору")
    );
    assert!(strings::PURGEABLE_TIME_MACHINE.contains("Time Machine"));
    assert_eq!(
        strings::TMUTIL_COPIED,
        "Команда tmutil скопирована. Вставьте её в Terminal, чтобы удалить локальные снапшоты"
    );
    assert!(!strings::COPY_TMUTIL.contains('!'));
    assert!(!strings::UNACCOUNTED_HINT.contains('!'));
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

#[test]
fn the_collection_list_and_the_card_question_speak_plainly() {
    assert_eq!(strings::TRAY, "К удалению");
    assert_eq!(strings::ASK_DOMOVOI, "Спросить Домового");
    for mark in ['!', '/', '~'] {
        assert!(
            !strings::CARD_QUESTION.contains(mark),
            "the card question is a fixed text without paths: {mark}"
        );
    }
    assert_eq!(strings::TO_TRAY, "Отметить к удалению");
    assert_eq!(
        strings::TRAY_EMPTY,
        "Пока ничего не отмечено к удалению. Отметьте рекомендации или копии дубликатов, и они соберутся здесь."
    );
    assert_eq!(
        strings::TRAY_NOTE,
        "Удаление необратимо: файлы не попадают в Корзину macOS. Перед стартом Домовой спросит ещё раз."
    );
}

#[test]
fn releasing_space_speaks_plainly_and_only_the_menu_item_opens_a_sheet() {
    assert!(strings::PURGE_SPACE.ends_with('…'), "the menu item opens a sheet");
    assert!(
        !strings::PURGE_RELEASE.contains('…'),
        "the sheet button starts the work"
    );
    assert_eq!(strings::PURGE_RELEASE, "Освободить");
    assert_eq!(strings::PURGE_TITLE, "Освобождение");
    assert_eq!(strings::PURGING, "Освобождаю:");
    assert_eq!(strings::PURGED, "Освобождено:");
    assert!(strings::PURGE_GOES_TIME_MACHINE.contains("Time Machine"));
    assert!(!strings::PURGE_GOES_PLAIN.contains("Time Machine"));
    let failed = strings::purge_failed_line("CoreServices refused the recovery with code 5");
    let texts = [
        strings::PURGE_SPACE,
        strings::PURGE_TITLE,
        strings::PURGE_RELEASE,
        strings::PURGE_ESTIMATE,
        strings::PURGE_GOES_TIME_MACHINE,
        strings::PURGE_GOES_PLAIN,
        strings::PURGING,
        strings::PURGED,
        strings::PURGE_STOPPED,
        strings::PURGE_TIMED_OUT,
        strings::PURGE_STALLED,
        failed.as_str(),
    ];
    for text in texts {
        assert!(!text.contains('!'), "{text:?} exclaims");
        for phrase in FORBIDDEN {
            assert!(!text.contains(phrase), "{text:?} uses the forbidden phrase {phrase:?}");
        }
    }
}

#[test]
fn application_removal_states_its_reasons_plainly() {
    let phrases = [
        strings::REMOVE_WITH_FILES,
        strings::WITH_ITS_FILES,
        strings::NOT_MARKED_NAME_ONLY,
        strings::OWNER,
        strings::OWNER_REMOVED,
        strings::REMOVAL_BUNDLE,
        strings::OWNER_BUNDLE_ID_NAME,
        strings::OWNER_CONTAINER_METADATA,
        strings::OWNER_APP_GROUP,
        strings::OWNER_TEAM_PREFIX,
        strings::OWNER_LAUNCHD,
        strings::OWNER_NAME_ONLY,
    ];
    for phrase in phrases {
        assert!(!phrase.contains('!'), "{phrase} raises its voice");
        assert!(
            !phrase.to_lowercase().contains("похоже"),
            "{phrase} hedges like a hypothesis"
        );
    }
    assert_eq!(strings::REMOVE_WITH_FILES, "Удалить вместе с файлами");
    assert!(strings::OWNER_NAME_ONLY.ends_with("проверьте сами"));
    assert_eq!(
        strings::rule_title("apps.leftover-data"),
        Some("Данные удалённых программ")
    );
}

#[test]
fn dropping_rows_on_the_collection_button_speaks_plainly() {
    assert_eq!(strings::DROP_ADD, "Добавить");
    assert_eq!(strings::DROP_DENIED, "Нельзя:");
    assert_eq!(strings::DROP_ALREADY, "Уже в списке");
    assert_eq!(strings::NOT_ADDED, "Не добавлено:");
    let refused = DropVerdict {
        refused: vec![
            DropRefusal {
                name: "System".to_owned(),
                reason: DenyReason::SystemVolume,
            },
            DropRefusal {
                name: "usr".to_owned(),
                reason: DenyReason::SystemVolume,
            },
        ],
        ..DropVerdict::default()
    };
    assert_eq!(refused.hover().title, "Нельзя: системный том");
    assert_eq!(
        refused.refusal_line().as_deref(),
        Some("Не добавлено: System — системный том · usr — системный том")
    );
    let collected = TraySummary {
        objects: 3,
        bytes: 42_200_000_000,
    };
    assert_eq!(spoken(&collected.title()), "К удалению · 3 · ≈ 42,2 ГБ");
    assert_eq!(TraySummary::default().title(), "К удалению");
}
