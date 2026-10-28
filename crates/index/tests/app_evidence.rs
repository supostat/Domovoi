mod app_fixture;

use std::fs;
use std::path::Path;

use app_fixture::{Value, application, container, volume_index, write_property_list};
use domovoi_index::{Category, Enrichment, Index, IndexRoot, Installation, NodeId, OwnerEvidence};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{ImageVolume, TempTree, Visibility, VolumeFormat};

const LIBRARY: &str = "Users/tester/Library";

fn at(index: &Index, relative: &str) -> NodeId {
    Path::new(relative).components().fold(index.root(), |node, name| {
        index
            .child_named(node, name.as_os_str())
            .unwrap_or_else(|| panic!("{relative} is indexed"))
    })
}

fn owner(index: &Index, relative: &str) -> Option<(String, Installation, OwnerEvidence)> {
    let ownership = index.ownership(at(index, relative))?;
    let app = index
        .app(ownership.app)
        .expect("an ownership names a known application");
    Some((app.bundle_id.clone(), app.installation.clone(), ownership.evidence))
}

fn installed(index: &Index, relative: &str) -> Installation {
    Installation::Installed {
        bundle_path: index.path(at(index, relative)).to_string_lossy().into_owned(),
    }
}

fn library(root: &Path, relative: &str) -> std::path::PathBuf {
    root.join(LIBRARY).join(relative)
}

#[test]
fn exact_signals_bind_the_files_of_an_installed_application_to_its_bundle() {
    let tree = TempTree::new().expect("a temp tree is created");
    let root = tree.path();
    let tiny = application(
        &root.join("Applications"),
        "Tiny",
        "com.domovoi.tiny",
        &["group.com.domovoi.tiny"],
    );
    application(&root.join("Users/tester/Downloads"), "Loose", "com.domovoi.loose", &[]);
    container(&library(root, "Containers/com.domovoi.tiny"), "com.domovoi.tiny");
    container(&library(root, "Containers/Renamed"), "com.domovoi.tiny");
    fs::create_dir_all(library(root, "Containers/com.domovoi.loose")).expect("a container without metadata");
    fs::create_dir_all(library(root, "Group Containers/group.com.domovoi.tiny")).expect("a group container");
    write_property_list(
        &library(root, "LaunchAgents/com.domovoi.tiny.helper.plist"),
        &[
            ("Label", Value::Text("com.domovoi.tiny.helper")),
            ("AssociatedBundleIdentifiers", Value::Texts(&["com.domovoi.tiny"])),
        ],
    );
    let program = fs::canonicalize(&tiny)
        .expect("the bundle resolves")
        .join("Contents/MacOS/Tiny");
    write_property_list(
        &library(root, "LaunchAgents/agent.plist"),
        &[
            ("Label", Value::Text("agent")),
            ("Program", Value::Text(&program.to_string_lossy())),
        ],
    );
    fs::create_dir_all(library(root, "Application Support/Tiny")).expect("a folder named like the app");
    fs::create_dir_all(library(root, "Application Support/Gone")).expect("a folder named like no app");
    let index = volume_index(root);

    let bundle = index
        .bundles()
        .iter()
        .find(|bundle| bundle.bundle_id == "com.domovoi.tiny")
        .expect("the bundle is discovered in the index");
    assert_eq!(bundle.name, "Tiny");
    assert_eq!(bundle.team_id, None, "an ad-hoc signature carries no Team ID");
    assert_eq!(bundle.app_groups, ["group.com.domovoi.tiny"]);
    let tiny_installed = installed(&index, "Applications/Tiny.app");
    let tiny_owner = |evidence: OwnerEvidence| Some(("com.domovoi.tiny".to_owned(), tiny_installed.clone(), evidence));

    let tiny_container = format!("{LIBRARY}/Containers/com.domovoi.tiny");
    assert_eq!(
        owner(&index, &tiny_container),
        tiny_owner(OwnerEvidence::ContainerMetadata)
    );
    assert_eq!(
        index
            .ownership(at(&index, &format!("{tiny_container}/data.db")))
            .map(|ownership| ownership.root),
        Some(at(&index, &tiny_container)),
        "the files of a container inherit its owner"
    );
    assert_eq!(
        owner(&index, &format!("{LIBRARY}/Containers/Renamed")),
        tiny_owner(OwnerEvidence::ContainerMetadata),
        "the metadata wins over the folder name"
    );
    assert_eq!(
        owner(&index, &format!("{LIBRARY}/Containers/com.domovoi.loose")),
        Some((
            "com.domovoi.loose".to_owned(),
            installed(&index, "Users/tester/Downloads/Loose.app"),
            OwnerEvidence::BundleIdName
        )),
        "a bundle outside Applications still counts as installed"
    );
    assert_eq!(
        owner(&index, &format!("{LIBRARY}/Group Containers/group.com.domovoi.tiny")),
        tiny_owner(OwnerEvidence::AppGroup)
    );
    assert_eq!(
        owner(&index, &format!("{LIBRARY}/LaunchAgents/com.domovoi.tiny.helper.plist")),
        tiny_owner(OwnerEvidence::LaunchdAssociation)
    );
    assert_eq!(
        owner(&index, &format!("{LIBRARY}/LaunchAgents/agent.plist")),
        tiny_owner(OwnerEvidence::LaunchdAssociation),
        "the program inside the bundle names the owner"
    );
    assert_eq!(
        owner(&index, &format!("{LIBRARY}/Application Support/Tiny")),
        tiny_owner(OwnerEvidence::NameOnly)
    );
    assert_eq!(owner(&index, &format!("{LIBRARY}/Application Support/Gone")), None);
}

