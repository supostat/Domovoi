mod support;

use std::f32::consts::TAU;
use std::time::Instant;

use domovoi_index::Category;
use domovoi_render::{
    ArcSegment, DiagramLayout, DiagramMode, DiagramOutline, DiagramState, DiagramTree, EffectiveSizes, FontFamily,
    GROUP_MIN_HEIGHT, GROUP_MIN_WIDTH, Hit, LABEL_MIN_HEIGHT, LABEL_MIN_WIDTH, MarkedNodes, NodeOrigin, Point,
    RING_COUNT, ROOT, SMALL_GROUP_SPAN, SegmentKind, SmallGroups, SunburstLayout, TextStyle, TileRole, TreemapLayout,
    VISIBLE_SIDE, center_text, fit_end,
};
use domovoi_theme::Palette;
use support::{SMALL_FILE_BYTES, SMALL_FILES, STRINGS, book_v, fonts, small_group_tree};

const SUNBURST_SIZE: f32 = 460.0;
const TREEMAP_WIDTH: f32 = 820.0;
const TREEMAP_HEIGHT: f32 = 480.0;
const ANGLE_TOLERANCE: f32 = 1e-4;
const PATH: &str = "~/Library/Application Support/MobileSync/Backup/00008110-001A2C3E4F56/Manifest.db";
const NAME: &str = "com.apple.dt.Xcode.DerivedData.Facet-gxkqzjwbvdemafbtyhlu";
const LONG_NAME: &str = "Final Cut · библиотека проекта «Отпуск в Карелии 2026»";

fn span(segment: &ArcSegment) -> f32 {
    segment.end_angle - segment.start_angle
}

fn closed() -> SmallGroups {
    SmallGroups::default()
}

#[test]
fn every_ring_of_the_sunburst_closes_the_full_circle_and_maps_to_the_node_depth() {
    let tree = book_v();
    let layout = SunburstLayout::compute(&tree, ROOT, SUNBURST_SIZE, 1.0, &closed(), &MarkedNodes::default());
    assert!(
        (layout.ring_angle_sum(0) - TAU).abs() < ANGLE_TOLERANCE,
        "{}",
        layout.ring_angle_sum(0)
    );
    for segment in &layout.segments {
        let depth = tree.node(segment.node).depth;
        match segment.kind {
            SegmentKind::Node => {
                assert_eq!(segment.ring + 1, depth, "{}", tree.node(segment.node).name);
                assert!(
                    span(segment) > SMALL_GROUP_SPAN,
                    "a node arc narrower than the threshold joins the small group"
                );
            }
            SegmentKind::SmallGroup { count, bytes } => {
                assert_eq!(segment.ring, depth, "the group sits among the children of its parent");
                assert!(count > 0 && bytes > 0);
            }
            SegmentKind::Collapsed => panic!("nothing is marked in this layout"),
        }
        assert!(segment.ring < RING_COUNT);
        let (start, end) = segment.drawn_angles();
        assert!(start < end, "the padding never inverts a segment");
    }
    let mut parents: Vec<usize> = layout.groups().map(|group| group.node).collect();
    let group_count = parents.len();
    parents.sort_unstable();
    parents.dedup();
    assert_eq!(parents.len(), group_count, "one group per directory");
    for group in layout.groups() {
        let parent_span = if group.node == layout.focus {
            TAU
        } else {
            let parent = layout
                .segments
                .iter()
                .find(|segment| segment.kind == SegmentKind::Node && segment.node == group.node)
                .expect("the parent of a group is laid out");
            span(parent)
        };
        let nodes_span: f32 = layout
            .segments
            .iter()
            .filter(|segment| segment.kind == SegmentKind::Node && tree.node(segment.node).parent == Some(group.node))
            .map(span)
            .sum();
        assert!(
            (parent_span - nodes_span - span(group)).abs() < ANGLE_TOLERANCE,
            "the group takes exactly the remainder of {}",
            tree.node(group.node).name
        );
    }
    let focus = tree.node(ROOT).children[0];
    let zoomed = SunburstLayout::compute(&tree, focus, SUNBURST_SIZE, 1.0, &closed(), &MarkedNodes::default());
    assert!((zoomed.ring_angle_sum(0) - TAU).abs() < ANGLE_TOLERANCE);
    assert!(
        zoomed
            .segments
            .iter()
            .filter(|segment| segment.kind == SegmentKind::Node)
            .all(|segment| segment.ring + 2 == tree.node(segment.node).depth)
    );
}

