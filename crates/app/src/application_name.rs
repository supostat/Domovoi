use objc2_app_kit::NSRunningApplication;
use objc2_foundation::NSString;

pub(crate) fn running_application_name(bundle_id: &str) -> String {
    NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(bundle_id))
        .firstObject()
        .and_then(|application| application.localizedName())
        .map_or_else(|| bundle_id.to_owned(), |name| name.to_string())
}
