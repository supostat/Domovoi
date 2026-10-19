const SHIM: &str = "shim.c";

fn main() {
    println!("cargo::rerun-if-changed={SHIM}");
    cc::Build::new()
        .file(SHIM)
        .warnings_into_errors(true)
        .compile("domovoi_log_shim");
}
