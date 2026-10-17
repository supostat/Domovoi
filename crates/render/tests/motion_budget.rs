mod support;

use std::time::{Duration, Instant};

use domovoi_render::{DiagramLayout, DiagramMode, DiagramState, GlyphAtlas, HOVER_DIM_OPACITY, ROOT, Transition};
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
    assert_eq!(state.hovered, Some(hovered.node));
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
