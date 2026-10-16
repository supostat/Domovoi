use std::io;

use crate::install_error::InstallError;
use crate::stop_request::StopRequest;

#[derive(Debug)]
pub(crate) enum Halt {
    Stopped(StopRequest),
    Failed(InstallError),
}

impl From<InstallError> for Halt {
    fn from(error: InstallError) -> Self {
        Self::Failed(error)
    }
}

impl From<io::Error> for Halt {
    fn from(error: io::Error) -> Self {
        Self::Failed(InstallError::Io(error))
    }
}
