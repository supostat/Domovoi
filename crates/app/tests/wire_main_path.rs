use std::f64::consts::TAU;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Command;

use domovoi_app::strings;
use domovoi_downloader::{Catalog, CatalogEntry, ModelStore};
use domovoi_render::BgraImage;
use domovoi_testkit::{
    CDN_ETAG, HfServer, ImageVolume, PropertyValue, ServerBehaviour, TempTree, Visibility, VolumeFormat, app_container,
    application_bundle, write_property_list,
};
use domovoi_theme::{NO_BREAK_SPACE, group_thousands};
use serde_json::{Value, json};

const BUNDLED_EXECUTABLE: &str = "../../target/bundle/DOMOVOI.app/Contents/MacOS/domovoi";
const TEST_CATALOG: &str = "../../assets/test/catalog.json";
const TINY_MODEL: &str = "../../assets/test/tiny-random.gguf";
const ENTRY: &str = "test.tiny-random";
const QUESTION: &str = "Что занимает больше всего места?";
const FOLLOW_UP: &str = "А что внутри самой большой папки?";
const VALIDATOR_ATTRIBUTE: &str = "com.domovoi.download-validator";
const XATTR: &str = "/usr/bin/xattr";
const CHFLAGS: &str = "/usr/bin/chflags";
const CHMOD: &str = "/bin/chmod";
const CP: &str = "/bin/cp";
const METAL_DEVICES: &str = "GGML_METAL_DEVICES";
const MEGABYTE: usize = 1024 * 1024;
const SPARSE_LENGTH: u64 = 64 * 1024 * 1024;
const FRESH_FILE_BYTES: usize = 64 * 1024;
const DEEP_FOLDER: &str = "Users/tester/Library/Developer/Xcode/DerivedData/App-abc/Build/Intermediates";
const CHANGE_TIMEOUT_SECONDS: u64 = 10;
const DELETE_HEAVY_NODES: usize = 2;
const SNAPSHOTS: usize = 16;
const NOTES: &str = "Notes";
const NOTE_FILES: usize = 60;
const OFFSET_TOLERANCE: f64 = 0.5;
const PARTIAL_STATUS: u16 = 206;
const REDIRECT_STATUS: u16 = 302;
const RANDOM_MODEL_TOKENS_PER_STEP: u32 = 32_768;
const ARCHIVE: &str = "Archive";
const ARCHIVE_PARTS: usize = 100;
const ARCHIVE_BLOB_BYTES: usize = 8 * MEGABYTE;
const MUSIC: &str = "Music";
const LIBRARY: &str = "Users/tester/Library";
const TINY_BUNDLE: &str = "Applications/Tiny.app";
const TINY_AGENT: &str = "LaunchAgents/com.domovoi.tiny.helper.plist";
const NESTED_MUSIC: &str = "Music ▸ Music";
const ANGLE_TOLERANCE: f64 = 1e-3;
const CARD_RULE: &str = "cargo.target";
const LOGS: &str = "Logs";
const TOOL_LOGS: &str = "Logs/Tool";
const USER_CRASH_REPORTS: &str = "Logs/DiagnosticReports";
const SYSTEM_CRASH_REPORTS: &str = "Library/Logs/DiagnosticReports";
const CRASH_REPORT: &str = "Tool.ips";
const MAIL_DOWNLOADS: &str = "Containers/com.apple.mail/Data/Library/Mail Downloads";
const ATTACHMENT: &str = "6F1E2D3C-4B5A-4978-8695-A4B3C2D1E0F9";
const TRASH: &str = "Users/tester/.Trash";
const TRASHED: &str = "old.zip";
const PURGE_TIMEOUT_SECONDS: u64 = 30;
const DROPPED: &str = "Drops/Keep";
const PINNED: &str = "Drops/Pinned";

