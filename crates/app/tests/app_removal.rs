use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use domovoi_app::{IndexFacts, Provenance, RecommendationsModel, Session, SessionPaths, TrayModel, strings};
use domovoi_index::{Index, NodeId, OwnerEvidence};
use domovoi_rules::{DenyReason, RemovalReason};
use domovoi_scanner::CancelToken;
use domovoi_testkit::{
    ImageVolume, PropertyValue, TempTree, Visibility, VolumeFormat, app_container, application_bundle,
    write_property_list,
};

const LIBRARY: &str = "Users/tester/Library";
const CHFLAGS: &str = "/usr/bin/chflags";
const LAUNCH_AGENT: &str = "LaunchAgents/com.domovoi.tiny.helper.plist";

fn library(root: &Path, relative: &str) -> PathBuf {
    root.join(LIBRARY).join(relative)
}

fn node(index: &Index, path: &Path) -> NodeId {
    index
        .find(path)
        .unwrap_or_else(|| panic!("{} is indexed", path.display()))
}

fn populate(root: &Path) {
    application_bundle(
        &root.join("Applications"),
        "TinyApp",
        "com.domovoi.tiny",
        &["group.com.domovoi.tiny"],
    )
    .expect("the application is built");
    app_container(&library(root, "Containers/com.domovoi.tiny"), "com.domovoi.tiny").expect("a container");
    app_container(&library(root, "Containers/com.example.renamed"), "com.domovoi.gone").expect("a leftover");
    let group = library(root, "Group Containers/group.com.domovoi.tiny");
    fs::create_dir_all(&group).expect("a group container is created");
    fs::write(group.join("shared.db"), vec![2_u8; 4_096]).expect("group data is written");
    let name_only = library(root, "Application Support/TinyApp");
    fs::create_dir_all(&name_only).expect("a folder named like the app is created");
    fs::write(name_only.join("state.json"), b"{}").expect("a file is written");
    let agent = library(root, LAUNCH_AGENT);
    write_property_list(
        &agent,
        &[
            ("Label", PropertyValue::Text("com.domovoi.tiny.helper")),
            ("AssociatedBundleIdentifiers", PropertyValue::Text("com.domovoi.tiny")),
        ],
    )
    .expect("a launch agent is written");
    let locked = Command::new(CHFLAGS)
        .arg("uchg")
        .arg(&agent)
        .status()
        .expect("chflags runs");
    assert!(locked.success(), "the launch agent is locked");
}

fn leftover_card_names_its_reasons(session: &Session, now: SystemTime) {
    let index = session.index().expect("the volume is indexed");
    let model = RecommendationsModel::build(
        index,
        session.rules(),
        session.recommendations().expect("recommendations are built"),
        now,
    );
    let leftover = model
        .cards
        .iter()
        .find(|card| card.provenance == Provenance::Rule("apps.leftover-data".to_owned()))
        .expect("the removed application leaves a card");
    assert_eq!(leftover.paths.len(), leftover.path_reasons.len());
    assert!(
        leftover
            .reasons_line()
            .is_some_and(|line| line.contains(strings::OWNER_CONTAINER_METADATA)),
        "the leftover card names why its folders belong to a removed application"
    );
}

#[test]
fn removing_an_application_takes_its_files_with_reasons_and_leaves_a_name_match_unmarked() {
    let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("the image attaches");
    let root = fs::canonicalize(volume.mount_point()).expect("the mount point resolves");
    populate(&root);
    let state = TempTree::new().expect("a state folder is created");
    let mut session =
        Session::open(&root, &SessionPaths::in_directory(state.path()), "0.0.0-test").expect("the session opens");
    session.scan(None, CancelToken::default()).expect("the volume scans");
    let now = SystemTime::now();

    let index = session.index().expect("the volume is indexed");
    let bundle = node(index, &root.join("Applications/TinyApp.app"));
    let container = node(index, &library(&root, "Containers/com.domovoi.tiny"));
    assert!(IndexFacts::of(index, bundle, now).removable_application);
    assert!(!IndexFacts::of(index, container, now).removable_application);
    assert_eq!(
        IndexFacts::of(index, container, now).owner.as_deref(),
        Some(format!("{} TinyApp · {}", strings::OWNER, strings::OWNER_CONTAINER_METADATA).as_str())
    );
    leftover_card_names_its_reasons(&session, now);

    assert!(session.add_application_to_tray(bundle));
    let index = session.index().expect("the volume is indexed");
    let taken: BTreeMap<PathBuf, Option<RemovalReason>> = session
        .tray()
        .items()
        .iter()
        .map(|item| (item.path.clone(), item.reason))
        .collect();
    assert_eq!(
        taken,
        BTreeMap::from([
            (root.join("Applications/TinyApp.app"), Some(RemovalReason::Bundle)),
            (
                library(&root, "Containers/com.domovoi.tiny"),
                Some(RemovalReason::Owner(OwnerEvidence::ContainerMetadata))
            ),
            (
                library(&root, "Group Containers/group.com.domovoi.tiny"),
                Some(RemovalReason::Owner(OwnerEvidence::AppGroup))
            ),
            (
                library(&root, LAUNCH_AGENT),
                Some(RemovalReason::Owner(OwnerEvidence::LaunchdAssociation))
            ),
        ])
    );
    let applications = session.tray().applications();
    assert_eq!(applications.len(), 1);
    assert_eq!(
        applications[0].name_only,
        [library(&root, "Application Support/TinyApp")],
        "a folder that only shares the name stays unmarked"
    );

    let sizes = TrayModel::estimated_sizes(index, session.tray());
    let sheet = TrayModel::measured(index, session.tray(), &sizes);
    let group = sheet
        .rows
        .iter()
        .find(|row| row.title == format!("TinyApp {}", strings::WITH_ITS_FILES))
        .expect("the sheet shows the application as one group");
    assert_eq!(group.details.len(), 4);
    assert!(
        group
            .details
            .iter()
            .any(|detail| detail.ends_with(strings::REMOVAL_BUNDLE))
    );
    assert!(
        group
            .details
            .iter()
            .any(|detail| detail.ends_with(strings::OWNER_APP_GROUP))
    );
    assert_eq!(group.unmarked.len(), 1);

    let (subjects, refusals) = session
        .take_tray_for_delete(&sizes.items)
        .expect("running programs are checked");
    assert_eq!(refusals.len(), 1, "{refusals:?}");
    assert_eq!(refusals[0].path, library(&root, LAUNCH_AGENT));
    assert!(matches!(refusals[0].reason, DenyReason::ProtectedFlag(_)));
    assert_eq!(subjects.len(), 3);
    assert!(
        subjects
            .iter()
            .all(|subject| !subject.path.starts_with(library(&root, "Application Support/TinyApp")))
    );
}
