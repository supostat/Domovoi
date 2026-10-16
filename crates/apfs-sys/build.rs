use std::env;
use std::path::PathBuf;
use std::process::Command;

const CONSTANTS: &str = "ATTR_.*|FSOPT_.*|EF_.*|SF_.*|UF_.*|CLONE_.*|DIR_MNTSTATUS_.*|VOL_CAP_.*|VOL_CAPABILITIES_.*|\
                         IOPOL_.*|MNT_EXT_.*|COPYFILE_.*|F_NOCACHE|RENAME_SWAP|RENAME_EXCL|PROC_ALL_PIDS";
const FUNCTIONS: &str = "fs_snapshot_list|renamex_np|renameatx_np|proc_listpids|proc_pidpath|proc_listpidspath|\
                         xattr_flags_from_name|setiopolicy_np|getiopolicy_np";

fn main() {
    println!("cargo::rerun-if-changed=wrapper.h");
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_args(["-isysroot", &sdk_path()])
        .allowlist_var(CONSTANTS)
        .allowlist_function(FUNCTIONS)
        .allowlist_type("vtype")
        .constified_enum_module("vtype")
        .blocklist_type("attrlist")
        .raw_line("use libc::attrlist;")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("bindgen generates the APFS bindings");
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
