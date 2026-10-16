use std::sync::mpsc::SyncSender;
use std::time::Duration;

use domovoi_scanner::CancelToken;

use crate::cloud_folders::CloudFolders;
use crate::duplicate_progress::DuplicateProgress;

const DEFAULT_MINIMUM_SIZE: u64 = 1024 * 1024;
const DEFAULT_FULL_HASH_LIMIT: u64 = 4 * 1024 * 1024 * 1024;
const DEFAULT_CLOUD_TIME_LIMIT: Duration = Duration::from_secs(60);

#[derive(Debug)]
pub struct DuplicateOptions {
    pub minimum_size: u64,
    pub full_hash_limit: u64,
    pub cloud_folders: CloudFolders,
    pub cloud_time_limit: Duration,
    pub progress: Option<SyncSender<DuplicateProgress>>,
    pub cancel: CancelToken,
}

impl DuplicateOptions {
    pub fn new(cloud_folders: CloudFolders) -> Self {
        Self {
            minimum_size: DEFAULT_MINIMUM_SIZE,
            full_hash_limit: DEFAULT_FULL_HASH_LIMIT,
            cloud_folders,
            cloud_time_limit: DEFAULT_CLOUD_TIME_LIMIT,
            progress: None,
            cancel: CancelToken::default(),
        }
    }
}
