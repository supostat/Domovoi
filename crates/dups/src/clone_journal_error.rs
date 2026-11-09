use std::error::Error;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum CloneJournalError {
    Io(io::Error),
    Corrupt(String),
}

impl fmt::Display for CloneJournalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "clone journal is unreadable: {error}"),
            Self::Corrupt(reason) => write!(formatter, "clone journal is corrupt: {reason}"),
        }
    }
}

impl Error for CloneJournalError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Corrupt(_) => None,
        }
    }
}
