use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use domovoi_app::{
    AccessState, AiModels, CareModels, DiskOverview, Expansion, LevelRow, NODE_BUDGET, ScanStage, ScanStatus, Session,
    SessionPaths, ShellData, strings,
};
use domovoi_index::NodeId;
use domovoi_render::{DiagramLayout, DiagramMode, DiagramState, EffectiveSizes, NodeOrigin, ROOT};
use domovoi_scanner::CancelToken;
use domovoi_testkit::{ImageVolume, TempTree, Visibility, VolumeFormat};
use domovoi_theme::{Motion, Palette};

const MEGABYTE: usize = 1024 * 1024;
const SIDE: f32 = 440.0;
const ONLY_THE_ROOT_EXPANDS: usize = 1;
const ARCHIVE: &str = "Archive";
const PROJECTS: &str = "Projects";
const ARCHIVE_2019: &str = "Archive/2019";

struct ScannedVolume {
    _volume: ImageVolume,
    _state: TempTree,
    root: PathBuf,
    session: Session,
}

impl ScannedVolume {
    fn open() -> Self {
        let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("the image attaches");
        let root = fs::canonicalize(volume.mount_point()).expect("the mount point resolves");
        populate(&root);
        let state = TempTree::new().expect("a state folder is created");
        let mut session =
            Session::open(&root, &SessionPaths::in_directory(state.path()), "0.0.0-test").expect("the session opens");
        session.scan(None, CancelToken::default()).expect("the volume scans");
        Self {
            _volume: volume,
            _state: state,
            root,
            session,
        }
    }

    fn node(&self, relative: &str) -> NodeId {
        let index = self.session.index().expect("the volume is indexed");
        index
            .find(&self.root.join(relative))
            .unwrap_or_else(|| panic!("{relative} is indexed"))
    }

    fn drop_folder(&mut self, relative: &str) {
        let verdict = self.session.drop_into_tray(&[self.root.join(relative)]);
        assert_eq!(verdict.allowed, [self.node(relative)], "{relative} lands in the tray");
    }

    fn data(&self, budget: usize) -> ShellData {
        let index = self.session.index().expect("the volume is indexed");
        let overview = DiskOverview::measure(&self.root).expect("the volume is measured");
        let mut data = ShellData::measured(
            AccessState::Granted,
            &overview,
            Some(index),
            &Expansion::around(index, index.root(), budget),
            ScanStatus {
                bytes: 0,
                files: 0,
                sweep: 1.0,
                threads: self.session.workers(),
                current: String::new(),
                full_first_scan: false,
                stage: ScanStage::Walking,
            },
            CareModels::default(),
            AiModels::default(),
        );
        data.mark_tray(index, self.session.tray());
        data
    }

    fn position(&self, data: &ShellData, relative: &str) -> usize {
        data.tree_position(NodeOrigin::Indexed(self.node(relative)))
            .unwrap_or_else(|| panic!("{relative} is a node of the diagram"))
    }
}

fn populate(root: &Path) {
    for (folder, megabytes) in [("Projects/app", 6), ("Archive/2019", 4), ("Archive/2020", 2)] {
        let directory = root.join(folder);
        fs::create_dir_all(&directory).expect("a folder is created");
        fs::write(directory.join("payload.bin"), vec![7_u8; megabytes * MEGABYTE]).expect("a file is written");
    }
}

fn marked_names(rows: &[LevelRow]) -> Vec<&str> {
    rows.iter()
        .filter(|row| row.marked)
        .map(|row| row.name.as_str())
        .collect()
}

#[test]
fn a_dropped_folder_marks_its_row_and_leaves_its_neighbours() {
    let mut volume = ScannedVolume::open();
    volume.drop_folder(ARCHIVE);

    let data = volume.data(NODE_BUDGET);
    let rows = data.level_rows(ROOT);

    assert!(
        rows.iter().any(|row| row.name == PROJECTS),
        "the neighbour is a row of the level"
    );
    assert_eq!(marked_names(&rows), [ARCHIVE]);
}

#[test]
fn every_row_inside_a_marked_folder_is_marked() {
    let mut volume = ScannedVolume::open();
    volume.drop_folder(ARCHIVE);

    let data = volume.data(NODE_BUDGET);
    let rows = data.level_rows(volume.position(&data, ARCHIVE));

    assert_eq!(rows.len(), 2, "both years are rows of the marked folder");
    assert!(rows.iter().all(|row| row.marked), "{rows:?}");
}

#[test]
fn removing_the_folder_from_the_tray_leaves_no_marked_row() {
    let mut volume = ScannedVolume::open();
    volume.drop_folder(ARCHIVE);
    volume.session.remove_from_tray(volume.node(ARCHIVE));

    let data = volume.data(NODE_BUDGET);

    assert!(data.marked.is_empty());
    assert!(marked_names(&data.level_rows(ROOT)).is_empty());
    assert!(marked_names(&data.level_rows(volume.position(&data, ARCHIVE))).is_empty());
}

#[test]
fn the_marked_folder_collapses_its_segment_once_the_trash_collapse_ends() {
    let mut volume = ScannedVolume::open();
    volume.drop_folder(ARCHIVE);
    let data = volume.data(NODE_BUDGET);
    let archive = volume.position(&data, ARCHIVE);
    let mut diagram = DiagramState::new(
        data.tree.clone(),
        DiagramMode::Sunburst,
        strings::DIAGRAM,
        &Palette::DARK,
        SIDE,
        SIDE,
    );
    let start = Instant::now();

    diagram.set_marked(data.marked.clone(), start);

    assert!(diagram.is_animating(start), "the collapse runs");
    let end = start + Motion::TrashCollapse.duration(false);
    assert!(!diagram.is_animating(end), "the collapse ends after its budget");
    let DiagramLayout::Sunburst(layout) = &diagram.layout else {
        panic!("the diagram draws a sunburst");
    };
    assert!(
        layout.collapsed().any(|segment| segment.node == archive),
        "the marked folder is a collapsed segment"
    );
    assert!(
        diagram.segments().iter().all(|segment| segment.node != archive),
        "a collapsed segment is not an accessible segment"
    );
}

#[test]
fn an_item_below_the_shown_tree_shrinks_its_nearest_shown_folder_without_marking_it() {
    let mut volume = ScannedVolume::open();
    volume.drop_folder(ARCHIVE_2019);

    let data = volume.data(ONLY_THE_ROOT_EXPANDS);
    let year = volume.node(ARCHIVE_2019);
    let archive = volume.position(&data, ARCHIVE);

    assert_eq!(
        data.tree_position(NodeOrigin::Indexed(year)),
        None,
        "the year lies below the expanded part of the tree"
    );
    assert!(marked_names(&data.level_rows(ROOT)).is_empty());
    let index = volume.session.index().expect("the volume is indexed");
    let sizes = EffectiveSizes::of(&data.tree, &data.marked, ROOT);
    assert_eq!(
        sizes.size(archive),
        data.tree.node(archive).size - index.node(year).allocated
    );
    assert!(!sizes.is_collapsed(archive));
}
