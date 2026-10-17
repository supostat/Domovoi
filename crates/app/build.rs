const FRAMEWORKS_RPATH: &str = "@executable_path/../Frameworks";

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rustc-link-arg-bin=domovoi=-Wl,-rpath,{FRAMEWORKS_RPATH}");
}
