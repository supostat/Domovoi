use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use domovoi_render::BgraImage;
use domovoi_testkit::TempTree;
use serde_json::Value;

const BINARY: &str = env!("CARGO_BIN_EXE_domovoi");
const THEMES: [&str; 2] = ["dark", "light"];
const SCALES: [u32; 2] = [1, 2];
const SCREENS: [&str; 3] = ["01", "02", "03"];
const WIDE_WINDOW: [u32; 2] = [1100, 700];
const NARROW_WIDTH: u32 = 560;
const NARROW_MAX_HEIGHT: u32 = 600;
const CONTRAST: &str = "03-dark@2x-contrast";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shell_state.json")
}

fn review_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/snapshots")
}

fn snapshot(directory: &Path, arguments: &[&str]) {
    let output = Command::new(BINARY)
        .args(["--snapshot"])
        .args(arguments)
        .arg("--state")
        .arg(fixture())
        .arg(directory)
        .output()
        .expect("the binary starts");
    assert!(
        output.status.success(),
        "domovoi --snapshot {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn render_set(directory: &Path) -> Vec<String> {
    fs::create_dir_all(directory).expect("the snapshot directory is created");
    let mut expected = Vec::new();
    for theme in THEMES {
        for scale in SCALES {
            let scale_text = scale.to_string();
            snapshot(directory, &["all", "--theme", theme, "--scale", &scale_text]);
            for screen in SCREENS {
                expected.push(format!("{screen}-{theme}@{scale}x"));
            }
        }
    }
    snapshot(
        directory,
        &["03", "--theme", "dark", "--scale", "2", "--contrast", "high"],
    );
    expected.push(CONTRAST.to_owned());
    expected
}

#[test]
fn every_screen_renders_in_both_themes_and_scales_at_the_window_size() {
    let directory = review_directory();
    for stem in render_set(&directory) {
        let image = BgraImage::read_png(&directory.join(format!("{stem}.png"))).expect("the snapshot reads");
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
        if stem.starts_with("01") {
            assert_eq!(window[0], NARROW_WIDTH, "{stem}");
            assert!(window[1] <= NARROW_MAX_HEIGHT, "{stem} is {} pt tall", window[1]);
        } else {
            assert_eq!(window, WIDE_WINDOW, "{stem}");
        }
        let expected = window.map(|side| (side * scale) as usize);
        assert_eq!([image.width, image.height], expected, "{stem}");
    }
}

#[test]
fn snapshots_of_the_same_state_are_identical() {
    let first = TempTree::new().expect("a temp tree is created");
    let second = TempTree::new().expect("a temp tree is created");
    let expected = render_set(first.path());
    render_set(second.path());
    for stem in expected {
        let name = format!("{stem}.png");
        let left = fs::read(first.path().join(&name)).expect("the first snapshot reads");
        let right = fs::read(second.path().join(&name)).expect("the second snapshot reads");
        assert!(left == right, "{name} differs between two renders of one state");
    }
}

#[test]
fn an_unknown_screen_is_refused_with_usage() {
    let output = Command::new(BINARY)
        .args(["--snapshot", "09", "--theme", "dark", "--scale", "2", "--state"])
        .arg(fixture())
        .arg(review_directory())
        .output()
        .expect("the binary starts");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown screen 09"));
}
