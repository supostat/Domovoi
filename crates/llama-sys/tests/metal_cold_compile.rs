#![cfg(feature = "metal")]

use std::fs;
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use domovoi_llama_sys::{GGML_BACKEND_DEVICE_TYPE_GPU, ggml_backend_dev_by_type};
use domovoi_testkit::TempTree;

const METAL_SOURCES: &str = "../../vendor/llama.cpp/ggml/src";
const SHARED_HEADER: &str = "ggml-common.h";
const METAL_DIRECTORY: &str = "ggml-metal";
const METAL_HEADER: &str = "ggml-metal-impl.h";
const KERNELS: &str = "kernels";
const KERNEL_EXTENSION: &str = "metal";

fn copy_sources_with_a_fresh_cache_key(destination: &Path) {
    let sources = Path::new(env!("CARGO_MANIFEST_DIR")).join(METAL_SOURCES);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is after the epoch")
        .as_nanos();
    fs::copy(sources.join(SHARED_HEADER), destination.join(SHARED_HEADER)).expect("the shared header copies");
    let metal = destination.join(METAL_DIRECTORY);
    fs::create_dir_all(metal.join(KERNELS)).expect("the kernel directory is created");
    fs::copy(
        sources.join(METAL_DIRECTORY).join(METAL_HEADER),
        metal.join(METAL_HEADER),
    )
    .expect("the Metal header copies");
    for entry in fs::read_dir(sources.join(METAL_DIRECTORY).join(KERNELS)).expect("the kernels list") {
        let path = entry.expect("a kernel entry reads").path();
        let source = fs::read_to_string(&path).expect("a kernel source reads");
        let marked = if path.extension().is_some_and(|extension| extension == KERNEL_EXTENSION) {
            format!("#define DOMOVOI_COLD_COMPILE_{nonce} 1\n{source}")
        } else {
            source
        };
        fs::write(
            metal.join(KERNELS).join(path.file_name().expect("a kernel has a name")),
            marked,
        )
        .expect("a kernel source writes");
    }
}

#[test]
#[ignore = "measures a cold Metal shader compilation on the nightly self-hosted machine"]
fn kernel_sources_compile_cold_on_first_device_use() {
    let workspace = TempTree::new().expect("a temp tree is created");
    copy_sources_with_a_fresh_cache_key(workspace.path());
    let resources = workspace.path().join(METAL_DIRECTORY);
    unsafe { std::env::set_var("GGML_METAL_PATH_RESOURCES", &resources) };
    let started = Instant::now();
    let device = unsafe { ggml_backend_dev_by_type(GGML_BACKEND_DEVICE_TYPE_GPU) };
    let elapsed = started.elapsed();
    assert!(!device.is_null(), "a Metal device is available");
    eprintln!("cold compilation of the Metal kernel sources took {elapsed:?}");
}
