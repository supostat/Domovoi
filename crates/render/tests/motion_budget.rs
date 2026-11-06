mod support;

use std::time::{Duration, Instant};

use domovoi_render::{
    DiagramLayout, DiagramMode, DiagramState, GlyphAtlas, HOVER_DIM_OPACITY, MarkedNodes, ROOT, SegmentKind,
    ShapeInstance, SunburstLayout, Transition,
};
use domovoi_theme::{Motion, Palette};
use support::{STRINGS, book_v, fonts};

const SIDE: f32 = 460.0;
const BUDGETS: [(Motion, u64); 4] = [
    (Motion::LevelZoom, 220),
    (Motion::HoverDim, 120),
    (Motion::TrashCollapse, 320),
    (Motion::CleanupOrnament, 1400),
];

fn sunburst_state() -> DiagramState {
    DiagramState::new(book_v(), DiagramMode::Sunburst, STRINGS, &Palette::DARK, SIDE, SIDE)
}

#[test]
fn every_motion_keeps_its_budget_and_reduce_motion_swaps_the_frame() {
    let now = Instant::now();
    for (motion, milliseconds) in BUDGETS {
        let transition = Transition::start(motion, now, false);
        assert_eq!(transition.duration, Duration::from_millis(milliseconds), "{motion:?}");
        assert!(transition.progress(now + Duration::from_millis(milliseconds / 2)) < 1.0);
        assert!((transition.progress(now + Duration::from_millis(milliseconds)) - 1.0).abs() < f32::EPSILON);
        let reduced = Transition::start(motion, now, true);
        assert_eq!(reduced.duration, Duration::ZERO);
        assert!(
            (reduced.progress(now) - 1.0).abs() < f32::EPSILON,
            "reduce motion jumps to the final frame"
        );
        assert!(reduced.is_finished(now));
    }
}

#[test]
fn a_level_zoom_animates_for_220_ms_and_reduce_motion_skips_it() {
    fonts();
    let mut state = sunburst_state();
    let child = state.tree.node(ROOT).children[0];
    let started = Instant::now();
    state.set_focus(child, started);
    let mut atlas = GlyphAtlas::default();
    assert!(state.is_animating(started + Duration::from_millis(100)));
    let halfway = state
        .scene(started + Duration::from_millis(110), 1.0, &mut atlas)
        .progress;
    assert!(halfway > 0.0 && halfway < 1.0, "{halfway}");
    assert!(!state.is_animating(started + Duration::from_millis(221)));
    assert!(
        (state
            .scene(started + Duration::from_millis(220), 1.0, &mut atlas)
            .progress
            - 1.0)
            .abs()
            < f32::EPSILON
    );

    let mut calm = sunburst_state();
    calm.reduce_motion = true;
    calm.set_focus(child, started);
    assert!(!calm.is_animating(started));
    assert!((calm.scene(started, 1.0, &mut atlas).progress - 1.0).abs() < f32::EPSILON);
}

#[test]
fn hovering_dims_the_other_segments_to_45_percent_in_120_ms() {
    fonts();
    let mut state = sunburst_state();
    let DiagramLayout::Sunburst(layout) = state.layout.clone() else {
        panic!("the state starts as a sunburst");
    };
    let hovered = layout
        .segments
        .iter()
        .find(|segment| segment.ring == 0)
        .expect("a first ring exists");
    let other = layout
        .segments
        .iter()
        .position(|segment| segment.ring == 0 && segment.node != hovered.node)
        .expect("a second segment exists");
    let (angle, radius) = hovered.middle();
    let started = Instant::now();
    let mut atlas = GlyphAtlas::default();
    let resting = state.scene(started, 1.0, &mut atlas).shapes[other].fill[3];
    state.hover(Some(layout.point_at(angle, radius)), started);
    assert_eq!(state.hovered, Some(hovered.hit()));
    let halfway = state.scene(started + Duration::from_millis(30), 1.0, &mut atlas).shapes[other].fill[3];
    let dimmed = state
        .scene(started + Duration::from_millis(120), 1.0, &mut atlas)
        .shapes[other]
        .fill[3];
    assert!(
        halfway < resting && halfway > dimmed,
        "{resting} → {halfway} → {dimmed}"
    );
    assert!(
        (dimmed - resting * HOVER_DIM_OPACITY).abs() < 1e-4,
        "{dimmed} is not 45 % of {resting}"
    );
    assert!(!state.is_animating(started + Duration::from_millis(121)));
}

const HALF_COLLAPSE: Duration = Duration::from_millis(160);
const FULL_COLLAPSE: Duration = Duration::from_millis(320);
const GEOMETRY_TOLERANCE: f32 = 1e-6;

fn close(left: f32, right: f32) -> bool {
    (left - right).abs() < GEOMETRY_TOLERANCE
}

fn same(left: [f32; 4], right: [f32; 4]) -> bool {
    left.iter().zip(right).all(|(left, right)| close(*left, right))
}

fn sunburst(state: &DiagramState) -> SunburstLayout {
    let DiagramLayout::Sunburst(layout) = state.layout.clone() else {
        panic!("the state is a sunburst");
    };
    layout
}

