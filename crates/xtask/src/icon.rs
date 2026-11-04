use std::fs;
use std::io;
use std::path::Path;

use resvg::{tiny_skia, usvg};

use crate::finding::Finding;
use crate::generate_mode::GenerateMode;
use crate::icon_artwork::IconArtwork;
use crate::icon_slot::ICON_SLOTS;
use crate::workspace;

pub(crate) const ICONSET: &str = "assets/icon/AppIcon.iconset";
const CANVAS_PIXELS: f32 = 1024.0;

pub(crate) fn run(mode: GenerateMode) -> io::Result<Vec<Finding>> {
    let root = workspace::root();
    let directory = root.join(ICONSET);
    match mode {
        GenerateMode::Write => {
            write_iconset(&root, &directory)?;
            Ok(Vec::new())
        }
        GenerateMode::Check => check_iconset(&root, &directory),
    }
}

pub(crate) fn render(root: &Path, artwork: IconArtwork, pixels: u32) -> io::Result<Vec<u8>> {
    let source = fs::read(root.join(artwork.source()))?;
    let tree = usvg::Tree::from_data(&source, &usvg::Options::default()).map_err(io::Error::other)?;
    let mut pixmap = tiny_skia::Pixmap::new(pixels, pixels)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "an icon slot has a size"))?;
    let scale = pixels as f32 / CANVAS_PIXELS;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().map_err(io::Error::other)
}

pub(crate) fn write_iconset(root: &Path, directory: &Path) -> io::Result<()> {
    if directory.exists() {
        fs::remove_dir_all(directory)?;
    }
    fs::create_dir_all(directory)?;
    for slot in ICON_SLOTS {
        fs::write(
            directory.join(slot.file_name),
            render(root, slot.artwork(), slot.pixels)?,
        )?;
    }
    Ok(())
}

pub(crate) fn check_iconset(root: &Path, directory: &Path) -> io::Result<Vec<Finding>> {
    let mut findings = Vec::new();
    for slot in ICON_SLOTS {
        let path = directory.join(slot.file_name);
        let expected = render(root, slot.artwork(), slot.pixels)?;
        match fs::read(&path) {
            Ok(committed) if committed == expected => {}
            Ok(_) => findings.push(finding(&path, "differs from its artwork: run cargo xtask icon")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                findings.push(finding(&path, "is missing: run cargo xtask icon"));
            }
            Err(error) => return Err(error),
        }
    }
    if directory.is_dir() {
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            let known = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| ICON_SLOTS.iter().any(|slot| slot.file_name == name));
            if !known {
                findings.push(finding(&path, "is not an icon slot: run cargo xtask icon"));
            }
        }
    }
    Ok(findings)
}

