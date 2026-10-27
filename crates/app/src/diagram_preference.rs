use domovoi_render::DiagramMode;
use objc2_foundation::{NSString, NSUserDefaults};

const KEY: &str = "DiagramMode";
const SMALL_GROUPS_KEY: &str = "ExpandSmallGroups";
const SUNBURST: &str = "sunburst";
const TREEMAP: &str = "treemap";

pub(crate) fn remembered_mode() -> DiagramMode {
    let stored = NSUserDefaults::standardUserDefaults().stringForKey(&NSString::from_str(KEY));
    match stored.map(|value| value.to_string()).as_deref() {
        Some(TREEMAP) => DiagramMode::Treemap,
        _ => DiagramMode::Sunburst,
    }
}

pub(crate) fn remembered_small_groups() -> bool {
    NSUserDefaults::standardUserDefaults().boolForKey(&NSString::from_str(SMALL_GROUPS_KEY))
}

pub(crate) fn remember_small_groups(always: bool) {
    NSUserDefaults::standardUserDefaults().setBool_forKey(always, &NSString::from_str(SMALL_GROUPS_KEY));
}

pub(crate) fn remember_mode(mode: DiagramMode) {
    let value = NSString::from_str(match mode {
        DiagramMode::Sunburst => SUNBURST,
        DiagramMode::Treemap => TREEMAP,
    });
    unsafe { NSUserDefaults::standardUserDefaults().setObject_forKey(Some(&value), &NSString::from_str(KEY)) };
}