fn manifest_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn noise(size: usize, seed: u8) -> Vec<u8> {
    (0..size)
        .map(|position| (position as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

fn write(path: &Path, content: &[u8]) {
    fs::create_dir_all(path.parent().expect("a fixture file has a folder")).expect("the fixture folder is created");
    fs::write(path, content).expect("the fixture file is written");
}

fn run(program: &str, arguments: &[&str], path: &Path) {
    let status = Command::new(program)
        .args(arguments)
        .arg(path)
        .status()
        .expect("the tool starts");
    assert!(status.success(), "{program} {arguments:?} {} failed", path.display());
}

fn populate(root: &Path) {
    write(&root.join(DEEP_FOLDER).join("main.o"), &noise(MEGABYTE, 1));
    write(
        &root.join("Projects/engine/Cargo.toml"),
        b"[package]\nname = \"engine\"\n",
    );
    write(
        &root.join("Projects/engine/target/debug/engine"),
        &noise(2 * MEGABYTE, 2),
    );
    write(&root.join("Projects/site/package.json"), b"{\"name\": \"site\"}\n");
    write(
        &root.join("Projects/site/node_modules/left-pad/index.js"),
        &noise(MEGABYTE, 3),
    );
    write(
        &root.join("Projects/frozen/Cargo.toml"),
        b"[package]\nname = \"frozen\"\n",
    );
    write(&root.join("Projects/frozen/target/debug/frozen"), &noise(MEGABYTE, 4));
    run(CHFLAGS, &["uchg"], &root.join("Projects/frozen/target"));
    let duplicate = noise(3 * MEGABYTE / 2, 5);
    write(&root.join("Media/a.bin"), &duplicate);
    write(&root.join("Media/b.bin"), &duplicate);
    let cloned = Command::new(CP)
        .arg("-c")
        .arg(root.join("Media/a.bin"))
        .arg(root.join("Media/c.bin"))
        .status()
        .expect("cp starts");
    assert!(cloned.success(), "the clone is made");
    write(&root.join("Disk/sparse.img"), b"head");
    File::options()
        .write(true)
        .open(root.join("Disk/sparse.img"))
        .and_then(|file| file.set_len(SPARSE_LENGTH))
        .expect("the sparse file grows without data");
    for number in 0..NOTE_FILES {
        write(
            &root.join(NOTES).join(format!("note-{number:02}.txt")),
            format!("note {number}\n").as_bytes(),
        );
    }
    write(&root.join("Locked/secret.txt"), b"closed");
    run(CHMOD, &["000"], &root.join("Locked"));
    write(&root.join(ARCHIVE).join("blob.bin"), &noise(ARCHIVE_BLOB_BYTES, 6));
    for number in 0..ARCHIVE_PARTS {
        write(
            &root.join(ARCHIVE).join(format!("part-{number:03}.txt")),
            format!("part {number}\n").as_bytes(),
        );
    }
    write(&root.join(MUSIC).join(MUSIC).join("song.mp3"), &noise(MEGABYTE, 7));
    write(&root.join(MUSIC).join("Podcasts/talk.mp3"), &noise(MEGABYTE / 2, 8));
    populate_applications(root);
    populate_cleanup(root);
    write(&root.join(DROPPED).join("draft.txt"), b"a draft kept for later\n");
    write(&root.join(PINNED).join("pinned.txt"), b"a pinned note\n");
    run(CHFLAGS, &["uchg"], &root.join(PINNED));
}

fn populate_cleanup(root: &Path) {
    let library = root.join(LIBRARY);
    write(&library.join(TOOL_LOGS).join("tool.log"), &noise(FRESH_FILE_BYTES, 12));
    write(
        &library.join(USER_CRASH_REPORTS).join(CRASH_REPORT),
        &noise(FRESH_FILE_BYTES, 13),
    );
    write(
        &root.join(SYSTEM_CRASH_REPORTS).join(CRASH_REPORT),
        &noise(FRESH_FILE_BYTES, 14),
    );
    write(
        &library.join(MAIL_DOWNLOADS).join(ATTACHMENT).join("invoice.pdf"),
        &noise(FRESH_FILE_BYTES, 15),
    );
    write(&root.join(TRASH).join(TRASHED), &noise(FRESH_FILE_BYTES, 16));
}

fn populate_applications(root: &Path) {
    let library = root.join(LIBRARY);
    application_bundle(
        &root.join("Applications"),
        "Tiny",
        "com.domovoi.tiny",
        &["group.com.domovoi.tiny"],
    )
    .expect("the installed application is built");
    app_container(&library.join("Containers/com.domovoi.tiny"), "com.domovoi.tiny").expect("its container");
    write(
        &library.join("Group Containers/group.com.domovoi.tiny/shared.db"),
        &noise(FRESH_FILE_BYTES, 9),
    );
    write_property_list(
        &library.join(TINY_AGENT),
        &[
            ("Label", PropertyValue::Text("com.domovoi.tiny.helper")),
            ("AssociatedBundleIdentifiers", PropertyValue::Text("com.domovoi.tiny")),
        ],
    )
    .expect("its launch agent is written");
    write(&library.join("Application Support/Tiny/state.json"), b"{}");
    app_container(&library.join("Containers/com.example.renamed"), "com.domovoi.gone").expect("a leftover container");
    write(
        &library.join("Caches/com.domovoi.gone/cache.db"),
        &noise(FRESH_FILE_BYTES, 10),
    );
    write(
        &library.join("Group Containers/ZYXWV98765.com.domovoi.gone/data.db"),
        &noise(FRESH_FILE_BYTES, 11),
    );
}

fn tiny_entry() -> CatalogEntry {
    Catalog::from_path(&manifest_path(TEST_CATALOG))
        .expect("the test catalog parses")
        .entry(ENTRY)
        .expect("the test catalog lists the tiny model")
        .clone()
}

fn serve(entry: &CatalogEntry, content: Vec<u8>) -> HfServer {
    HfServer::start(ServerBehaviour {
        content,
        revision: entry.revision.clone(),
        linked_etag: entry.sha256.to_string(),
        expired_links: 0,
        ignore_ranges: false,
        chunk: MEGABYTE,
        chunk_delay: std::time::Duration::ZERO,
    })
}

fn seed_partial(models: &Path, entry: &CatalogEntry, content: &[u8]) {
    let partial = ModelStore::open(models)
        .expect("the model store opens")
        .partial_path(entry);
    fs::write(&partial, &content[..content.len() / 2]).expect("half of the model is already on disk");
    let status = Command::new(XATTR)
        .args(["-w", VALIDATOR_ATTRIBUTE, CDN_ETAG])
        .arg(&partial)
        .status()
        .expect("xattr starts");
    assert!(status.success(), "the partial file remembers its validator");
}

fn steps<'a>(report: &'a Value, name: &str) -> Vec<&'a Value> {
    report["steps"]
        .as_array()
        .expect("the report lists steps")
        .iter()
        .filter(|step| step["step"] == name)
        .collect()
}

fn step<'a>(report: &'a Value, name: &str) -> &'a Value {
    report["steps"]
        .as_array()
        .expect("the report lists steps")
        .iter()
        .find(|step| step["step"] == name)
        .unwrap_or_else(|| panic!("the report has no {name} step: {report:#}"))
}

