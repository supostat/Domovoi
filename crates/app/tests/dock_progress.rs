use domovoi_app::{BAR_HEIGHT, BAR_INSET, bar_frame, bar_track};
use objc2_foundation::{NSRect, NSSize};

const TILE: NSSize = NSSize::new(128.0, 128.0);
const TOLERANCE: f64 = 1e-9;

fn close(left: f64, right: f64) -> bool {
    (left - right).abs() < TOLERANCE
}

fn same_origin(left: NSRect, right: NSRect) -> bool {
    close(left.origin.x, right.origin.x) && close(left.origin.y, right.origin.y)
}

#[test]
fn the_bar_grows_with_the_sweep_inside_the_track() {
    let track = bar_track(TILE);
    assert!(close(track.origin.x, BAR_INSET));
    assert!(close(track.size.width, TILE.width - 2.0 * BAR_INSET));
    assert!(close(track.size.height, BAR_HEIGHT));
    assert!(
        track.origin.y > 0.0 && track.origin.y + BAR_HEIGHT < TILE.height / 2.0,
        "the bar sits in the lower half"
    );

    let empty = bar_frame(0.0, TILE);
    let half = bar_frame(0.5, TILE);
    let full = bar_frame(1.0, TILE);
    assert!(close(empty.size.width, 0.0));
    assert!(close(half.size.width, track.size.width / 2.0));
    assert!(close(full.size.width, track.size.width));
    for frame in [empty, half, full] {
        assert!(same_origin(frame, track));
        assert!(close(frame.size.height, BAR_HEIGHT));
    }
    assert!(
        close(bar_frame(1.5, TILE).size.width, track.size.width),
        "the sweep is clamped above"
    );
    assert!(
        close(bar_frame(-0.5, TILE).size.width, 0.0),
        "the sweep is clamped below"
    );
}
