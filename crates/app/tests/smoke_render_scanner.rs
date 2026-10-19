use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use domovoi_app::{
    AccessState, AiModels, CacheOpening, CareModels, DiskOverview, Expansion, NODE_BUDGET, ScanStage, ScanStatus,
    Session, SessionPaths, ShellData, strings,
};
use domovoi_index::{Category, Source};
use domovoi_render::{DiagramMode, DiagramRenderer, DiagramState, GlyphAtlas, NodeOrigin, ROOT};
use domovoi_scanner::{CancelToken, ScanProgress};
use domovoi_testkit::{Fill, ImageVolume, TempTree, Visibility, VolumeFormat};
use domovoi_theme::Palette;

const MEGABYTE: u64 = 1024 * 1024;
const SIDE: f32 = 440.0;
const PROGRESS_BACKLOG: usize = 64;

fn populate(root: &Path) {
    for (folder, file, size) in [
        ("Projects", "model.bin", 6 * MEGABYTE),
        ("Projects", "notes.txt", MEGABYTE / 4),
        ("Media", "film.mov", 4 * MEGABYTE),
    ] {
        fs::create_dir_all(root.join(folder)).expect("a folder is created");
        let length = usize::try_from(size).expect("fixture sizes fit in memory");
        fs::write(root.join(folder).join(file), vec![b'd'; length]).expect("a file is written");
    }
}

fn position_named(data: &ShellData, name: &str) -> usize {
    data.tree
        .node(ROOT)
        .children
        .iter()
        .copied()
        .find(|child| data.tree.node(*child).name == name)
        .unwrap_or_else(|| panic!("{name} is a segment of the disk"))
}

fn scan_status(session: &Session) -> ScanStatus {
    ScanStatus {
        bytes: 0,
        files: 0,
        sweep: 1.0,
        threads: session.workers(),
        current: String::new(),
        full_first_scan: false,
        stage: ScanStage::Walking,
    }
}

#[test]
fn session_feeds_the_real_scanner_index_and_renderer_on_a_volume() {
    let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("the image attaches");
    let mount_point = fs::canonicalize(volume.mount_point()).expect("the mount point resolves");
    populate(&mount_point);
    let caches = TempTree::new().expect("a temp tree is created");
    let paths = SessionPaths::in_directory(caches.path());
    let mut session = Session::open(&mount_point, &paths, "0.0.0-test").expect("the session opens");
    assert_eq!(
        session.open_cached().expect("the cache probe runs"),
        CacheOpening::Absent
    );
    let (sender, progress) = mpsc::sync_channel(PROGRESS_BACKLOG);
    let listener = thread::spawn(move || progress.iter().collect::<Vec<ScanProgress>>());
    session
        .scan(Some(sender), CancelToken::default())
        .expect("the volume scans");
    let reported = listener.join().expect("the progress listener ends with the scan");

    let index = session.index().expect("the scan installs an index");
    assert!(
        matches!(reported.last(), Some(ScanProgress::Walked { files, .. }) if *files >= 3),
        "the walk ends with the indexing stage: {reported:?}"
    );
    let overview = DiskOverview::measure(&mount_point).expect("the volume is measured");
    let expansion = Expansion::around(index, index.root(), NODE_BUDGET);
    let data = ShellData::measured(
        AccessState::Granted,
        &overview,
        Some(index),
        &expansion,
        scan_status(&session),
        CareModels::default(),
        AiModels::default(),
    );

    let names: Vec<&str> = data
        .tree
        .node(ROOT)
        .children
        .iter()
        .map(|child| data.tree.node(*child).name.as_str())
        .collect();
    for expected in ["Projects", "Media", strings::DIAGRAM.category_name(Category::Hidden)] {
        assert!(names.contains(&expected), "{expected} is missing from {names:?}");
    }
    for foreign in [strings::OTHER_VOLUMES, strings::MACOS] {
        assert!(
            !names.contains(&foreign),
            "{foreign} belongs to another container: {names:?}"
        );
    }
    assert!(!data.disk_name.is_empty());
    assert_eq!(data.tree.node(ROOT).name, data.disk_name);
    let projects = position_named(&data, "Projects");
    let projects_id = index.find(&mount_point.join("Projects")).expect("Projects is indexed");
    assert_eq!(data.tree.node(projects).origin, NodeOrigin::Indexed(projects_id));
    assert!(data.tree.node(projects).size >= 6 * MEGABYTE + MEGABYTE / 4);
    let file_names: Vec<&str> = data
        .tree
        .node(projects)
        .children
        .iter()
        .map(|child| data.tree.node(*child).name.as_str())
        .collect();
    assert_eq!(file_names, ["model.bin", "notes.txt"]);
    let hidden = position_named(&data, strings::DIAGRAM.category_name(Category::Hidden));
    assert_eq!(data.tree.node(hidden).origin, NodeOrigin::Measured(Source::Getattrlist));
    assert_eq!(data.disk_total_bytes, overview.volumes.container_size.value);

    let renderer_path = Path::new(domovoi_render::BUILT_METALLIB);
    let mut renderer = DiagramRenderer::system_default(renderer_path).expect("the renderer starts");
    let mut atlas = GlyphAtlas::default();
    let state = DiagramState::new(
        data.tree.clone(),
        DiagramMode::Sunburst,
        strings::DIAGRAM,
        &Palette::DARK,
        SIDE,
        SIDE,
    );
    let scene = state.scene(Instant::now(), 2.0, &mut atlas);
    let frame = renderer.render_offscreen(&scene, &atlas);
    let painted = frame.image.pixels.chunks(4).filter(|pixel| pixel[3] > 0).count();
    assert!(
        painted > frame.image.pixels.len() / 16,
        "the diagram of a scanned volume is drawn"
    );

    let mut reopened = Session::open(&mount_point, &paths, "0.0.0-test").expect("the session reopens");
    assert_eq!(reopened.open_cached().expect("the cache opens"), CacheOpening::Opened);
    assert_eq!(
        reopened.index().expect("the cache installs an index").len(),
        index.len()
    );
}

#[test]
fn a_cache_from_another_application_version_is_reset() {
    let tree = TempTree::new().expect("a temp tree is created");
    tree.root()
        .expect("the tree root opens")
        .file("payload.bin", MEGABYTE, Fill::Byte(1))
        .expect("a file is written");
    let paths = SessionPaths::in_directory(&tree.path().join("state"));
    let mut first = Session::open(tree.path(), &paths, "1.0.0").expect("the session opens");
    first.scan(None, CancelToken::default()).expect("the tree scans");
    let mut upgraded = Session::open(tree.path(), &paths, "1.0.1").expect("the session opens");
    assert_eq!(
        upgraded.open_cached().expect("the cache probe runs"),
        CacheOpening::Reset
    );
    assert!(upgraded.index().is_none());
}
