use crate::bgra_image::BgraImage;

#[derive(Clone, Debug, PartialEq)]
pub struct OffscreenFrame {
    pub image: BgraImage,
    pub gpu_seconds: f64,
}
