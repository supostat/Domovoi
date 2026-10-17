use std::path::{Path, PathBuf};

use domovoi_render::BUILT_METALLIB;
use objc2_foundation::{NSBundle, NSString};

const DIAGRAM_LIBRARY: &str = "diagram";
const METALLIB: &str = "metallib";
const DEVELOPMENT_FONTS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/fonts");

pub(crate) fn is_bundled() -> bool {
    NSBundle::mainBundle().bundleIdentifier().is_some()
}

pub(crate) fn diagram_metallib() -> PathBuf {
    if is_bundled() {
        let bundle = NSBundle::mainBundle();
        let resource = bundle.pathForResource_ofType(
            Some(&NSString::from_str(DIAGRAM_LIBRARY)),
            Some(&NSString::from_str(METALLIB)),
        );
        PathBuf::from(resource.expect("the bundle carries the diagram shaders").to_string())
    } else {
        PathBuf::from(BUILT_METALLIB)
    }
}

pub fn development_fonts() -> Option<&'static Path> {
    (!is_bundled()).then(|| Path::new(DEVELOPMENT_FONTS))
}