#[test]
fn the_bundled_application_walks_the_main_path_on_real_modules() {
    let executable = manifest_path(BUNDLED_EXECUTABLE);
    assert!(
        executable.is_file(),
        "{} is missing: cargo xtask bundle --profile adhoc builds it",
        executable.display()
    );
    let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Browsable).expect("the image attaches");
    let mount_point = fs::canonicalize(volume.mount_point()).expect("the mount point resolves");
    populate(&mount_point);
    let state = TempTree::new().expect("a temp tree is created");
    let models = state.path().join("models");
    let entry = tiny_entry();
    let content = fs::read(manifest_path(TINY_MODEL)).expect("the tiny model reads");
    seed_partial(&models, &entry, &content);
    let server = serve(&entry, content);
    let csv = state.path().join("index.csv");
    let scenario = scenario(&mount_point, state.path(), &models, &csv, server.base());
    let scenario_path = state.path().join("scenario.json");
    let report_path = state.path().join("report.json");
    fs::write(&scenario_path, scenario.to_string()).expect("the scenario is written");
    let output = Command::new(&executable)
        .arg("--e2e")
        .arg(&scenario_path)
        .arg("--report")
        .arg(&report_path)
        .env(METAL_DEVICES, "0")
        .output()
        .expect("the bundled application starts");
    let report: Value = serde_json::from_str(&fs::read_to_string(&report_path).expect("the report is written"))
        .expect("the report parses");
    assert!(
        output.status.success(),
        "the main path failed: {}\n{report:#}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_index_and_care(&report, &mount_point);
    assert_disk_update(&report);
    assert_interface_techniques(&report, &csv);
    assert_scanner_techniques(&report);
    assert_space_release(&report);
    assert_tray_drag(&report, &mount_point);
    assert_model_and_answer(&report, &server);
    assert_snapshots(&report);
}

