use std::f64::consts::TAU;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Command;

use domovoi_app::strings;
use domovoi_downloader::{Catalog, CatalogEntry, ModelStore};
use domovoi_render::BgraImage;
use domovoi_testkit::{CDN_ETAG, HfServer, ImageVolume, ServerBehaviour, TempTree, Visibility, VolumeFormat};
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
const NESTED_MUSIC: &str = "Music ▸ Music";
const ANGLE_TOLERANCE: f64 = 1e-3;
const CARD_RULE: &str = "cargo.target";

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
        report["totals"]["safe"], 0,
        "the safe recommendations are gone once deleted: {:#}",
        report["totals"]
    );
    assert_delete_and_clone(report, mount_point);
}

fn assert_delete_and_clone(report: &Value, mount_point: &Path) {
    let deleted = step(report, "delete_recommendations");
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
