use objc2::rc::Retained;
use objc2_app_kit::NSScrollView;

pub(crate) struct TrayLayout {
    pub(crate) height: f64,
    pub(crate) body: Option<Retained<NSScrollView>>,
    pub(crate) primary: [f64; 4],
}
