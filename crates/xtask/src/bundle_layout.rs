use std::path::{Path, PathBuf};

pub(crate) const APP_EXECUTABLE: &str = "domovoi";
pub(crate) const SERVICE_EXECUTABLE: &str = "domovoi-inference";
pub(crate) const SERVICE_METALLIBS: [&str; 2] = ["default.metallib", "ggml-tensor.metallib"];
pub(crate) const DIAGRAM_METALLIB: &str = "diagram.metallib";
pub(crate) const CATALOG: &str = "catalog.json";
const APPLICATION: &str = "DOMOVOI.app";
const SERVICE_BUNDLE: &str = "com.domovoi.inference.xpc";
const SPARKLE: &str = "Sparkle.framework";
const INFO: &str = "Info.plist";

#[derive(Clone, Debug)]
pub(crate) struct BundleLayout {
    pub(crate) app: PathBuf,
}

impl BundleLayout {
    pub(crate) fn in_directory(directory: &Path) -> Self {
        Self {
            app: directory.join(APPLICATION),
        }
    }

    pub(crate) fn of_app(app: &Path) -> Self {
        Self { app: app.to_path_buf() }
    }

    pub(crate) fn contents(&self) -> PathBuf {
        self.app.join("Contents")
    }

    pub(crate) fn executable(&self) -> PathBuf {
        self.contents().join("MacOS").join(APP_EXECUTABLE)
    }

    pub(crate) fn info(&self) -> PathBuf {
        self.contents().join(INFO)
    }

    pub(crate) fn resources(&self) -> PathBuf {
        self.contents().join("Resources")
    }

    pub(crate) fn fonts(&self) -> PathBuf {
        self.resources().join("Fonts")
    }

    pub(crate) fn licenses(&self) -> PathBuf {
        self.resources().join("Licenses")
    }

    pub(crate) fn sparkle(&self) -> PathBuf {
        self.contents().join("Frameworks").join(SPARKLE)
    }

    pub(crate) fn service(&self) -> PathBuf {
        self.contents().join("XPCServices").join(SERVICE_BUNDLE)
    }

    pub(crate) fn service_executable(&self) -> PathBuf {
        self.service().join("Contents/MacOS").join(SERVICE_EXECUTABLE)
    }

    pub(crate) fn service_info(&self) -> PathBuf {
        self.service().join("Contents").join(INFO)
    }

    pub(crate) fn service_resources(&self) -> PathBuf {
        self.service().join("Contents/Resources")
    }
}
