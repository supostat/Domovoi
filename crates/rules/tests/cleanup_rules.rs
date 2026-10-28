mod synthetic_tree;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use domovoi_index::{Enrichment, Index, NodeId, SafetyLevel};
use domovoi_rules::{
    CaseRule, ConfigOverrides, DenyReason, Denylist, Recommendations, RuleSet, rule_table, screen_capture_location,
};
use domovoi_testkit::TempTree;
use synthetic_tree::{NoSimulators, data_volume_index_with, file, merge, path_tree};

const MAIL_LIBRARY: &str = "/Users/igor/Library/Containers/com.apple.mail/Data/Library";

struct Built {
    index: Index,
    rules: RuleSet,
    denylist: Denylist,
    recommendations: Recommendations,
}

fn data_volume(files: &[&str], enrichment: &Enrichment) -> Index {
    let trees = files
        .iter()
        .map(|path| {
            let path = Path::new(path);
            path_tree(
                &path.parent().expect("a file has a folder").to_string_lossy(),
                file(&path.file_name().expect("a file has a name").to_string_lossy()),
            )
        })
        .collect();
    data_volume_index_with(merge(trees), enrichment)
}

fn rule_set(overrides: &ConfigOverrides) -> RuleSet {
    RuleSet::new(rule_table(overrides), CaseRule::Insensitive, Box::new(NoSimulators))
}

fn built(files: &[&str], enrichment: &Enrichment, overrides: &ConfigOverrides) -> Built {
    let index = data_volume(files, enrichment);
    let rules = rule_set(overrides);
    let denylist = Denylist::new(CaseRule::Insensitive).expect("the denylist is prepared");
    let recommendations = Recommendations::build(&index, &rules, &denylist, &BTreeSet::new(), Vec::new());
    Built {
        index,
        rules,
        denylist,
        recommendations,
    }
}

fn card_paths(built: &Built, id: &str) -> BTreeSet<PathBuf> {
    built
        .recommendations
        .cards
        .iter()
        .filter(|card| built.rules.rule(card.rule).id == id)
        .flat_map(|card| &card.items)
        .map(|item| built.index.path(item.node))
        .collect()
}

fn blocked_paths(built: &Built, id: &str) -> BTreeSet<PathBuf> {
    built
        .recommendations
        .cards
        .iter()
        .filter(|card| built.rules.rule(card.rule).id == id)
        .flat_map(|card| &card.blocked)
        .map(|(node, _)| built.index.path(*node))
        .collect()
}

fn claims(built: &Built) -> BTreeMap<PathBuf, &'static str> {
    built
        .rules
        .claims(&built.index)
        .into_iter()
        .map(|claim| (built.index.path(claim.node), built.rules.rule(claim.rule).id))
        .collect()
}

fn level(rules: &RuleSet, id: &str) -> SafetyLevel {
    rules
        .rules()
        .iter()
        .find(|rule| rule.id == id)
        .unwrap_or_else(|| panic!("the table has {id}"))
        .level
}

fn node_at(index: &Index, data_relative: &str) -> NodeId {
    Path::new(data_relative).components().fold(index.root(), |node, name| {
        index
            .child_named(node, name.as_os_str())
            .unwrap_or_else(|| panic!("{data_relative} is indexed"))
    })
}

fn paths(items: &[&str]) -> BTreeSet<PathBuf> {
    items.iter().map(PathBuf::from).collect()
}

#[test]
fn logs_go_to_their_cards_while_their_system_folders_stay() {
    let built = built(
        &[
            "/Users/igor/Library/Logs/DiagnosticReports/Tool-2026-10-28.ips",
            "/Users/igor/Library/Logs/DiagnosticReports/.WindowServer-2026-10-28.ips",
            "/Users/igor/Library/Logs/Homebrew/install.log",
            "/Library/Logs/DiagnosticReports/Kernel-2026-10-28.panic",
            "/Library/Logs/Other/system.log",
        ],
        &Enrichment::default(),
        &ConfigOverrides::default(),
    );

    assert_eq!(level(&built.rules, "logs.crash-reports"), SafetyLevel::Recoverable);
    assert_eq!(level(&built.rules, "logs.user"), SafetyLevel::Safe);
    assert_eq!(
        card_paths(&built, "logs.crash-reports"),
        paths(&[
            "/Library/Logs/DiagnosticReports/Kernel-2026-10-28.panic",
            "/Users/igor/Library/Logs/DiagnosticReports/.WindowServer-2026-10-28.ips",
            "/Users/igor/Library/Logs/DiagnosticReports/Tool-2026-10-28.ips",
        ])
    );
    assert_eq!(
        card_paths(&built, "logs.user"),
        paths(&["/Users/igor/Library/Logs/Homebrew"])
    );
    let claimed = claims(&built);
    for folder in [
        "/Users/igor/Library/Logs/DiagnosticReports",
        "/Library/Logs/DiagnosticReports",
        "/Library/Logs/Other",
        "/Library/Logs/Other/system.log",
    ] {
        assert!(!claimed.contains_key(Path::new(folder)), "{folder} stays unclaimed");
    }
}

