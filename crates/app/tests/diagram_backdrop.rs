use std::path::Path;
use std::time::Instant;

use domovoi_app::{MainScreen, ScanningScreen, Screen, ShellData, ShellFixture, ShellPalette, development_fonts};
use domovoi_render::{DiagramMode, DiagramState, GlyphAtlas, ROOT, register_fonts};
use domovoi_theme::Palette;

const SCALE: f32 = 1.0;

fn shell_data(screen: Screen) -> ShellData {
    ShellFixture::read(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shell_state.json"))
        .and_then(|fixture| fixture.shell_data(screen))
        .expect("the fixture builds the screen data")
}

fn channel_bytes(color: [f32; 4]) -> [u8; 4] {
    color.map(|channel| (channel * 255.0).round() as u8)
}

fn cleared_with(state: &DiagramState) -> [u8; 4] {
    channel_bytes(state.scene(Instant::now(), SCALE, &mut GlyphAtlas::default()).clear)
}

#[test]
fn the_live_diagram_clears_with_the_pane_background_it_sits_on() {
    if let Some(fonts) = development_fonts() {
        register_fonts(fonts).expect("the fonts register");
    }
    let scanning = shell_data(Screen::Scanning);
    let main = shell_data(Screen::Main);
    for (theme, tokens) in [("dark", &Palette::DARK), ("light", &Palette::LIGHT)] {
        let palette = ShellPalette {
            tokens,
            high_contrast: false,
        };
        let pane = channel_bytes(tokens.bg.premultiplied());
        assert_eq!(
            cleared_with(&ScanningScreen::diagram_state(&scanning, &palette)),
            pane,
            "{theme}: the scanning diagram's frame shows the pane background around the ring"
        );
        assert_eq!(
            cleared_with(&MainScreen::diagram_state(&main, ROOT, DiagramMode::Sunburst, &palette)),
            pane,
            "{theme}: the main diagram's frame shows the pane background around the ring"
        );
    }
}
