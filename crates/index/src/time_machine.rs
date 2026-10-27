use std::path::Path;

use objc2::runtime::AnyObject;
use objc2_foundation::{NSDictionary, NSNumber, NSString};

use crate::ffi::property_list_dictionary;

pub const TIME_MACHINE_PLIST: &str = "/Library/Preferences/com.apple.TimeMachine.plist";
const AUTO_BACKUP_KEY: &str = "AutoBackup";
const MOBILE_BACKUPS_KEY: &str = "MobileBackups";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TimeMachineSettings {
    pub auto_backup: Option<bool>,
    pub mobile_backups: Option<bool>,
}

impl TimeMachineSettings {
    pub fn read(path: &Path) -> Self {
        let Some(settings) = property_list_dictionary(path) else {
            return Self::default();
        };
        Self {
            auto_backup: flag(&settings, AUTO_BACKUP_KEY),
            mobile_backups: flag(&settings, MOBILE_BACKUPS_KEY),
        }
    }

    pub fn explains_purgeable(self) -> bool {
        self.auto_backup == Some(true) || self.mobile_backups == Some(true)
    }
}

fn flag(settings: &NSDictionary<AnyObject, AnyObject>, key: &str) -> Option<bool> {
    let key = NSString::from_str(key);
    let value = settings.objectForKey(&key)?;
    value.downcast::<NSNumber>().ok().map(|number| number.boolValue())
}
