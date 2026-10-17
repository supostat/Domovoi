use std::sync::mpsc::Receiver;

use domovoi_downloader::{DownloadHandle, DownloadProgress};

use crate::transfer_speed::TransferSpeed;

#[derive(Debug)]
pub(crate) struct ModelDownload {
    pub entry_id: String,
    pub handle: DownloadHandle,
    pub updates: Receiver<DownloadProgress>,
    pub received: u64,
    pub speed: TransferSpeed,
}