#[test]
fn mail_attachments_open_while_the_rest_of_mail_stays_protected() {
    let attachment = format!("{MAIL_LIBRARY}/Mail Downloads/1A2B/invoice.pdf");
    let letter = format!("{MAIL_LIBRARY}/Mail/V10/letter.emlx");
    let built = built(
        &[&attachment, &letter],
        &Enrichment::default(),
        &ConfigOverrides::default(),
    );

    assert_eq!(level(&built.rules, "mail.downloads"), SafetyLevel::Recoverable);
    assert_eq!(
        card_paths(&built, "mail.downloads"),
        BTreeSet::from([PathBuf::from(format!("{MAIL_LIBRARY}/Mail Downloads/1A2B"))])
    );
    assert!(blocked_paths(&built, "mail.downloads").is_empty());
    let protected = |path: &str| {
        let node = built.index.find(Path::new(path)).expect("the path is indexed");
        built.denylist.verdict(&built.index, node)
    };
    assert_eq!(protected(&format!("{MAIL_LIBRARY}/Mail Downloads/1A2B")), None);
    assert!(matches!(
        protected(&format!("{MAIL_LIBRARY}/Mail/V10")),
        Some(DenyReason::ProtectedData(_))
    ));
    assert!(matches!(
        protected("/Users/igor/Library/Containers/com.apple.mail"),
        Some(DenyReason::ProtectedData(_))
    ));
}

#[test]
fn the_finder_trash_is_a_check_yourself_card_while_volume_trashes_stay_closed() {
    let built = built(
        &[
            "/Users/igor/.Trash/old-project/main.rs",
            "/Users/igor/.Trash/archive.zip",
            "/.Trashes/501/old.bin",
        ],
        &Enrichment::default(),
        &ConfigOverrides::default(),
    );

    assert_eq!(level(&built.rules, "trash.user"), SafetyLevel::CheckYourself);
    assert_eq!(
        card_paths(&built, "trash.user"),
        paths(&["/Users/igor/.Trash/archive.zip", "/Users/igor/.Trash/old-project"])
    );
    assert!(blocked_paths(&built, "trash.user").is_empty());
    assert!(!claims(&built).contains_key(Path::new("/Users/igor/.Trash")));
    assert!(matches!(
        built
            .denylist
            .verdict(&built.index, node_at(&built.index, ".Trashes/501/old.bin")),
        Some(DenyReason::ProtectedData(_))
    ));
}

#[test]
fn a_screen_capture_is_found_by_its_spotlight_badge_only() {
    let badged = [
        "/Users/igor/Desktop/Shot A.png",
        "/Users/igor/Pictures/Screenshots/Shot B.png",
        "/Users/igor/Library/Application Support/CleanShot/media/Shot C.png",
        "/Users/igor/Pictures/Holiday/Shot D.png",
    ];
    let look_alike = "/Users/igor/Desktop/Снимок экрана 2026-10-28 в 10.24.37.png";
    let enrichment = Enrichment {
        screen_captures: badged.iter().map(PathBuf::from).collect(),
        ..Enrichment::default()
    };
    let files: Vec<&str> = badged.iter().copied().chain([look_alike]).collect();
    let overrides =
        ConfigOverrides::default().with_screenshot_location(Some(PathBuf::from("/Users/igor/Pictures/Screenshots")));
    let built = built(&files, &enrichment, &overrides);

    assert_eq!(level(&built.rules, "screenshots"), SafetyLevel::CheckYourself);
    assert_eq!(
        card_paths(&built, "screenshots"),
        paths(&[
            "/Users/igor/Desktop/Shot A.png",
            "/Users/igor/Pictures/Screenshots/Shot B.png"
        ]),
        "a capture inside program data or a nested folder stays, a look-alike name without the badge stays"
    );
}

#[test]
fn the_screenshot_folder_comes_from_the_screen_capture_defaults() {
    let home = TempTree::new().expect("a temp home is created");
    let location = screen_capture_location(home.path());
    assert!(location.is_none_or(|folder| folder.is_absolute()));
}
