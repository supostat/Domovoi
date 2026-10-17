mod support;

use std::process::ExitCode;

use domovoi_render::{
    AccessibleSegment, DiagramGraphics, DiagramLayout, DiagramMode, DiagramState, DiagramTree, DiagramView, GlyphAtlas,
    ROOT, TileRole,
};
use domovoi_theme::{Palette, format_size};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, msg_send};
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType};
use objc2_foundation::{NSArray, NSPoint, NSRect, NSSize, NSString};
use support::{STRINGS, book_v, fonts, renderer};

const SIDE: f64 = 460.0;
const RIGHT_ARROW: u16 = 124;
const LEFT_ARROW: u16 = 123;
const RETURN: u16 = 36;

struct Checks {
    failures: Vec<String>,
    count: usize,
}

impl Checks {
    fn expect(&mut self, condition: bool, description: impl Into<String>) {
        self.count += 1;
        if !condition {
            self.failures.push(description.into());
        }
    }
}

fn press(view: &DiagramView, key_code: u16) {
    let characters = NSString::from_str("");
    let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        NSEventType::KeyDown,
        NSPoint::ZERO,
        NSEventModifierFlags::empty(),
        0.0,
        0,
        None,
        &characters,
        &characters,
        false,
        key_code,
    )
    .expect("AppKit builds a key event");
    let _: () = unsafe { msg_send![view, keyDown: &*event] };
}

fn children(view: &DiagramView) -> Retained<NSArray<AnyObject>> {
    unsafe { msg_send![view, accessibilityChildren] }
}

fn check_elements(checks: &mut Checks, view: &DiagramView, expected: &[AccessibleSegment]) {
    let elements = children(view);
    checks.expect(
        elements.count() == expected.len(),
        format!("{} elements for {} segments", elements.count(), expected.len()),
    );
    for (element, segment) in elements.iter().zip(expected) {
        let label: Option<Retained<NSString>> = unsafe { msg_send![&*element, accessibilityLabel] };
        let frame: NSRect = unsafe { msg_send![&*element, accessibilityFrameInParentSpace] };
        checks.expect(
            label.is_some_and(|label| label.to_string() == segment.label),
            format!("the label of {} matches", segment.label),
        );
        let same_frame = (frame.origin.x - f64::from(segment.frame.x)).abs() < 0.5
            && (frame.origin.y - f64::from(segment.frame.y)).abs() < 0.5
            && (frame.size.width - f64::from(segment.frame.width)).abs() < 0.5
            && (frame.size.height - f64::from(segment.frame.height)).abs() < 0.5;
        checks.expect(same_frame, format!("the frame of {} matches the layout", segment.label));
    }
}

fn check_sunburst(checks: &mut Checks, view: &DiagramView, tree: &DiagramTree) -> Vec<AccessibleSegment> {
    let expected = view.state().segments();
    let ring_zero = match &view.state().layout {
        DiagramLayout::Sunburst(layout) => layout.segments.iter().filter(|segment| segment.ring == 0).count(),
        DiagramLayout::Treemap(_) => 0,
    };
    checks.expect(
        expected.len() == ring_zero + 1,
        "every visible level segment and the center are exposed",
    );
    checks.expect(
        expected.last().is_some_and(|segment| segment.is_center),
        "the center is the last element",
    );
    let first = tree.node(expected[0].node);
    checks.expect(
        expected[0].label
            == format!(
                "{}, {}, {}",
                first.name,
                format_size(first.size),
                STRINGS.category_name(first.category)
            ),
        "a label reads name, size, category",
    );
    if let DiagramLayout::Sunburst(layout) = &view.state().layout {
        for segment in expected.iter().filter(|segment| !segment.is_center) {
            let arc = layout
                .segments
                .iter()
                .find(|arc| arc.node == segment.node)
                .expect("the segment is laid out");
            let (angle, radius) = arc.middle();
            let middle = layout.point_at(angle, radius);
            checks.expect(
                segment.frame.contains(middle),
                format!("the frame of {} holds its arc", segment.label),
            );
            checks.expect(
                layout.hit_test(middle) == Some(segment.node),
                format!("hit-testing {} finds it", segment.label),
            );
        }
    }
    check_elements(checks, view, &expected);
    expected
}

fn check_keyboard(checks: &mut Checks, view: &DiagramView, expected: &[AccessibleSegment]) {
    press(view, RIGHT_ARROW);
    checks.expect(
        view.state().keyboard_focus().as_ref() == Some(&expected[0]),
        "the right arrow focuses the first segment",
    );
    press(view, RIGHT_ARROW);
    checks.expect(
        view.state().keyboard_focus().as_ref() == Some(&expected[1]),
        "the right arrow moves on",
    );
    press(view, LEFT_ARROW);
    press(view, LEFT_ARROW);
    checks.expect(
        view.state().keyboard_focus().as_ref() == expected.last(),
        "the left arrow wraps to the center",
    );
    press(view, RIGHT_ARROW);
    press(view, RETURN);
    checks.expect(
        view.state().focus() == expected[0].node,
        "return opens the focused level",
    );
    checks.expect(view.state().focus() != ROOT, "the diagram left the root");
}

fn check_treemap(checks: &mut Checks, view: &DiagramView) {
    view.update_state(|state| state.set_mode(DiagramMode::Treemap));
    let tiles = match &view.state().layout {
        DiagramLayout::Treemap(layout) => layout.tiles.iter().filter(|tile| tile.role != TileRole::Nested).count(),
        DiagramLayout::Sunburst(_) => 0,
    };
    let segments = view.state().segments();
    checks.expect(segments.len() == tiles, "the treemap exposes its top-level tiles");
    check_elements(checks, view, &segments);
}

fn main() -> ExitCode {
    let marker = MainThreadMarker::new().expect("the accessibility check runs on the main thread");
    fonts();
    let tree = book_v();
    let state = DiagramState::new(
        tree.clone(),
        DiagramMode::Sunburst,
        STRINGS,
        &Palette::DARK,
        SIDE as f32,
        SIDE as f32,
    );
    let graphics = DiagramGraphics {
        renderer: renderer(),
        atlas: GlyphAtlas::default(),
    };
    let view = DiagramView::new(
        marker,
        NSRect::new(NSPoint::ZERO, NSSize::new(SIDE, SIDE)),
        state,
        graphics,
    );
    let mut checks = Checks {
        failures: Vec::new(),
        count: 0,
    };
    let expected = check_sunburst(&mut checks, &view, &tree);
    check_keyboard(&mut checks, &view, &expected);
    check_treemap(&mut checks, &view);
    if checks.failures.is_empty() {
        println!("accessibility: {} checks passed", checks.count);
        ExitCode::SUCCESS
    } else {
        eprintln!("accessibility failed:\n{}", checks.failures.join("\n"));
        ExitCode::FAILURE
    }
}