#[test]
fn small_children_fold_into_one_group_that_opens_on_demand() {
    let tree = small_group_tree();
    let large_folder = tree.node(ROOT).children[0];
    let closed_layout = SunburstLayout::compute(&tree, ROOT, SUNBURST_SIZE, 1.0, &closed(), &MarkedNodes::default());
    let group = *closed_layout.groups().next().expect("the small files form a group");
    assert_eq!(closed_layout.groups().count(), 1);
    assert_eq!((group.node, group.ring), (ROOT, 0));
    assert_eq!(
        group.kind,
        SegmentKind::SmallGroup {
            count: SMALL_FILES,
            bytes: SMALL_FILES as u64 * SMALL_FILE_BYTES,
        }
    );
    assert_eq!(
        closed_layout
            .segments
            .iter()
            .filter(|segment| segment.ring == 0)
            .count(),
        2,
        "the large folder and the group"
    );
    assert!((closed_layout.ring_angle_sum(0) - TAU).abs() < ANGLE_TOLERANCE);
    let (angle, radius) = group.middle();
    let middle = closed_layout.point_at(angle, radius);
    assert_eq!(closed_layout.hit_test(middle), Some(Hit::Group(ROOT)));

    let mut opened = closed();
    opened.open(ROOT);
    let open_layout = SunburstLayout::compute(&tree, ROOT, SUNBURST_SIZE, 1.0, &opened, &MarkedNodes::default());
    assert_eq!(open_layout.groups().count(), 0);
    assert_eq!(
        open_layout.segments.iter().filter(|segment| segment.ring == 0).count(),
        SMALL_FILES + 1
    );
    assert!((open_layout.ring_angle_sum(0) - TAU).abs() < ANGLE_TOLERANCE);
    let members: f32 = open_layout
        .segments
        .iter()
        .filter(|segment| segment.ring == 0 && segment.node != large_folder)
        .map(span)
        .sum();
    assert!(
        (members - span(&group)).abs() < ANGLE_TOLERANCE,
        "the members fill exactly the arc of the group"
    );
    let always = SunburstLayout::compute(
        &tree,
        ROOT,
        SUNBURST_SIZE,
        1.0,
        &SmallGroups::always(true),
        &MarkedNodes::default(),
    );
    assert_eq!(always.segments, open_layout.segments);

    let mut state = DiagramState::new(
        tree.clone(),
        DiagramMode::Sunburst,
        STRINGS,
        &Palette::DARK,
        SUNBURST_SIZE,
        SUNBURST_SIZE,
    );
    state.click(middle, Instant::now());
    assert!(state.small_groups.is_open(ROOT), "a click opens the group");
    assert_eq!(state.focus(), ROOT, "opening a group keeps the level");
    let DiagramLayout::Sunburst(after_click) = &state.layout else {
        panic!("the state stays a sunburst");
    };
    assert_eq!(after_click.segments, open_layout.segments);
}

#[test]
fn the_scan_sector_follows_the_actual_progress() {
    for tree in [book_v(), small_group_tree()] {
        let half = SunburstLayout::compute(&tree, ROOT, SUNBURST_SIZE, 0.5, &closed(), &MarkedNodes::default());
        assert!(
            half.segments
                .iter()
                .all(|segment| segment.end_angle <= TAU / 2.0 + ANGLE_TOLERANCE)
        );
        assert!((half.ring_angle_sum(0) - TAU / 2.0).abs() < ANGLE_TOLERANCE);
    }
}

