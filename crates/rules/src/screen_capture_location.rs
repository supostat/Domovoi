use std::path::{Path, PathBuf};

use objc2::AnyThread;
use objc2_foundation::{NSString, NSUserDefaults};

use crate::config_overrides::expand_home;

const SCREEN_CAPTURE_DOMAIN: &str = "com.apple.screencapture";
const LOCATION_KEY: &str = "location";

pub fn screen_capture_location(home: &Path) -> Option<PathBuf> {
    let defaults = NSUserDefaults::initWithSuiteName(
        NSUserDefaults::alloc(),
        Some(&NSString::from_str(SCREEN_CAPTURE_DOMAIN)),
    )?;
    let location = defaults.stringForKey(&NSString::from_str(LOCATION_KEY))?;
    Some(expand_home(&location.to_string(), home))
}
