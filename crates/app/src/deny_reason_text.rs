use domovoi_rules::DenyReason;

use crate::strings::{
    DENY_FIRMLINK, DENY_KEPT_COPY_MARKED, DENY_KEPT_COPY_MISSING, DENY_OTHER_VOLUME, DENY_OWN_APPLICATION,
    DENY_PARENT_NOT_WRITABLE, DENY_PROTECTED_DATA, DENY_PROTECTED_FLAG, DENY_RUNNING_APPLICATION, DENY_SYSTEM_VOLUME,
};

pub(crate) fn deny_reason_text(reason: &DenyReason) -> &'static str {
    match reason {
        DenyReason::ProtectedFlag(_) => DENY_PROTECTED_FLAG,
        DenyReason::SystemVolume => DENY_SYSTEM_VOLUME,
        DenyReason::OtherVolume => DENY_OTHER_VOLUME,
        DenyReason::Firmlink => DENY_FIRMLINK,
        DenyReason::ParentNotWritable => DENY_PARENT_NOT_WRITABLE,
        DenyReason::ProtectedData(_) => DENY_PROTECTED_DATA,
        DenyReason::RunningApplication(_) => DENY_RUNNING_APPLICATION,
        DenyReason::OwnApplication => DENY_OWN_APPLICATION,
        DenyReason::KeptCopyMarked => DENY_KEPT_COPY_MARKED,
        DenyReason::KeptCopyMissing => DENY_KEPT_COPY_MISSING,
    }
}
