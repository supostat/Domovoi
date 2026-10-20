use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Command;

use domovoi_downloader::{Catalog, CatalogEntry, ModelStore};
use domovoi_render::BgraImage;
use domovoi_testkit::{CDN_ETAG, HfServer, ImageVolume, ServerBehaviour, TempTree, Visibility, VolumeFormat};
use serde_json::{Value, json};

const BUNDLED_EXECUTABLE: &str = "../../target/bundle/DOMOVOI.app/Contents/MacOS/domovoi";
const TEST_CATALOG: &str = "../../assets/test/catalog.json";
const TINY_MODEL: &str = "../../assets/test/tiny-random.gguf";
const ENTRY: &str = "test.tiny-random";
const QUESTION: &str = "Что занимает больше всего места?";
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
const TRASH_HEAVY_NODES: usize = 2;
const SNAPSHOTS: usize = 16;
const PARTIAL_STATUS: u16 = 206;
const REDIRECT_STATUS: u16 = 302;
const RANDOM_MODEL_TOKENS_PER_STEP: u32 = 32_768;

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
    write(&root.join("Locked/secret.txt"), b"closed");
    run(CHMOD, &["000"], &root.join("Locked"));
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

fn trash_of(mount_point: &Path) -> PathBuf {
    mount_point.join(".Trashes").join(unsafe { libc::getuid() }.to_string())
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
    let snapshots = state.path().join("snapshots");
    let entry = tiny_entry();
    let content = fs::read(manifest_path(TINY_MODEL)).expect("the tiny model reads");
    seed_partial(&models, &entry, &content);
    let server = serve(&entry, content);
    let unwatched = mount_point.join("unwatched.txt");
    let fresh_in_root = mount_point.join("fresh.txt");
    let fresh_in_depth = mount_point.join(DEEP_FOLDER).join("fresh.o");
    let next_to_trashed = mount_point.join("Projects/engine/after-trash.txt");
    let scenario = json!({
        "root": mount_point,
        "state": state.path().join("session"),
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
            {"step": "trash_recommendations", "level": "safe"},
            {"step": "create_file", "path": next_to_trashed, "bytes": FRESH_FILE_BYTES},
            {
                "step": "await_change",
                "path": next_to_trashed,
                "timeout_seconds": CHANGE_TIMEOUT_SECONDS,
                "heavy_nodes": TRASH_HEAVY_NODES,
            },
            {"step": "restore_last"},
            {"step": "find_duplicates"},
            {"step": "replace_with_clone"},
            {"step": "purge_trash"},
            {
                "step": "download_model",
                "catalog": manifest_path(TEST_CATALOG),
                "entry": ENTRY,
                "mirror": server.base(),
                "models": models,
            },
            {
                "step": "ask_assistant",
                "question": QUESTION,
                "max_tokens_per_step": RANDOM_MODEL_TOKENS_PER_STEP,
            },
            {"step": "snapshots", "directory": snapshots, "scale": 1},
        ],
    });
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
    assert_model_and_answer(&report, &server);
    assert_snapshots(&report);
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
    assert_eq!(changes.len(), 3, "{report:#}");
    for change in &changes[..2] {
        assert!(change["allocated"].as_u64().is_some_and(|bytes| bytes > 0), "{change}");
        assert_eq!(
            change["descended"].as_u64(),
            Some(0),
            "only one level is read: {change}"
        );
    }
    let moved_to_trash = changes[2];
    assert!(
        moved_to_trash["applied_in_background"]
            .as_u64()
            .is_some_and(|batches| batches > 0),
        "the batch after the move to the Trash is applied with the session in a background job: {moved_to_trash}"
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
    assert!(report["totals"]["safe"].as_u64().is_some_and(|safe| safe > 0));
    let trash = trash_of(mount_point);
    let trashed = step(report, "trash_recommendations");
    assert!(trashed["moved"].as_u64().is_some_and(|moved| moved > 0), "{trashed:#}");
    assert!(
        trashed["measured_files"].as_u64().is_some_and(|files| files > 0),
        "the tray is measured in the background before the move: {trashed:#}"
    );
    assert_eq!(trashed["estimated"].as_bool(), Some(false), "{trashed:#}");
    assert!(
        trashed["trashed"]
            .as_array()
            .expect("the trash step lists moved paths")
            .iter()
            .filter_map(Value::as_str)
            .all(|path| Path::new(path).starts_with(&trash)),
        "{trashed:#}"
    );
    let restored = step(report, "restore_last")["restored"]
        .as_str()
        .expect("a restored path is reported");
    assert!(Path::new(restored).exists(), "{restored} is back");
    assert!(mount_point.join("Projects/frozen/target/debug/frozen").exists());
    assert!(
        step(report, "find_duplicates")["groups"]
            .as_array()
            .is_some_and(|groups| !groups.is_empty())
    );
    assert!(step(report, "replace_with_clone")["trashed"].as_str().is_some());
    let purge = step(report, "purge_trash");
    assert!(
        purge["purged"].as_u64().is_some_and(|purged| purged > 0),
        "what Domovoi moved to the Trash is removed for good: {purge}"
    );
    assert_eq!(purge["failed"], json!([]), "{purge}");
    assert_eq!(purge["journal_left"], 0, "the journal forgets the removed set: {purge}");
    assert_eq!(
        purge["paths_left"], 0,
        "the Trash no longer holds the removed set: {purge}"
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
    let answer = step(report, "ask_assistant");
    assert!(answer["service_pid"].as_i64().is_some_and(|pid| pid > 0), "{answer:#}");
    assert!(
        answer["tools"].as_array().is_some_and(|tools| !tools.is_empty()),
        "the first step calls a tool: {answer:#}"
    );
    assert!(answer["chat_entries"].as_u64().is_some_and(|entries| entries >= 3));
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
