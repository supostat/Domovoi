use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

const LLAMA_SOURCE: &str = "../../vendor/llama.cpp";
const WATCHED_SOURCES: [&str; 5] = ["CMakeLists.txt", "cmake", "include", "src", "ggml"];
const COMPILER_LAUNCHERS: [&str; 4] = [
    "CMAKE_C_COMPILER_LAUNCHER",
    "CMAKE_CXX_COMPILER_LAUNCHER",
    "CMAKE_OBJC_COMPILER_LAUNCHER",
    "CMAKE_OBJCXX_COMPILER_LAUNCHER",
];
const DISABLED_PARTS: [&str; 7] = [
    "LLAMA_BUILD_COMMON",
    "LLAMA_BUILD_APP",
    "LLAMA_BUILD_TOOLS",
    "LLAMA_BUILD_TESTS",
    "LLAMA_BUILD_EXAMPLES",
    "LLAMA_BUILD_SERVER",
    "LLAMA_OPENSSL",
];
const CPU_LIBRARIES: [&str; 5] = ["llama", "ggml", "ggml-base", "ggml-cpu", "ggml-blas"];
const METAL_LIBRARY: &str = "ggml-metal";
const CPU_FRAMEWORKS: [&str; 2] = ["Foundation", "Accelerate"];
const METAL_FRAMEWORKS: [&str; 2] = ["Metal", "MetalKit"];
const METALLIBS: [&str; 2] = ["default.metallib", "ggml-tensor.metallib"];
const BINDING_PATTERN: &str = "llama_.*|ggml_.*|gguf_.*";
const CONSTANT_PATTERN: &str = "LLAMA_.*|GGML_.*|GGUF_.*";

fn main() {
    let source =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR")).join(LLAMA_SOURCE);
    println!("cargo::rerun-if-changed=build.rs");
    for watched in WATCHED_SOURCES {
        println!("cargo::rerun-if-changed={}", source.join(watched).display());
    }
    for launcher in COMPILER_LAUNCHERS {
        println!("cargo::rerun-if-env-changed={launcher}");
    }
    let metal = env::var_os("CARGO_FEATURE_METAL").is_some();
    let cpu_only = env::var_os("CARGO_FEATURE_CPU_ONLY").is_some();
    assert!(
        metal != cpu_only,
        "enable exactly one of the metal and cpu-only features"
    );

    let installed = build_llama(&source, metal);
    link(&installed, metal);
    if metal {
        export_metallibs(&installed.join("bin"));
    }
    generate_bindings(&source, metal);
}

fn build_llama(source: &Path, metal: bool) -> PathBuf {
    let mut config = cmake::Config::new(source);
    config
        .profile("Release")
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("GGML_METAL", switch(metal))
        .define("GGML_METAL_EMBED_LIBRARY", "OFF")
        .define("GGML_NATIVE", "OFF")
        .define("GGML_OPENMP", "OFF")
        .define("GGML_BLAS", "ON")
        .define("GGML_ACCELERATE", "ON");
    for part in DISABLED_PARTS {
        config.define(part, "OFF");
    }
    for launcher in COMPILER_LAUNCHERS {
        if let Ok(command) = env::var(launcher) {
            config.define(launcher, command);
        }
    }
    config.build()
}

fn link(installed: &Path, metal: bool) {
    println!("cargo::rustc-link-search=native={}", installed.join("lib").display());
    for library in CPU_LIBRARIES {
        println!("cargo::rustc-link-lib=static={library}");
    }
    for framework in CPU_FRAMEWORKS {
        println!("cargo::rustc-link-lib=framework={framework}");
    }
    if metal {
        println!("cargo::rustc-link-lib=static={METAL_LIBRARY}");
        for framework in METAL_FRAMEWORKS {
            println!("cargo::rustc-link-lib=framework={framework}");
        }
    }
    println!("cargo::rustc-link-lib=c++");
}

fn export_metallibs(directory: &Path) {
    for metallib in METALLIBS {
        assert!(
            directory.join(metallib).is_file(),
            "the llama.cpp build did not produce {metallib} in {}",
            directory.display()
        );
    }
    println!("cargo::rustc-env=DOMOVOI_METALLIB_DIR={}", directory.display());
    println!("cargo::metadata=metallib_dir={}", directory.display());
}

fn generate_bindings(source: &Path, metal: bool) {
    let mut builder = bindgen::Builder::default()
        .header(source.join("include/llama.h").to_string_lossy())
        .clang_arg(format!("-I{}", source.join("ggml/include").display()))
        .clang_args(["-isysroot", &sdk_path()])
        .allowlist_function(BINDING_PATTERN)
        .allowlist_type(BINDING_PATTERN)
        .allowlist_var(CONSTANT_PATTERN)
        .allowlist_recursively(false)
        .derive_partialeq(true)
        .prepend_enum_name(false)
        .raw_line("use libc::FILE;")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));
    if metal {
        builder = builder.header(source.join("ggml/include/ggml-metal.h").to_string_lossy());
    }
    let bindings = builder.generate().expect("bindgen generates the llama.cpp bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR")).join("bindings.rs");
    bindings.write_to_file(output).expect("the bindings file is written");
}

fn switch(enabled: bool) -> &'static str {
    if enabled { "ON" } else { "OFF" }
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
