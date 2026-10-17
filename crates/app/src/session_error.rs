use std::error::Error;
use std::fmt;
use std::io;

use domovoi_scanner::ScanError;

#[derive(Debug)]
pub enum SessionError {
    Scan(ScanError),
    Cache(io::Error),
}

impl fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scan(error) => error.fmt(formatter),
            Self::Cache(error) => write!(formatter, "scan cache cannot be written: {error}"),
        }
    }
}

impl Error for SessionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scan(error) => Some(error),
            Self::Cache(error) => Some(error),
        }
    }
}

impl From<ScanError> for SessionError {
    fn from(error: ScanError) -> Self {
        Self::Scan(error)
    }
}
