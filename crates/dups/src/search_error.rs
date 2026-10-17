use std::error::Error;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum SearchError {
    Cancelled,
    Io(io::Error),
}

impl fmt::Display for SearchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str("the duplicate search was cancelled"),
            Self::Io(error) => write!(formatter, "the duplicate search failed: {error}"),
        }
    }
}

impl Error for SearchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Cancelled => None,
            Self::Io(error) => Some(error),
        }
    }
}