#[test]
fn hit_testing_inverts_both_layouts() {
    let tree = book_v();
    let sunburst = SunburstLayout::compute(&tree, ROOT, SUNBURST_SIZE, 1.0, &closed(), &MarkedNodes::default());
    for segment in &sunburst.segments {
        let (angle, radius) = segment.middle();
        assert_eq!(sunburst.hit_test(sunburst.point_at(angle, radius)), Some(segment.hit()));
    }
    assert_eq!(
        sunburst.hit_test(sunburst.center()),
        None,
        "the center belongs to no segment"
    );
    let treemap = TreemapLayout::compute(&tree, ROOT, TREEMAP_WIDTH, TREEMAP_HEIGHT, &MarkedNodes::default());
    for tile in &treemap.tiles {
        let deeper_inside = treemap.tiles.iter().any(|other| {
            other.role == TileRole::Nested
                && tile.role == TileRole::Group
                && tile.rect.contains(other.rect.center())
                && other.rect.contains(tile.rect.center())
        });
        if !deeper_inside {
            let hit = treemap.hit_test(tile.rect.center());
            assert!(
                hit == Some(tile.node) || tile.role == TileRole::Group,
                "{}",
                tree.node(tile.node).name
            );
        }
    }
    assert_eq!(treemap.hit_test(Point { x: -1.0, y: -1.0 }), None);
}

#[test]
fn squarified_tiles_stay_inside_their_bounds_without_overlap_and_repeat_exactly() {
    let tree = book_v();
    let first = TreemapLayout::compute(&tree, ROOT, TREEMAP_WIDTH, TREEMAP_HEIGHT, &MarkedNodes::default());
    let second = TreemapLayout::compute(&tree, ROOT, TREEMAP_WIDTH, TREEMAP_HEIGHT, &MarkedNodes::default());
    assert_eq!(first, second, "the layout is deterministic");
    let top: Vec<_> = first
        .tiles
        .iter()
        .filter(|tile| tile.role != TileRole::Nested)
        .collect();
    let nested: Vec<_> = first
        .tiles
        .iter()
        .filter(|tile| tile.role == TileRole::Nested)
        .collect();
    for group in [&top, &nested] {
        for (position, tile) in group.iter().enumerate() {
            assert!(tile.rect.x >= 0.0 && tile.rect.y >= 0.0);
            assert!(tile.rect.x + tile.rect.width <= TREEMAP_WIDTH + 0.01);
            assert!(tile.rect.y + tile.rect.height <= TREEMAP_HEIGHT + 0.01);
            assert!(tile.rect.width >= 1.0 && tile.rect.height >= 1.0);
            for other in &group[position + 1..] {
                assert!(
                    tile.rect.overlap_area(&other.rect) < 0.01,
                    "{} overlaps {}",
                    tree.node(tile.node).name,
                    tree.node(other.node).name
                );
            }
        }
    }
    for tile in &top {
        assert!(tile.rect.width >= VISIBLE_SIDE && tile.rect.height >= VISIBLE_SIDE);
    }
    for tile in &nested {
        let parent = tree.node(tile.node).parent.expect("a nested tile has a parent");
        let frame = first.tile_of(parent).expect("the group is laid out").rect;
        assert!(tile.rect.x >= frame.x && tile.rect.x + tile.rect.width <= frame.x + frame.width + 0.01);
    }
}