#[test]
fn removed_applications_leave_owners_only_by_exact_signals() {
    let tree = TempTree::new().expect("a temp tree is created");
    let root = tree.path();
    let applications = root.join("Applications");
    application(&applications, "Tiny", "com.domovoi.tiny", &["group.com.domovoi.shared"]);
    application(
        &applications,
        "Other",
        "com.domovoi.other",
        &["group.com.domovoi.shared"],
    );
    container(&library(root, "Containers/com.example.renamed"), "com.domovoi.gone");
    fs::create_dir_all(library(root, "Group Containers/ZYXWV98765.com.domovoi.gone"))
        .expect("a group of a removed team");
    fs::create_dir_all(library(root, "Group Containers/group.com.domovoi.shared")).expect("a shared group");
    fs::create_dir_all(library(root, "Group Containers/group.com.apple.notes")).expect("a system group");
    write_property_list(
        &library(root, "LaunchAgents/com.domovoi.gone.helper.plist"),
        &[
            ("Label", Value::Text("com.domovoi.gone.helper")),
            ("Program", Value::Text("/Applications/Gone.app/Contents/MacOS/helper")),
        ],
    );
    fs::create_dir_all(library(root, "Application Support/Gone")).expect("a folder named like the removed app");
    let index = volume_index(root);

    assert_eq!(
        owner(&index, &format!("{LIBRARY}/Containers/com.example.renamed")),
        Some((
            "com.domovoi.gone".to_owned(),
            Installation::Missing,
            OwnerEvidence::ContainerMetadata
        ))
    );
    assert_eq!(
        owner(
            &index,
            &format!("{LIBRARY}/Group Containers/ZYXWV98765.com.domovoi.gone")
        ),
        Some((
            "ZYXWV98765.com.domovoi.gone".to_owned(),
            Installation::Missing,
            OwnerEvidence::TeamPrefix
        ))
    );
    assert_eq!(
        owner(&index, &format!("{LIBRARY}/Group Containers/group.com.domovoi.shared")),
        None,
        "a group listed by two applications belongs to neither"
    );
    assert_eq!(
        owner(&index, &format!("{LIBRARY}/Group Containers/group.com.apple.notes")),
        None
    );
    assert_eq!(
        owner(&index, &format!("{LIBRARY}/LaunchAgents/com.domovoi.gone.helper.plist")),
        Some((
            "com.domovoi.gone.helper".to_owned(),
            Installation::Missing,
            OwnerEvidence::BundleIdName
        ))
    );
    assert_eq!(
        owner(&index, &format!("{LIBRARY}/Application Support/Gone")),
        None,
        "a removed application has no name to match"
    );
}

#[test]
fn a_mounted_volume_is_indexed_relative_to_its_root() {
    let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("the image attaches");
    let root = fs::canonicalize(volume.mount_point()).expect("the mount point resolves");
    container(&library(&root, "Containers/com.example.renamed"), "com.domovoi.gone");
    let kind = IndexRoot::for_path(&root).expect("root kind");
    assert_eq!(
        kind,
        IndexRoot::Volume {
            path: root.to_string_lossy().into_owned()
        }
    );
    let report = scan(&root, &ScanOptions::default()).expect("the volume scans");
    let index = Index::build(&report, kind, &Enrichment::default());
    assert_eq!(index.node(at(&index, "Users/tester")).category, Category::Home);
    assert_eq!(
        owner(&index, &format!("{LIBRARY}/Containers/com.example.renamed")),
        Some((
            "com.domovoi.gone".to_owned(),
            Installation::Missing,
            OwnerEvidence::ContainerMetadata
        ))
    );
}