fn scenario(mount_point: &Path, state: &Path, models: &Path, csv: &Path, mirror: &str) -> Value {
    let snapshots = state.join("snapshots");
    let unwatched = mount_point.join("unwatched.txt");
    let fresh_in_root = mount_point.join("fresh.txt");
    let fresh_in_depth = mount_point.join(DEEP_FOLDER).join("fresh.o");
    let next_to_deleted = mount_point.join("Projects/engine/after-delete.txt");
    let notes = mount_point.join(NOTES);
    let fresh_note = notes.join("fresh-note.txt");
    let archive = mount_point.join(ARCHIVE);
    let music = mount_point.join(MUSIC);
    json!({
        "root": mount_point,
        "state": state.join("session"),
        "steps": [
            {"step": "scan"},
            {"step": "reopen_cache"},
            {"step": "create_file", "path": unwatched, "bytes": FRESH_FILE_BYTES},
            {"step": "rescan", "path": unwatched},
            {"step": "create_file", "path": fresh_in_root, "bytes": FRESH_FILE_BYTES},
            {"step": "await_change", "path": fresh_in_root, "timeout_seconds": CHANGE_TIMEOUT_SECONDS},
            {"step": "create_file", "path": fresh_in_depth, "bytes": FRESH_FILE_BYTES},
            {"step": "await_change", "path": fresh_in_depth, "timeout_seconds": CHANGE_TIMEOUT_SECONDS},
            {"step": "recommendations"},
            {
                "step": "download_model",
                "catalog": manifest_path(TEST_CATALOG),
                "entry": ENTRY,
                "mirror": mirror,
                "models": models,
            },
            {
                "step": "ask_about_card",
                "rule": CARD_RULE,
                "max_tokens_per_step": RANDOM_MODEL_TOKENS_PER_STEP,
            },
            {"step": "delete_recommendations", "level": "safe"},
            {"step": "delete_recommendations", "level": "recoverable"},
            {"step": "create_file", "path": next_to_deleted, "bytes": FRESH_FILE_BYTES},
            {
                "step": "await_change",
                "path": next_to_deleted,
                "timeout_seconds": CHANGE_TIMEOUT_SECONDS,
                "heavy_nodes": DELETE_HEAVY_NODES,
            },
            {"step": "scroll_disk_list", "path": notes},
            {"step": "create_file", "path": fresh_note, "bytes": FRESH_FILE_BYTES},
            {"step": "await_change", "path": fresh_note, "timeout_seconds": CHANGE_TIMEOUT_SECONDS},
            {"step": "update_disk_screen"},
            {"step": "scan_notice"},
            {"step": "small_groups", "path": archive},
            {"step": "preview_targets", "path": music},
            {"step": "export_csv", "path": csv},
            {"step": "hidden_space"},
            {"step": "tmutil_commands"},
            {"step": "purge_space", "timeout_seconds": PURGE_TIMEOUT_SECONDS},
            {"step": "hidden_space"},
            {"step": "remove_application", "path": mount_point.join(TINY_BUNDLE)},
            {"step": "delete_recommendations", "level": "check_yourself"},
            {"step": "mark_from_list", "paths": [mount_point.join(DROPPED), mount_point.join(PINNED)]},
            {"step": "unmark_tray_row", "row": 0},
            {"step": "find_duplicates"},
            {"step": "replace_with_clone"},
            {
                "step": "ask_assistant",
                "question": QUESTION,
                "max_tokens_per_step": RANDOM_MODEL_TOKENS_PER_STEP,
            },
            {
                "step": "ask_assistant",
                "question": FOLLOW_UP,
                "max_tokens_per_step": RANDOM_MODEL_TOKENS_PER_STEP,
            },
            {"step": "snapshots", "directory": snapshots, "scale": 1},
        ],
    })
}

fn assert_index_and_care(report: &Value, mount_point: &Path) {
    assert!(step(report, "scan")["files"].as_u64().is_some_and(|files| files > 0));
    let cached_event = step(report, "reopen_cache")["last_event_id"]
        .as_u64()
        .expect("the cache carries an event id");
    let rescan = step(report, "rescan");
    assert_eq!(
        rescan["found"], true,
        "the rescan finds a file no event delivered: {rescan}"
    );
    assert!(
        rescan["last_event_id"]
            .as_u64()
            .is_some_and(|event| event > cached_event),
        "events resume from the rescan, not from the cache: {rescan}"
    );
    let changes = steps(report, "await_change");
    assert_eq!(changes.len(), 4, "{report:#}");
    for change in &changes[..2] {
        assert!(change["allocated"].as_u64().is_some_and(|bytes| bytes > 0), "{change}");
        assert_eq!(
            change["descended"].as_u64(),
            Some(0),
            "only one level is read: {change}"
        );
    }
    let after_delete = changes[2];
    assert!(
        after_delete["applied_in_background"]
            .as_u64()
            .is_some_and(|batches| batches > 0),
        "the batch after the delete is applied with the session in a background job: {after_delete}"
    );
    let recommendations = step(report, "recommendations");
    let cards: Vec<&str> = recommendations["cards"]
        .as_array()
        .expect("the recommendations list cards")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(cards.iter().any(|card| card.ends_with("cargo.target")), "{cards:?}");
    assert!(cards.iter().any(|card| card.ends_with("dev.deps")), "{cards:?}");
    assert!(
        recommendations["blocked"]
            .as_array()
            .is_some_and(|blocked| !blocked.is_empty()),
        "the locked build folder is refused by the denylist: {recommendations:#}"
    );
    assert!(
        recommendations["total"].as_u64().is_some_and(|total| total > 0),
        "the rules find something to free before the delete: {recommendations:#}"
    );
    assert_eq!(
        report["totals"],
        json!({"safe": 0, "recoverable": 0, "check_yourself": 0}),
        "the recommendations of every level are gone once deleted"
    );
    assert_delete_and_clone(report, mount_point);
    assert_application_removal(report, mount_point);
    assert_cleanup_cards(report, mount_point);
}

