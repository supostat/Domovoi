use std::sync::{Arc, Mutex};
use std::time::Duration;

use domovoi_theme::{Motion, observe_reduce_motion, reduce_motion};
use objc2_app_kit::{NSWorkspace, NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification};

const EVERY_MOTION: [Motion; 4] = [
    Motion::LevelZoom,
    Motion::HoverDim,
    Motion::TrashCollapse,
    Motion::CleanupOrnament,
];

fn post_accessibility_display_options_change() {
    let center = NSWorkspace::sharedWorkspace().notificationCenter();
    unsafe { center.postNotificationName_object(NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification, None) };
}

#[test]
fn durations_follow_the_motion_budget() {
    let durations = EVERY_MOTION.map(|motion| motion.duration(false));
    assert_eq!(
        durations,
        [220, 120, 320, 1400].map(Duration::from_millis),
        "level zoom, hover dim, trash collapse, cleanup ornament"
    );
}

#[test]
fn reduce_motion_replaces_every_animation_with_a_frame_change() {
    for motion in EVERY_MOTION {
        assert_eq!(motion.duration(true), Duration::ZERO, "{motion:?}");
    }
}

#[test]
fn observer_reports_the_current_setting_until_dropped() {
    let reported = Arc::new(Mutex::new(Vec::new()));
    let observation = observe_reduce_motion({
        let reported = Arc::clone(&reported);
        move |reduce| reported.lock().expect("observer lock").push(reduce)
    });
    post_accessibility_display_options_change();
    assert_eq!(*reported.lock().expect("observer lock"), vec![reduce_motion()]);
    drop(observation);
    post_accessibility_display_options_change();
    assert_eq!(reported.lock().expect("observer lock").len(), 1);
}