fn finding(path: &Path, reason: &str) -> Finding {
    Finding {
        path: path.to_path_buf(),
        line: 0,
        reason: reason.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use resvg::tiny_skia::Pixmap;

    use super::{check_iconset, render, write_iconset};
    use crate::icon_artwork::IconArtwork;
    use crate::icon_slot::ICON_SLOTS;
    use crate::workspace;

    const EXPECTED_SLOTS: [(&str, u32); 10] = [
        ("icon_16x16.png", 16),
        ("icon_16x16@2x.png", 32),
        ("icon_32x32.png", 32),
        ("icon_32x32@2x.png", 64),
        ("icon_128x128.png", 128),
        ("icon_128x128@2x.png", 256),
        ("icon_256x256.png", 256),
        ("icon_256x256@2x.png", 512),
        ("icon_512x512.png", 512),
        ("icon_512x512@2x.png", 1024),
    ];

    struct ScratchIconset {
        directory: PathBuf,
    }

    impl ScratchIconset {
        fn written(name: &str) -> Self {
            let directory =
                std::env::temp_dir().join(format!("domovoi-icon-{name}-{}/AppIcon.iconset", std::process::id()));
            write_iconset(&workspace::root(), &directory).expect("the iconset is written");
            Self { directory }
        }

        fn findings(&self) -> Vec<String> {
            check_iconset(&workspace::root(), &self.directory)
                .expect("the iconset is checked")
                .iter()
                .map(ToString::to_string)
                .collect()
        }

        fn slot(&self, name: &str) -> PathBuf {
            self.directory.join(name)
        }
    }

    impl Drop for ScratchIconset {
        fn drop(&mut self) {
            let parent = self.directory.parent().expect("the iconset has a parent folder");
            let _ = fs::remove_dir_all(parent);
        }
    }

    fn decoded(root: &Path, artwork: IconArtwork, pixels: u32) -> Pixmap {
        Pixmap::decode_png(&render(root, artwork, pixels).expect("the artwork renders")).expect("the png decodes")
    }

    #[test]
    fn icon_slots_carry_the_names_and_pixels_iconutil_expects() {
        let slots: Vec<(&str, u32)> = ICON_SLOTS.iter().map(|slot| (slot.file_name, slot.pixels)).collect();
        assert_eq!(slots, EXPECTED_SLOTS);
    }

    #[test]
    fn icon_slots_up_to_32_pixels_take_the_small_artwork() {
        for slot in ICON_SLOTS {
            let expected = if slot.pixels <= 32 {
                IconArtwork::Small
            } else {
                IconArtwork::Full
            };
            assert_eq!(slot.artwork(), expected, "{}", slot.file_name);
        }
    }

    #[test]
    fn icon_png_has_exactly_the_size_of_its_slot() {
        let root = workspace::root();
        let small = decoded(&root, IconArtwork::Small, 16);
        assert_eq!((small.width(), small.height()), (16, 16));
        let full = decoded(&root, IconArtwork::Full, 1024);
        assert_eq!((full.width(), full.height()), (1024, 1024));
    }

    #[test]
    fn icon_is_transparent_outside_its_rounded_square_and_opaque_in_the_center() {
        let full = decoded(&workspace::root(), IconArtwork::Full, 1024);
        let corner = full.pixel(0, 0).expect("the corner pixel exists");
        let center = full.pixel(512, 512).expect("the center pixel exists");
        assert_eq!(corner.alpha(), 0, "the corner lies outside the rounded square");
        assert_eq!(center.alpha(), 255, "the center of the rosette is opaque");
    }

    #[test]
    fn icon_check_is_silent_on_an_iconset_drawn_from_the_artwork() {
        let iconset = ScratchIconset::written("fresh");
        assert_eq!(iconset.findings(), Vec::<String>::new());
    }

    #[test]
    fn icon_check_names_a_png_that_differs_from_its_artwork() {
        let iconset = ScratchIconset::written("differs");
        let path = iconset.slot("icon_128x128.png");
        let mut bytes = fs::read(&path).expect("the slot reads");
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        fs::write(&path, bytes).expect("the slot is corrupted");
        let findings = iconset.findings();
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert!(
            findings[0].contains("icon_128x128.png") && findings[0].contains("differs"),
            "{findings:?}"
        );
    }

    #[test]
    fn icon_check_names_a_missing_png() {
        let iconset = ScratchIconset::written("missing");
        fs::remove_file(iconset.slot("icon_32x32.png")).expect("the slot is removed");
        let findings = iconset.findings();
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert!(
            findings[0].contains("icon_32x32.png") && findings[0].contains("missing"),
            "{findings:?}"
        );
    }

    #[test]
    fn icon_check_names_a_stray_file_in_the_iconset() {
        let iconset = ScratchIconset::written("stray");
        fs::write(iconset.slot("notes.txt"), b"stray").expect("a stray file is written");
        let findings = iconset.findings();
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert!(
            findings[0].contains("notes.txt") && findings[0].contains("not an icon slot"),
            "{findings:?}"
        );
    }
}