fn delete_steps(report: &Value) -> [&Value; 3] {
    steps(report, "delete_recommendations")
        .try_into()
        .unwrap_or_else(|deletes: Vec<&Value>| panic!("one delete per level: {deletes:#?}"))
}

fn assert_cleanup_cards(report: &Value, mount_point: &Path) {
    let library = mount_point.join(LIBRARY);
    let user_reports = library.join(USER_CRASH_REPORTS);
    let system_reports = mount_point.join(SYSTEM_CRASH_REPORTS);
    let mail_downloads = library.join(MAIL_DOWNLOADS);
    let trash = mount_point.join(TRASH);
    assert_eq!(card_paths(report, "logs.user"), [library.join(TOOL_LOGS)]);
    let mut crash_reports = card_paths(report, "logs.crash-reports");
    crash_reports.sort();
    let mut expected_reports = vec![user_reports.join(CRASH_REPORT), system_reports.join(CRASH_REPORT)];
    expected_reports.sort();
    assert_eq!(
        crash_reports, expected_reports,
        "the reports go while their folders stay unclaimed"
    );
    assert_eq!(card_paths(report, "mail.downloads"), [mail_downloads.join(ATTACHMENT)]);
    assert_eq!(card_paths(report, "trash.user"), [trash.join(TRASHED)]);
    let [_, recoverable, check_yourself] = delete_steps(report);
    for deleted in [recoverable, check_yourself] {
        assert!(
            deleted["deleted"].as_u64().is_some_and(|objects| objects > 0),
            "{deleted:#}"
        );
        assert_eq!(deleted["failures"], json!([]), "{deleted:#}");
        assert_eq!(deleted["paths_left"], 0, "{deleted:#}");
    }
    assert!(!mail_downloads.join(ATTACHMENT).exists(), "{recoverable:#}");
    assert!(!user_reports.join(CRASH_REPORT).exists(), "{recoverable:#}");
    assert!(!system_reports.join(CRASH_REPORT).exists(), "{recoverable:#}");
    assert!(!trash.join(TRASHED).exists(), "{check_yourself:#}");
    for folder in [&trash, &user_reports, &system_reports, &mail_downloads] {
        assert!(folder.is_dir(), "{} stays", folder.display());
    }
}

fn card_paths(report: &Value, rule: &str) -> Vec<PathBuf> {
    let recommendations = step(report, "recommendations");
    let position = recommendations["cards"]
        .as_array()
        .expect("the report lists cards")
        .iter()
        .position(|card| {
            card.as_str()
                .is_some_and(|label| label.ends_with(&format!(" · {rule}")))
        })
        .unwrap_or_else(|| panic!("there is a {rule} card: {recommendations:#}"));
    recommendations["card_paths"][position]
        .as_array()
        .expect("a card lists its paths")
        .iter()
        .map(|path| PathBuf::from(path.as_str().expect("a path is text")))
        .collect()
}

fn listed(value: &Value) -> Vec<PathBuf> {
    value
        .as_array()
        .expect("the report lists paths")
        .iter()
        .map(|path| PathBuf::from(path.as_str().expect("a path is text")))
        .collect()
}

fn assert_application_removal(report: &Value, mount_point: &Path) {
    let library = mount_point.join(LIBRARY);
    assert_eq!(
        card_paths(report, "apps.leftover-caches"),
        [library.join("Caches/com.domovoi.gone")]
    );
    let mut leftover_data = card_paths(report, "apps.leftover-data");
    leftover_data.sort();
    assert_eq!(
        leftover_data,
        [
            library.join("Containers/com.example.renamed"),
            library.join("Group Containers/ZYXWV98765.com.domovoi.gone"),
        ],
        "only the removed application leaves data behind"
    );
    let removed = step(report, "remove_application");
    let mut deleted = listed(&removed["deleted"]);
    deleted.sort();
    let mut expected = vec![
        mount_point.join(TINY_BUNDLE),
        library.join("Containers/com.domovoi.tiny"),
        library.join("Group Containers/group.com.domovoi.tiny"),
        library.join(TINY_AGENT),
    ];
    expected.sort();
    assert_eq!(deleted, expected, "{removed:#}");
    assert_eq!(removed["refused"], 0, "{removed:#}");
    assert_eq!(removed["failures"], 0, "{removed:#}");
    assert_eq!(
        removed["paths_left"], 0,
        "the application and its files are gone: {removed:#}"
    );
    assert_eq!(
        listed(&removed["name_only"]),
        [library.join("Application Support/Tiny")],
        "a folder that only shares the name is reported unmarked"
    );
    assert!(library.join("Application Support/Tiny/state.json").exists());
    for shared in ["Containers", "Group Containers", "LaunchAgents", "Application Support"] {
        assert!(library.join(shared).is_dir(), "{shared} stays");
    }
    assert!(mount_point.join("Applications").is_dir());
}

