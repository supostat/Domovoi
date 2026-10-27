use std::cell::Cell;

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2::runtime::Bool;
use objc2_app_kit::NSApplication;
use objc2_foundation::{NSBundle, NSError, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest, UNUserNotificationCenter,
};

use crate::strings::APP_NAME;

const SCAN_FINISHED_IDENTIFIER: &str = "scan-finished";

#[derive(Debug)]
pub(crate) struct ScanNotifier {
    authorization_requested: Cell<bool>,
}

impl ScanNotifier {
    pub(crate) fn in_bundle() -> Option<Self> {
        NSBundle::mainBundle().bundleIdentifier().map(|_| Self {
            authorization_requested: Cell::new(false),
        })
    }

    pub(crate) fn request_authorization(&self) {
        if self.authorization_requested.replace(true) {
            return;
        }
        let handler = RcBlock::new(|_granted: Bool, _error: *mut NSError| {});
        UNUserNotificationCenter::currentNotificationCenter()
            .requestAuthorizationWithOptions_completionHandler(UNAuthorizationOptions::Alert, &handler);
    }

    pub(crate) fn scan_finished(&self, marker: MainThreadMarker, line: &str) {
        if !self.authorization_requested.get() || NSApplication::sharedApplication(marker).isActive() {
            return;
        }
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(APP_NAME));
        content.setBody(&NSString::from_str(line));
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
            &NSString::from_str(SCAN_FINISHED_IDENTIFIER),
            &content,
            None,
        );
        UNUserNotificationCenter::currentNotificationCenter()
            .addNotificationRequest_withCompletionHandler(&request, None);
    }
}
