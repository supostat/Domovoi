use std::error::Error;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum ScanError {
    Setup(io::Error),
    Root(io::Error),
    Cancelled,
}

impl fmt::Display for ScanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Setup(error) => write!(formatter, "scan setup failed: {error}"),
            Self::Root(error) => write!(formatter, "scan root is not readable: {error}"),
            Self::Cancelled => formatter.write_str("scan was cancelled"),
        }
    }
}

impl Error for ScanError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Setup(error) | Self::Root(error) => Some(error),
            Self::Cancelled => None,
        }
    }
}
