use std::collections::BTreeSet;

use objc2_app_kit::NSWorkspace;

pub fn running_bundle_ids() -> BTreeSet<String> {
    NSWorkspace::sharedWorkspace()
        .runningApplications()
        .iter()
        .filter_map(|application| application.bundleIdentifier())
        .map(|identifier| identifier.to_string())
        .collect()
}