#[test]
fn treemap_labels_appear_only_above_the_thresholds_and_are_cut_at_the_end() {
    fonts();
    let tree = book_v();
    let layout = TreemapLayout::compute(&tree, ROOT, TREEMAP_WIDTH, TREEMAP_HEIGHT, &MarkedNodes::default());
    for tile in &layout.tiles {
        match tile.role {
            TileRole::Group => {
                assert!(tile.rect.width > GROUP_MIN_WIDTH && tile.rect.height > GROUP_MIN_HEIGHT);
                assert!(tile.label.is_some_and(|label| label.heading));
            }
            TileRole::Leaf | TileRole::Nested => {
                let large = tile.rect.width > LABEL_MIN_WIDTH && tile.rect.height > LABEL_MIN_HEIGHT;
                assert_eq!(tile.label.is_some(), large, "{}", tree.node(tile.node).name);
            }
        }
    }
    let style = TextStyle {
        family: FontFamily::Interface,
        weight: 600.0,
        size: 11.5,
    };
    let cut = fit_end(LONG_NAME, style, 80.0);
    assert!(cut.truncated && cut.width() <= 80.0);
    let whole = fit_end("Docker", style, 80.0);
    assert!(!whole.truncated);
}

#[test]
fn the_sun_center_shrinks_to_the_minimum_before_the_ellipsis() {
    fonts();
    let tree = book_v();
    for size in (140..=560).step_by(20) {
        let layout = SunburstLayout::compute(&tree, ROOT, size as f32, 1.0, &closed(), &MarkedNodes::default());
        for text in [PATH, NAME, LONG_NAME] {
            for line in center_text(&layout, text, text, text) {
                let width = line.line.width();
                assert!(
                    width <= line.max_width + 0.5,
                    "{text} at {size}: {width} > {}",
                    line.max_width
                );
                assert!(line.line.style.size + 0.01 >= line.minimum_size);
                if line.line.truncated {
                    assert!(
                        (line.line.style.size - line.minimum_size).abs() < 0.01,
                        "{text} at {size} was cut before reaching the minimum size"
                    );
                }
            }
        }
    }
}

const MEDIA: u64 = 600;
const MEDIA_ORIGINALS: u64 = 400;
const MEDIA_EXPORTS: u64 = 200;
const DOCUMENTS: u64 = 300;
const PROJECTS: u64 = 100;
const DISK: u64 = MEDIA + DOCUMENTS + PROJECTS;
const OUT_OF_TREE: u64 = 50;

struct MarkedFixture {
    tree: DiagramTree,
    media: usize,
    originals: usize,
    exports: usize,
    documents: usize,
    projects: usize,
}

impl MarkedFixture {
    fn new() -> Self {
        let leaf = |name: &str, size: u64, category: Category| DiagramOutline {
            name: name.to_owned(),
            category,
            size,
            origin: NodeOrigin::Outline,
            children: Vec::new(),
        };
        let tree = DiagramTree::from_outline(&DiagramOutline {
            name: "Macintosh HD".to_owned(),
            category: Category::Home,
            size: 0,
            origin: NodeOrigin::Outline,
            children: vec![
                DiagramOutline {
                    name: "Media".to_owned(),
                    category: Category::Media,
                    size: 0,
                    origin: NodeOrigin::Outline,
                    children: vec![
                        leaf("Originals", MEDIA_ORIGINALS, Category::Media),
                        leaf("Exports", MEDIA_EXPORTS, Category::Media),
                    ],
                },
                leaf("Documents", DOCUMENTS, Category::Docs),
                leaf("Projects", PROJECTS, Category::Dev),
            ],
        });
        let child = |parent: usize, name: &str| {
            tree.node(parent)
                .children
                .iter()
                .copied()
                .find(|child| tree.node(*child).name == name)
                .expect("the fixture names its children")
        };
        let media = child(ROOT, "Media");
        let originals = child(media, "Originals");
        let exports = child(media, "Exports");
        let documents = child(ROOT, "Documents");
        let projects = child(ROOT, "Projects");
        Self {
            tree,
            media,
            originals,
            exports,
            documents,
            projects,
        }
    }

    fn sunburst(&self, focus: usize, marked: &MarkedNodes) -> SunburstLayout {
        SunburstLayout::compute(&self.tree, focus, SUNBURST_SIZE, 1.0, &closed(), marked)
    }
}