fn assert_delete_and_clone(report: &Value, mount_point: &Path) {
    let [deleted, _, _] = delete_steps(report);
    let expected = deleted["expected"].as_u64().expect("the tray total is reported");
    assert!(expected > 0, "{deleted:#}");
    assert!(
        deleted["measured_files"].as_u64().is_some_and(|files| files > 0),
        "the tray is measured in the background before the delete: {deleted:#}"
    );
    assert!(
        deleted["deleted"].as_u64().is_some_and(|objects| objects > 0),
        "the safe recommendations are deleted: {deleted:#}"
    );
    assert_eq!(deleted["freed"].as_u64(), Some(expected), "{deleted:#}");
    let files_total = deleted["files_total"].as_u64().expect("the file total is reported");
    assert!(files_total > 0, "the taken items hold files: {deleted:#}");
    assert_eq!(
        deleted["files_done"].as_u64(),
        Some(files_total),
        "the count reaches the total from the index: {deleted:#}"
    );
    let line = deleted["deleting_line"].as_str().expect("the sheet line is reported");
    let total = group_thousands(files_total);
    assert!(
        ["файлов", "файла"]
            .iter()
            .any(|word| line.ends_with(&format!("{total}{NO_BREAK_SPACE}{word}"))),
        "the sheet line ends with the file total: {line}"
    );
    assert_eq!(
        deleted["deleting_share"].as_f64(),
        Some(1.0),
        "the bar is full once the delete is done: {deleted:#}"
    );
    assert_eq!(deleted["failures"], json!([]), "{deleted:#}");
    assert_eq!(
        deleted["paths_left"], 0,
        "the disk no longer holds the taken items: {deleted:#}"
    );
    let (free_before, free_after) = (
        deleted["free_before"].as_u64().expect("free space before"),
        deleted["free_after"].as_u64().expect("free space after"),
    );
    assert!(
        free_after > free_before,
        "the volume has more free space after the delete: {deleted:#}"
    );
    assert!(!mount_point.join("Projects/engine/target").exists());
    assert!(!mount_point.join("Projects/site/node_modules").exists());
    assert!(mount_point.join("Projects/frozen/target/debug/frozen").exists());
    let library = mount_point.join(LIBRARY);
    assert!(
        !library.join(TOOL_LOGS).exists(),
        "the safe delete takes the program logs: {deleted:#}"
    );
    assert!(library.join(LOGS).is_dir(), "the logs folder itself stays");
    assert!(
        step(report, "find_duplicates")["groups"]
            .as_array()
            .is_some_and(|groups| !groups.is_empty())
    );
    let replaced = step(report, "replace_with_clone");
    assert!(
        replaced["freed"].as_u64().is_some_and(|freed| freed > 0),
        "the old body of the replaced copy is deleted: {replaced:#}"
    );
    let temporaries: Vec<PathBuf> = fs::read_dir(mount_point.join("Media"))
        .expect("the media folder lists")
        .map(|entry| entry.expect("an entry reads").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(".domovoi-tmp-"))
        })
        .collect();
    assert!(temporaries.is_empty(), "no temporary is left behind: {temporaries:?}");
    assert!(
        !mount_point.join(".Trashes").exists(),
        "nothing goes to the Trash of the volume"
    );
}

fn assert_disk_update(report: &Value) {
    let shown = step(report, "scroll_disk_list");
    let updated = step(report, "update_disk_screen");
    assert_eq!(
        shown["rows"].as_u64(),
        Some(NOTE_FILES as u64),
        "the notes folder lists every note: {shown}"
    );
    let offset = shown["offset"].as_f64().expect("the list reports its offset");
    assert!(offset > 0.0, "the notes list scrolls to its end: {shown}");
    assert_eq!(
        updated["same_scroll"], true,
        "a real FSEvents batch keeps the list in its scroll view: {updated}"
    );
    assert_eq!(updated["same_table"], true, "the list keeps its table: {updated}");
    assert_eq!(
        updated["rows"].as_u64(),
        Some(NOTE_FILES as u64 + 1),
        "the fresh note joins the list: {updated}"
    );
    let kept = updated["offset"].as_f64().expect("the list reports its offset");
    assert!(
        (kept - offset).abs() < OFFSET_TOLERANCE,
        "the list stays where it was ({offset} → {kept} pt): {updated}"
    );
}

