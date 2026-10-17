mod support;

use std::f32::consts::TAU;

use domovoi_render::{
    FontFamily, GROUP_MIN_HEIGHT, GROUP_MIN_WIDTH, LABEL_MIN_HEIGHT, LABEL_MIN_WIDTH, Point, RING_COUNT, ROOT,
    SunburstLayout, TextStyle, TileRole, TreemapLayout, VISIBLE_SIDE, center_text, fit_end,
};
use support::{book_v, fonts};

const SUNBURST_SIZE: f32 = 460.0;
const TREEMAP_WIDTH: f32 = 820.0;
const TREEMAP_HEIGHT: f32 = 480.0;
const ANGLE_TOLERANCE: f32 = 1e-4;
const PATH: &str = "~/Library/Application Support/MobileSync/Backup/00008110-001A2C3E4F56/Manifest.db";
const NAME: &str = "com.apple.dt.Xcode.DerivedData.Facet-gxkqzjwbvdemafbtyhlu";
const LONG_NAME: &str = "Final Cut · библиотека проекта «Отпуск в Карелии 2026»";

#[test]
fn every_ring_of_the_sunburst_closes_the_full_circle_and_maps_to_the_node_depth() {
    let tree = book_v();
    let layout = SunburstLayout::compute(&tree, ROOT, SUNBURST_SIZE, 1.0);
    assert!(
        (layout.ring_angle_sum(0) - TAU).abs() < ANGLE_TOLERANCE,
        "{}",
        layout.ring_angle_sum(0)
    );
    for segment in &layout.segments {
        assert_eq!(
            segment.ring + 1,
            tree.node(segment.node).depth,
            "{}",
            tree.node(segment.node).name
        );
        assert!(segment.ring < RING_COUNT);
        assert!(
            segment.end_angle - segment.start_angle > 0.006,
            "level of detail drops slivers"
        );
        let (start, end) = segment.drawn_angles();
        assert!(start < end, "the padding never inverts a segment");
    }
    let focus = tree.node(ROOT).children[0];
    let zoomed = SunburstLayout::compute(&tree, focus, SUNBURST_SIZE, 1.0);
    assert!((zoomed.ring_angle_sum(0) - TAU).abs() < ANGLE_TOLERANCE);
    assert!(
        zoomed
            .segments
            .iter()
            .all(|segment| segment.ring + 2 == tree.node(segment.node).depth)
    );
}

#[test]
fn the_scan_sector_follows_the_actual_progress() {
    let tree = book_v();
    let half = SunburstLayout::compute(&tree, ROOT, SUNBURST_SIZE, 0.5);
    assert!(
        half.segments
            .iter()
            .all(|segment| segment.end_angle <= TAU / 2.0 + ANGLE_TOLERANCE)
    );
    assert!((half.ring_angle_sum(0) - TAU / 2.0).abs() < ANGLE_TOLERANCE);
}

#[test]
fn hit_testing_inverts_both_layouts() {
    let tree = book_v();
    let sunburst = SunburstLayout::compute(&tree, ROOT, SUNBURST_SIZE, 1.0);
    for segment in &sunburst.segments {
        let (angle, radius) = segment.middle();
        assert_eq!(sunburst.hit_test(sunburst.point_at(angle, radius)), Some(segment.node));
    }
    assert_eq!(
        sunburst.hit_test(sunburst.center()),
        None,
        "the center belongs to no segment"
    );
    let treemap = TreemapLayout::compute(&tree, ROOT, TREEMAP_WIDTH, TREEMAP_HEIGHT);
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
    let first = TreemapLayout::compute(&tree, ROOT, TREEMAP_WIDTH, TREEMAP_HEIGHT);
    let second = TreemapLayout::compute(&tree, ROOT, TREEMAP_WIDTH, TREEMAP_HEIGHT);
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
    let layout = TreemapLayout::compute(&tree, ROOT, TREEMAP_WIDTH, TREEMAP_HEIGHT);
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
        let layout = SunburstLayout::compute(&tree, ROOT, size as f32, 1.0);
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
