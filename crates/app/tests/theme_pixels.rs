use std::fs;
use std::path::Path;
use std::process::Command;

use domovoi_render::BgraImage;
use domovoi_testkit::TempTree;
use domovoi_theme::{Palette, Rgba};
use serde_json::Value;

const BINARY: &str = env!("CARGO_BIN_EXE_domovoi");
const TOLERANCE: u8 = 1;
const BUTTON_EDGE_INSET: f64 = 6.0;

fn render_main(directory: &Path, theme: &str, scale: u32) -> (BgraImage, Value) {
    let output = Command::new(BINARY)
        .args([
            "--snapshot",
            "03",
            "--theme",
            theme,
            "--scale",
            &scale.to_string(),
            "--state",
        ])
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shell_state.json"))
        .arg(directory)
        .output()
        .expect("the binary starts");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let stem = format!("03-{theme}@{scale}x");
    let image = BgraImage::read_png(&directory.join(format!("{stem}.png"))).expect("the snapshot reads");
    let manifest = fs::read_to_string(directory.join(format!("{stem}.json"))).expect("the manifest reads");
    (image, serde_json::from_str(&manifest).expect("the manifest parses"))
}

fn expected_bgra(token: Rgba) -> [u8; 3] {
    let [red, green, blue, _] = token.straight().map(|channel| (channel * 255.0).round() as u8);
    [blue, green, red]
}

fn assert_token(image: &BgraImage, scale: f64, point: [f64; 2], token: Rgba, name: &str) {
    let x = (point[0] * scale).round() as usize;
    let y = (point[1] * scale).round() as usize;
    let pixel = image.pixel(x, y);
    let expected = expected_bgra(token);
    for channel in 0..3 {
        assert!(
            pixel[channel].abs_diff(expected[channel]) <= TOLERANCE,
            "{name} at ({x}, {y}) is {pixel:?}, the token is {expected:?}"
        );
    }
}

fn point(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .expect("a probe is a coordinate list")
        .iter()
        .map(|coordinate| coordinate.as_f64().expect("a coordinate is a number"))
        .collect()
}

#[test]
fn window_sidebar_and_accent_button_carry_the_palette_tokens() {
    let directory = TempTree::new().expect("a temp tree is created");
    for (theme, palette) in [("dark", &Palette::DARK), ("light", &Palette::LIGHT)] {
        for scale in [1, 2] {
            let (image, manifest) = render_main(directory.path(), theme, scale);
            let factor = f64::from(scale);
            let probes = &manifest["probes"];
            let window = point(&probes["window_background"]);
            assert_token(
                &image,
                factor,
                [window[0], window[1]],
                palette.bg,
                &format!("{theme}@{scale}x window"),
            );
            let sidebar = point(&probes["sidebar"]);
            assert_token(
                &image,
                factor,
                [sidebar[0], sidebar[1]],
                palette.bg_2,
                &format!("{theme}@{scale}x sidebar"),
            );
            let button = point(&probes["accent_button"]);
            let inside = [button[0] + BUTTON_EDGE_INSET, button[1] + button[3] / 2.0];
            assert_token(
                &image,
                factor,
                inside,
                palette.wax,
                &format!("{theme}@{scale}x accent button"),
            );
        }
    }
}