fn folder_with_children(state: &DiagramState) -> usize {
    let layout = sunburst(state);
    layout
        .segments
        .iter()
        .find(|segment| {
            segment.ring == 0
                && segment.kind == SegmentKind::Node
                && layout
                    .segments
                    .iter()
                    .any(|child| child.ring == 1 && state.tree.node(child.node).parent == Some(segment.node))
        })
        .expect("a first-ring folder shows its children")
        .node
}

fn marking(node: usize) -> MarkedNodes {
    let mut marked = MarkedNodes::default();
    marked.mark(node);
    marked
}

fn collapse_point(state: &DiagramState, node: usize) -> (f32, f32) {
    let segment = *sunburst(state)
        .collapsed()
        .find(|segment| segment.node == node)
        .expect("the marked node collapses");
    (segment.start_angle, segment.inner_radius)
}

fn shrinking_into(shapes: &[ShapeInstance], angle: f32) -> Vec<&ShapeInstance> {
    shapes
        .iter()
        .filter(|shape| {
            close(shape.geometry[0], angle) && close(shape.geometry[1], angle) && !same(shape.previous, shape.geometry)
        })
        .collect()
}

#[test]
fn marked_segment_collapses_toward_the_center_in_320_ms_with_its_children() {
    fonts();
    let mut state = sunburst_state();
    let folder = folder_with_children(&state);
    let before = sunburst(&state);
    let drawn = before
        .segments
        .iter()
        .find(|segment| segment.node == folder && segment.kind == SegmentKind::Node)
        .expect("the folder is drawn")
        .drawn_angles();
    let started = Instant::now();
    state.set_marked(marking(folder), started);
    let (angle, inner) = collapse_point(&state, folder);
    let mut atlas = GlyphAtlas::default();

    assert!(state.is_animating(started + HALF_COLLAPSE));
    let halfway = state.scene(started + HALF_COLLAPSE, 1.0, &mut atlas);
    assert!(halfway.progress > 0.0 && halfway.progress < 1.0, "{}", halfway.progress);
    let shrinking = shrinking_into(&halfway.shapes, angle);
    let own = shrinking
        .iter()
        .find(|shape| close(shape.previous[0], drawn.0) && close(shape.previous[1], drawn.1))
        .expect("the folder shrinks from its former arc");
    assert!(
        same(own.geometry, [angle, angle, inner, inner]),
        "it ends as a point on its inner radius: {:?}",
        own.geometry
    );
    assert!(
        shrinking.len() > 1,
        "the children in the outer rings shrink into the same point instead of vanishing"
    );

    assert!(!state.is_animating(started + FULL_COLLAPSE));
    let settled = state.scene(started + FULL_COLLAPSE, 1.0, &mut atlas);
    assert!((settled.progress - 1.0).abs() < f32::EPSILON);
    assert!(
        shrinking_into(&settled.shapes, angle).is_empty(),
        "the collapsed segment and its children are gone from the final frame"
    );
}

#[test]
fn marked_collapse_swaps_the_frame_under_reduce_motion() {
    fonts();
    let mut state = sunburst_state();
    state.reduce_motion = true;
    let folder = folder_with_children(&state);
    let started = Instant::now();
    state.set_marked(marking(folder), started);
    let (angle, _) = collapse_point(&state, folder);
    assert!(!state.is_animating(started));
    let scene = state.scene(started, 1.0, &mut GlyphAtlas::default());
    assert!((scene.progress - 1.0).abs() < f32::EPSILON);
    assert!(shrinking_into(&scene.shapes, angle).is_empty());
}

#[test]
fn marked_segment_grows_back_when_the_mark_is_removed() {
    fonts();
    let mut state = sunburst_state();
    let folder = folder_with_children(&state);
    let resting = sunburst(&state);
    let started = Instant::now();
    state.set_marked(marking(folder), started);
    let (angle, inner) = collapse_point(&state, folder);
    let unmarked = started + FULL_COLLAPSE + Duration::from_millis(10);
    state.set_marked(MarkedNodes::default(), unmarked);
    assert_eq!(
        sunburst(&state),
        resting,
        "removing the mark restores the former layout"
    );

    let mut atlas = GlyphAtlas::default();
    let halfway = state.scene(unmarked + HALF_COLLAPSE, 1.0, &mut atlas);
    let growing: Vec<&ShapeInstance> = halfway
        .shapes
        .iter()
        .filter(|shape| close(shape.previous[0], angle) && close(shape.previous[1], angle))
        .collect();
    let drawn = resting
        .segments
        .iter()
        .find(|segment| segment.node == folder && segment.kind == SegmentKind::Node)
        .expect("the folder is drawn again")
        .drawn_angles();
    assert!(
        growing
            .iter()
            .any(|shape| same(shape.previous, [angle, angle, inner, inner]) && close(shape.geometry[0], drawn.0)),
        "the folder grows out of its collapse point"
    );
    assert!(growing.len() > 1, "its children grow out of the same point");
    assert!(!state.is_animating(unmarked + FULL_COLLAPSE));
}