fn marked(nodes: &[usize]) -> MarkedNodes {
    let mut marked = MarkedNodes::default();
    for node in nodes {
        marked.mark(*node);
    }
    marked
}

fn segment_of(layout: &SunburstLayout, node: usize) -> &ArcSegment {
    layout
        .segments
        .iter()
        .find(|segment| segment.node == node && !matches!(segment.kind, SegmentKind::SmallGroup { .. }))
        .expect("the node has a segment")
}

fn share(bytes: u64, total: u64) -> f32 {
    TAU * bytes as f32 / total as f32
}

#[test]
fn marked_bytes_leave_every_ancestor_and_the_siblings_share_the_circle() {
    let fixture = MarkedFixture::new();
    let marked = marked(&[fixture.originals]);
    let sizes = EffectiveSizes::of(&fixture.tree, &marked, ROOT);
    assert_eq!(sizes.size(fixture.originals), 0);
    assert_eq!(sizes.size(fixture.media), MEDIA - MEDIA_ORIGINALS);
    assert_eq!(sizes.size(ROOT), DISK - MEDIA_ORIGINALS);
    assert_eq!(sizes.size(fixture.documents), DOCUMENTS);

    let layout = fixture.sunburst(ROOT, &marked);
    let remaining = DISK - MEDIA_ORIGINALS;
    for (node, bytes) in [
        (fixture.media, MEDIA_EXPORTS),
        (fixture.documents, DOCUMENTS),
        (fixture.projects, PROJECTS),
    ] {
        let segment = segment_of(&layout, node);
        assert!(
            (span(segment) - share(bytes, remaining)).abs() < ANGLE_TOLERANCE,
            "{}",
            fixture.tree.node(node).name
        );
    }
    assert!((layout.ring_angle_sum(0) - TAU).abs() < ANGLE_TOLERANCE);
    let exports = segment_of(&layout, fixture.exports);
    assert!(
        (span(exports) - span(segment_of(&layout, fixture.media))).abs() < ANGLE_TOLERANCE,
        "the remaining child takes the whole sector of its parent"
    );
}

#[test]
fn marked_node_collapses_to_zero_angle_and_the_children_keep_their_order() {
    let fixture = MarkedFixture::new();
    let layout = fixture.sunburst(ROOT, &marked(&[fixture.media]));
    let ring: Vec<(usize, SegmentKind)> = layout
        .segments
        .iter()
        .filter(|segment| segment.ring == 0)
        .map(|segment| (segment.node, segment.kind))
        .collect();
    assert_eq!(
        ring,
        [
            (fixture.media, SegmentKind::Collapsed),
            (fixture.documents, SegmentKind::Node),
            (fixture.projects, SegmentKind::Node),
        ],
        "the tree order stays even though Media is now the smallest"
    );
    let media = segment_of(&layout, fixture.media);
    assert!(span(media).abs() < f32::EPSILON);
    assert!(
        layout
            .segments
            .iter()
            .all(|segment| segment.node != fixture.originals && segment.node != fixture.exports),
        "the children of a collapsed node leave the layout and shrink into its point while the frame moves"
    );
    assert_eq!(layout.collapsed().count(), 1);
}

#[test]
fn marked_focus_keeps_the_sizes_of_its_level() {
    let fixture = MarkedFixture::new();
    let marked = marked(&[fixture.media]);
    let sizes = EffectiveSizes::of(&fixture.tree, &marked, fixture.media);
    assert_eq!(sizes.size(fixture.originals), MEDIA_ORIGINALS);
    assert_eq!(sizes.size(fixture.media), MEDIA);
    let layout = fixture.sunburst(fixture.media, &marked);
    assert_eq!(layout.collapsed().count(), 0);
    let originals = segment_of(&layout, fixture.originals);
    assert!((span(originals) - share(MEDIA_ORIGINALS, MEDIA)).abs() < ANGLE_TOLERANCE);
    assert!(marked.covers(&fixture.tree, fixture.originals));
}

