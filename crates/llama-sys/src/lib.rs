#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    non_snake_case,
    dead_code,
    unreachable_pub,
    missing_debug_implementations,
    unpredictable_function_pointer_comparisons,
    clippy::all,
    clippy::pedantic
)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(feature = "metal")]
pub const METALLIB_DIRECTORY: &str = env!("DOMOVOI_METALLIB_DIR");
