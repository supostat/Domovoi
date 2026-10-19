use std::error::Error;
use std::fmt;
use std::io;

use domovoi_index::StaleDelta;
use domovoi_scanner::ScanError;

#[derive(Debug)]
pub enum SessionError {
    Scan(ScanError),
    Io(io::Error),
    Stale(StaleDelta),
}

impl fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scan(error) => error.fmt(formatter),
            Self::Io(error) => error.fmt(formatter),
            Self::Stale(error) => error.fmt(formatter),
        }
    }
}

impl Error for SessionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scan(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::Stale(error) => Some(error),
        }
    }
}

impl From<ScanError> for SessionError {
    fn from(error: ScanError) -> Self {
        Self::Scan(error)
    }
}
