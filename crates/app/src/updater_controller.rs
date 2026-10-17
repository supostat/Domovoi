use std::ffi::CStr;

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, NSObject};
use objc2::{MainThreadMarker, MainThreadOnly, extern_class, msg_send};
use objc2_foundation::{NSBundle, NSString};

use crate::bundle_resources::is_bundled;

const SPARKLE_FRAMEWORK: &str = "Sparkle.framework";
const CONTROLLER_CLASS: &CStr = c"SPUStandardUpdaterController";

extern_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "SPUStandardUpdaterController"]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UpdaterController;
);

impl UpdaterController {
    pub(crate) fn start(marker: MainThreadMarker) -> Option<Retained<Self>> {
        if !is_bundled() {
            return None;
        }
        let frameworks = NSBundle::mainBundle().privateFrameworksPath()?;
        let path = NSString::from_str(&format!("{frameworks}/{SPARKLE_FRAMEWORK}"));
        let loaded = NSBundle::bundleWithPath(&path).is_some_and(|framework| unsafe { framework.load() });
        if !loaded || AnyClass::get(CONTROLLER_CLASS).is_none() {
            eprintln!("domovoi: {SPARKLE_FRAMEWORK} is missing from the application bundle");
            return None;
        }
        let this = Self::alloc(marker);
        let controller: Retained<Self> = unsafe {
            msg_send![
                this,
                initWithStartingUpdater: true,
                updaterDelegate: None::<&AnyObject>,
                userDriverDelegate: None::<&AnyObject>
            ]
        };
        Some(controller)
    }

    pub(crate) fn check_for_updates(&self) {
        let _: () = unsafe { msg_send![self, checkForUpdates: None::<&AnyObject>] };
    }
}
