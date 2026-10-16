#![allow(dead_code)]

use std::env;
use std::fs;
use std::path::Path;

use domovoi_llama_sys::METALLIB_DIRECTORY;

const METALLIBS: [&str; 2] = ["default.metallib", "ggml-tensor.metallib"];

pub(crate) fn place_metallibs(directory: &Path) {
    fs::create_dir_all(directory).expect("the metallib directory is created");
    for metallib in METALLIBS {
        fs::copy(Path::new(METALLIB_DIRECTORY).join(metallib), directory.join(metallib)).expect("a metallib is copied");
    }
}

pub(crate) fn place_metallibs_beside_the_test_binary() {
    let binary = env::current_exe().expect("the test binary is known");
    place_metallibs(binary.parent().expect("the test binary lies in a directory"));
}