fn assert_interface_techniques(report: &Value, csv: &Path) {
    let notice = step(report, "scan_notice");
    let line = notice["line"].as_str().expect("the notice carries a line");
    assert!(line.starts_with("Скан завершён за"), "{notice}");
    assert!(!line.contains('!'), "the notice keeps the voice: {line}");
    let groups = step(report, "small_groups");
    assert_eq!(
        groups["groups"], 1,
        "the tiny parts of the archive fold into one group: {groups}"
    );
    assert_eq!(groups["segments"], 2, "the blob and the group: {groups}");
    let ring_sum = groups["ring_sum"].as_f64().expect("the ring sum is reported");
    assert!((ring_sum - TAU).abs() < ANGLE_TOLERANCE, "the ring closes: {groups}");
    let preview = step(report, "preview_targets");
    let rows: Vec<&str> = preview["rows"]
        .as_array()
        .expect("the level rows are listed")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(rows.contains(&NESTED_MUSIC), "{rows:?}");
    assert_eq!(preview["targets"].as_u64(), Some(rows.len() as u64), "{preview}");
    assert!(rows.len() > 1, "{rows:?}");
    let export = step(report, "export_csv");
    assert_eq!(export["rows"], export["nodes"], "one row per node: {export}");
    let text = fs::read_to_string(csv).expect("the export is on disk");
    let lines = text.split("\r\n").filter(|row| !row.is_empty()).count();
    assert_eq!(lines as u64 - 1, export["rows"].as_u64().expect("rows"), "{export}");
    assert!(
        text.starts_with("path,bytes,category,modified\r\n"),
        "{}",
        &text[..64.min(text.len())]
    );
}

fn assert_scanner_techniques(report: &Value) {
    let rescan = step(report, "rescan");
    assert_eq!(
        rescan["sweep"].as_f64(),
        Some(1.0),
        "the rescan ends with the whole sector: {rescan}"
    );
    let hidden = step(report, "hidden_space");
    assert_eq!(hidden["discrepancy"], false, "{hidden:#}");
    assert!(
        hidden["total"].as_i64().is_some_and(|total| total >= 0),
        "the volume holds at least what the scan saw: {hidden:#}"
    );
    assert_eq!(
        hidden["other_user_homes"], 0,
        "the image has no home folders of other users: {hidden:#}"
    );
    assert!(
        hidden["hidden_folders"].as_u64().is_some_and(|folders| folders >= 1),
        "the locked folder stays hidden: {hidden:#}"
    );
    let commands = step(report, "tmutil_commands");
    let time_machine = commands["time_machine"]
        .as_u64()
        .expect("Time Machine snapshots are counted");
    let thinning = u64::from(commands["other"].as_u64().expect("other snapshots are counted") > 0);
    assert_eq!(
        commands["lines"].as_u64(),
        Some(time_machine + thinning),
        "one line per Time Machine snapshot and one thinning line for the rest: {commands}"
    );
    assert_eq!(
        commands["time_machine"].as_u64(),
        step(report, "hidden_space")["snapshots"]
            .as_u64()
            .map(|all| all - commands["other"].as_u64().unwrap_or(0)),
        "{commands}"
    );
}

fn assert_space_release(report: &Value) {
    let release = step(report, "purge_space");
    assert_eq!(
        release["error"],
        Value::Null,
        "the release finishes without an error: {release:#}"
    );
    assert_eq!(
        release["estimate"], 0,
        "an image volume has nothing to recover: {release:#}"
    );
    assert_eq!(
        release["finish"], "completed",
        "the mechanism reports its own completion: {release:#}"
    );
    let before = release["available_before"]
        .as_u64()
        .expect("the available space before the release is reported");
    let after = release["available_after"]
        .as_u64()
        .expect("the available space after the release is reported");
    assert_eq!(
        release["freed"],
        after.saturating_sub(before),
        "what is freed is the growth of the available space, whatever else the volume does meanwhile: {release:#}"
    );
    let hidden = steps(report, "hidden_space");
    assert_eq!(
        hidden.len(),
        2,
        "the hidden space is measured before and after the release: {report:#}"
    );
    assert_eq!(
        hidden[1]["discrepancy"], false,
        "the release leaves the volume consistent with the scan: {:#}",
        hidden[1]
    );
}

