use std::env;
use std::path::PathBuf;
use std::process::Command;

const FUNCTIONS: &str = "xpc_.*|dispatch_queue_create|dispatch_release";
const TYPES: &str = "xpc_.*|_xpc_.*|dispatch_queue_t";
const VARIABLES: &str = "_xpc_error_.*|_xpc_type_.*|XPC_.*";

fn main() {
    println!("cargo::rerun-if-changed=wrapper.h");
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_args(["-fblocks", "-isysroot", &sdk_path()])
        .allowlist_function(FUNCTIONS)
        .allowlist_type(TYPES)
        .allowlist_var(VARIABLES)
        .prepend_enum_name(false)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("bindgen generates the XPC bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR")).join("bindings.rs");
    bindings.write_to_file(output).expect("the bindings file is written");
}

fn sdk_path() -> String {
    let output = Command::new("xcrun")
        .args(["--sdk", "macosx", "--show-sdk-path"])
        .output()
        .expect("xcrun runs");
    assert!(output.status.success(), "xcrun --show-sdk-path failed");
    String::from_utf8(output.stdout)
        .expect("the SDK path is UTF-8")
        .trim()
        .to_owned()
}
