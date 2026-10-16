use std::io;
use std::path::PathBuf;

use domovoi_rules::TrashFailure;

use crate::clone_refusal::CloneRefusal;

#[derive(Debug)]
pub enum CloneReplaceError {
    Refused(CloneRefusal),
    OldBodyNotTrashed { location: PathBuf, failure: TrashFailure },
    Io(io::Error),
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
