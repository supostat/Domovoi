use domovoi_index::{DeltaRequest, DeltaTarget, DirectoryListing};
use domovoi_scanner::{ScanError, ScanOptions};

#[derive(Clone, Debug)]
pub struct PendingDelta {
    pub(crate) request: DeltaRequest,
    pub(crate) last_event_id: u64,
}

impl PendingDelta {
    pub fn targets(&self) -> &[DeltaTarget] {
        self.request.targets()
    }

    pub fn read(&self, options: &ScanOptions) -> Result<Vec<DirectoryListing>, ScanError> {
        self.targets().iter().map(|target| target.read(options)).collect()
    }
}
