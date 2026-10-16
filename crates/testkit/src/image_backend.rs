use std::env;

const BACKEND_VARIABLE: &str = "DOMOVOI_IMAGE_BACKEND";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageBackend {
    Hdiutil,
    DiskutilImage,
}

impl ImageBackend {
    pub fn from_environment() -> Self {
        match env::var(BACKEND_VARIABLE).as_deref() {
            Err(env::VarError::NotPresent) | Ok("hdiutil") => Self::Hdiutil,
            Ok("diskutil") => Self::DiskutilImage,
            other => panic!("{BACKEND_VARIABLE} must be `hdiutil` or `diskutil`, got {other:?}"),
        }
    }
}
