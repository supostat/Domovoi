use std::sync::Arc;
use std::sync::mpsc::SyncSender;

use domovoi_apfs_sys::AttributeSet;

use crate::cancel_token::CancelToken;
use crate::onboarding::Onboarding;
use crate::protected_folders::ProtectedFolders;
use crate::scan_progress::ScanProgress;

#[derive(Debug)]
pub struct ScanOptions {
    pub attributes: AttributeSet,
    pub progress: Option<SyncSender<ScanProgress>>,
    pub cancel: CancelToken,
    pub protected: ProtectedFolders,
    pub onboarding: Arc<Onboarding>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            attributes: AttributeSet::Extended,
            progress: None,
            cancel: CancelToken::default(),
            protected: ProtectedFolders::default(),
            onboarding: Arc::new(Onboarding::finished()),
        }
    }
}