#[test]
fn marked_segment_takes_no_pointer_and_no_accessibility_element() {
    let fixture = MarkedFixture::new();
    let marked = marked(&[fixture.documents]);
    let layout = fixture.sunburst(ROOT, &marked);
    let documents = segment_of(&layout, fixture.documents);
    let radius = f32::midpoint(documents.inner_radius, documents.outer_radius);
    for angle in [documents.start_angle, documents.start_angle + ANGLE_TOLERANCE] {
        assert_ne!(
            layout.hit_test(layout.point_at(angle, radius)),
            Some(Hit::Node(fixture.documents))
        );
    }

    let mut state = DiagramState::new(
        fixture.tree.clone(),
        DiagramMode::Sunburst,
        STRINGS,
        &Palette::DARK,
        SUNBURST_SIZE,
        SUNBURST_SIZE,
    );
    state.jump_marked(marked);
    assert!(
        state
            .segments()
            .iter()
            .all(|segment| segment.is_center || segment.node != fixture.documents),
        "a collapsed segment is not offered to VoiceOver"
    );
    assert_eq!(state.segments().len(), 3, "Media, Projects and the center");
}

#[test]
fn marked_treemap_tiles_share_the_former_bounds_without_the_marked_one() {
    let fixture = MarkedFixture::new();
    let marked = marked(&[fixture.documents]);
    let layout = TreemapLayout::compute(&fixture.tree, ROOT, TREEMAP_WIDTH, TREEMAP_HEIGHT, &marked);
    assert!(layout.tile_of(fixture.documents).is_none());
    for tile in &layout.tiles {
        assert!(tile.rect.x >= 0.0 && tile.rect.y >= 0.0);
        assert!(tile.rect.x + tile.rect.width <= TREEMAP_WIDTH + ANGLE_TOLERANCE);
        assert!(tile.rect.y + tile.rect.height <= TREEMAP_HEIGHT + ANGLE_TOLERANCE);
    }
    let area = |node: usize| {
        let rect = layout.tile_of(node).expect("the tile exists").rect;
        rect.width * rect.height
    };
    let ratio = area(fixture.media) / area(fixture.projects);
    let unmarked = TreemapLayout::compute(
        &fixture.tree,
        ROOT,
        TREEMAP_WIDTH,
        TREEMAP_HEIGHT,
        &MarkedNodes::default(),
    );
    let unmarked_media = unmarked.tile_of(fixture.media).expect("Media has a tile").rect;
    assert!(
        area(fixture.media) > unmarked_media.width * unmarked_media.height,
        "Media grows into the place of the marked tile"
    );
    assert!(ratio > 4.0, "Media still outweighs Projects six to one: {ratio}");
}

#[test]
fn marked_partial_deduction_shrinks_the_node_and_its_ancestors_without_collapsing_it() {
    let fixture = MarkedFixture::new();
    let mut marked = MarkedNodes::default();
    marked.deduct(fixture.exports, OUT_OF_TREE);
    let sizes = EffectiveSizes::of(&fixture.tree, &marked, ROOT);
    assert_eq!(sizes.size(fixture.exports), MEDIA_EXPORTS - OUT_OF_TREE);
    assert_eq!(sizes.size(fixture.media), MEDIA - OUT_OF_TREE);
    assert_eq!(sizes.size(ROOT), DISK - OUT_OF_TREE);
    assert!(!sizes.is_collapsed(fixture.exports));
    assert!(!marked.covers(&fixture.tree, fixture.exports));

    let layout = fixture.sunburst(ROOT, &marked);
    assert_eq!(layout.collapsed().count(), 0);
    let exports = segment_of(&layout, fixture.exports);
    assert_eq!(exports.kind, SegmentKind::Node);
    assert!((span(exports) - share(MEDIA_EXPORTS - OUT_OF_TREE, DISK - OUT_OF_TREE)).abs() < ANGLE_TOLERANCE);
}
