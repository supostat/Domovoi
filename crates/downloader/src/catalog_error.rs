use std::fmt;
use std::io;

#[derive(Debug)]
pub enum CatalogError {
    Unreadable(io::Error),
    Malformed(serde_json::Error),
    Invalid { id: String, problem: String },
}

impl fmt::Display for CatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable(error) => write!(formatter, "the catalog cannot be read: {error}"),
            Self::Malformed(error) => write!(formatter, "the catalog is not valid JSON of entries: {error}"),
            Self::Invalid { id, problem } => write!(formatter, "catalog entry {id:?}: {problem}"),
        }
    }
}

impl std::error::Error for CatalogError {}
