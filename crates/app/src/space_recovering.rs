use std::io;
use std::path::Path;

use domovoi_index::{RecoveryHandle, SpaceRecovery};

use crate::recovery_request::RecoveryRequest;

pub trait SpaceRecovering {
    type Request: RecoveryRequest;

    fn estimate(&self, volume: &Path) -> io::Result<u64>;

    fn start(&self, volume: &Path, bytes: u64) -> io::Result<Self::Request>;

    fn cancel(&self, request: &Self::Request);
}

impl SpaceRecovering for SpaceRecovery {
    type Request = RecoveryHandle;

    fn estimate(&self, volume: &Path) -> io::Result<u64> {
        SpaceRecovery::estimate(*self, volume)
    }

    fn start(&self, volume: &Path, bytes: u64) -> io::Result<RecoveryHandle> {
        SpaceRecovery::start(*self, volume, bytes)
    }

    fn cancel(&self, request: &RecoveryHandle) {
        SpaceRecovery::cancel(*self, request);
    }
}
