#![allow(dead_code)]

use std::env;
use std::fs::File;
use std::os::fd::OwnedFd;

pub(crate) const COMPACT_MODEL_VARIABLE: &str = "DOMOVOI_NIGHTLY_COMPACT_MODEL";
pub(crate) const COMPACT_CONTEXT_LENGTH: u32 = 8192;

pub(crate) fn compact_model() -> OwnedFd {
    let path = env::var_os(COMPACT_MODEL_VARIABLE)
        .unwrap_or_else(|| panic!("{COMPACT_MODEL_VARIABLE} names the GGUF of the compact catalog entry"));
    File::open(path).expect("the compact model opens").into()
}
