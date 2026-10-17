use std::env;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;

const SHADER: &str = "shaders/diagram.metal";
const XCRUN: &str = "/usr/bin/xcrun";

fn main() {
    println!("cargo::rerun-if-changed={SHADER}");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    let air = out.join("diagram.air");
    let library = out.join("diagram.metallib");
    xcrun(&[
        OsStr::new("-sdk"),
        OsStr::new("macosx"),
        OsStr::new("metal"),
        OsStr::new("-c"),
        OsStr::new(SHADER),
        OsStr::new("-o"),
        air.as_os_str(),
    ]);
    xcrun(&[
        OsStr::new("-sdk"),
        OsStr::new("macosx"),
        OsStr::new("metallib"),
        air.as_os_str(),
        OsStr::new("-o"),
        library.as_os_str(),
    ]);
    println!("cargo::rustc-env=DOMOVOI_DIAGRAM_METALLIB={}", library.display());
}

fn xcrun(arguments: &[&OsStr]) {
    let status = Command::new(XCRUN).args(arguments).status().expect("xcrun runs");
    assert!(status.success(), "xcrun {arguments:?} failed");
}