fn assert_tray_drag(report: &Value, mount_point: &Path) {
    let marked = step(report, "mark_from_list");
    assert_eq!(marked["hover_kind"], "add", "{marked:#}");
    let hover = marked["hover"].as_str().expect("the button names the drop");
    assert!(hover.starts_with("Добавить 1 из 2 · ≈ "), "{marked:#}");
    assert_eq!(marked["added"], 1, "{marked:#}");
    assert_eq!(marked["refused"], 1, "{marked:#}");
    assert_eq!(
        marked["refusal_line"], "Не добавлено: Pinned — защищено системой",
        "{marked:#}"
    );
    let summary = marked["summary"].as_str().expect("the button shows the summary");
    assert!(summary.starts_with("К удалению · 1 · ≈ "), "{marked:#}");
    assert_eq!(marked["tray"], json!([mount_point.join(DROPPED)]), "{marked:#}");
    assert_eq!(marked["marked_rows"], json!(["Keep"]), "{marked:#}");
    let collapsed = marked["collapsed_segments"]
        .as_u64()
        .expect("the step counts collapsed segments");
    assert!(collapsed > 0, "{marked:#}");
    let unmarked = step(report, "unmark_tray_row");
    assert_eq!(unmarked["rows_before"], 1, "{unmarked:#}");
    assert_eq!(unmarked["rows_after"], 0, "{unmarked:#}");
    assert_eq!(unmarked["summary"], "К удалению", "{unmarked:#}");
    assert_eq!(unmarked["marked_rows"], json!([]), "{unmarked:#}");
    assert_eq!(unmarked["collapsed_segments"], 0, "{unmarked:#}");
}

fn assert_model_and_answer(report: &Value, server: &HfServer) {
    let download = step(report, "download_model");
    assert_eq!(download["sha256_matches"], true, "{download:#}");
    assert_eq!(download["excluded_from_backup"], true, "{download:#}");
    let requests = server.requests();
    assert!(
        requests.iter().any(|request| request.status == REDIRECT_STATUS),
        "{requests:?}"
    );
    assert!(
        server
            .transfers()
            .iter()
            .any(|request| request.status == PARTIAL_STATUS && request.if_range.as_deref() == Some(CDN_ETAG)),
        "the download resumed with Range and If-Range: {requests:?}"
    );
    let card = step(report, "ask_about_card");
    assert_eq!(card["question"], strings::CARD_QUESTION, "{card:#}");
    assert_eq!(card["card_rule"], CARD_RULE, "{card:#}");
    assert_eq!(card["history_turns"], 0, "the card question opens the chat: {card:#}");
    assert!(
        card["tools"].as_array().is_some_and(|tools| !tools.is_empty()),
        "a question about a card starts with a tool call: {card:#}"
    );
    assert!(
        card["answer_characters"]
            .as_u64()
            .is_some_and(|characters| characters > 0),
        "{card:#}"
    );
    let answers = steps(report, "ask_assistant");
    assert_eq!(answers.len(), 2, "{report:#}");
    let (first, follow_up) = (answers[0], answers[1]);
    for answer in [first, follow_up] {
        assert!(answer["service_pid"].as_i64().is_some_and(|pid| pid > 0), "{answer:#}");
        assert!(
            answer["tools"].as_array().is_some_and(|tools| !tools.is_empty()),
            "the first step calls a tool: {answer:#}"
        );
        assert!(
            answer["answer_characters"]
                .as_u64()
                .is_some_and(|characters| characters > 0),
            "{answer:#}"
        );
    }
    assert_eq!(
        first["history_turns"], 1,
        "the question about a card is the first turn of the chat: {first:#}"
    );
    assert!(first["chat_entries"].as_u64().is_some_and(|entries| entries >= 3));
    assert_eq!(
        follow_up["history_turns"], 2,
        "the follow-up sees the earlier turns: {follow_up:#}"
    );
    assert!(
        follow_up["chat_entries"].as_u64() > first["chat_entries"].as_u64(),
        "one chat carries both turns: {follow_up:#}"
    );
}

fn assert_snapshots(report: &Value) {
    let files: Vec<PathBuf> = step(report, "snapshots")["files"]
        .as_array()
        .expect("the snapshot step lists files")
        .iter()
        .filter_map(Value::as_str)
        .map(PathBuf::from)
        .collect();
    assert_eq!(files.len(), SNAPSHOTS, "{files:?}");
    for file in files {
        let image = BgraImage::read_png(&file).expect("the snapshot reads");
        assert!(image.width > 0 && image.height > 0, "{}", file.display());
        assert!(
            image.pixels.iter().any(|channel| *channel != 0),
            "{} is not empty",
            file.display()
        );
    }
}
