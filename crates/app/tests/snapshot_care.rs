use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use domovoi_render::BgraImage;
use domovoi_testkit::TempTree;
use serde_json::Value;

const BINARY: &str = env!("CARGO_BIN_EXE_domovoi");
const THEMES: [&str; 2] = ["dark", "light"];
const SCALES: [u32; 2] = [1, 2];
const SCREENS: [(&str, &[&str]); 3] = [("04", &[""]), ("06", &[""]), ("08", &["", "-measuring", "-purging"])];
const WIDE_WINDOW: [u32; 2] = [1100, 700];
const SHEET_WIDTH: u32 = 560;
const SHEET_MAX_HEIGHT: u32 = 700;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shell_state.json")
}

fn review_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/snapshots")
}

fn render_set(directory: &Path) -> Vec<String> {
    fs::create_dir_all(directory).expect("the snapshot directory is created");
    let mut stems = Vec::new();
    for (screen, variants) in SCREENS {
        for theme in THEMES {
            for scale in SCALES {
                let output = Command::new(BINARY)
                    .args([
                        "--snapshot",
                        screen,
                        "--theme",
                        theme,
                        "--scale",
                        &scale.to_string(),
                        "--state",
                    ])
                    .arg(fixture())
                    .arg(directory)
                    .output()
                    .expect("the binary starts");
                assert!(
                    output.status.success(),
                    "domovoi --snapshot {screen} failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                stems.extend(
                    variants
                        .iter()
                        .map(|variant| format!("{screen}{variant}-{theme}@{scale}x")),
                );
            }
        }
    }
    stems
}

fn window_of(directory: &Path, stem: &str) -> ([u32; 2], u32) {
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(directory.join(format!("{stem}.json"))).expect("the probe manifest reads"),
    )
    .expect("the probe manifest parses");
    let window = [&manifest["window"][0], &manifest["window"][1]]
        .map(|side| side.as_f64().expect("the manifest names the window size").round() as u32);
    let scale = manifest["scale"]
        .as_f64()
        .expect("the manifest names the scale")
        .round() as u32;
    (window, scale)
}

#[test]
fn care_screens_render_in_both_themes_and_scales_at_their_window_size() {
    let directory = review_directory();
    for stem in render_set(&directory) {
        let image = BgraImage::read_png(&directory.join(format!("{stem}.png"))).expect("the snapshot reads");
        let (window, scale) = window_of(&directory, &stem);
        if stem.starts_with("08") {
            assert_eq!(window[0], SHEET_WIDTH, "{stem}");
            assert!(window[1] <= SHEET_MAX_HEIGHT, "{stem} is {} pt tall", window[1]);
        } else {
            assert_eq!(window, WIDE_WINDOW, "{stem}");
        }
        let expected = window.map(|side| (side * scale) as usize);
        assert_eq!([image.width, image.height], expected, "{stem}");
    }
}

#[test]
fn care_snapshots_of_the_same_state_are_identical() {
    let first = TempTree::new().expect("a temp tree is created");
    let second = TempTree::new().expect("a temp tree is created");
    let stems = render_set(first.path());
    render_set(second.path());
    for stem in stems {
        let name = format!("{stem}.png");
        let left = fs::read(first.path().join(&name)).expect("the first snapshot reads");
        let right = fs::read(second.path().join(&name)).expect("the second snapshot reads");
        assert!(left == right, "{name} differs between two renders of one state");
    }
}
