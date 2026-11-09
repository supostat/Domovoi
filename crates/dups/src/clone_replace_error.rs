use std::io;
use std::path::PathBuf;

use domovoi_rules::DeleteFailure;

use crate::clone_refusal::CloneRefusal;

#[derive(Debug)]
pub enum CloneReplaceError {
    Refused(CloneRefusal),
    OldBodyNotDeleted { location: PathBuf, failure: DeleteFailure },
    Io(io::Error),
    Stopped,
}

impl From<io::Error> for CloneReplaceError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<CloneRefusal> for CloneReplaceError {
    fn from(refusal: CloneRefusal) -> Self {
        Self::Refused(refusal)
    }
}
