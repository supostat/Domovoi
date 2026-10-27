use std::fmt;
use std::io;

use domovoi_rules::DeleteFailure;

use crate::sha256_digest::Sha256Digest;

#[derive(Debug)]
pub enum InstallError {
    InsufficientSpace {
        required: u64,
        available: u64,
    },
    RevisionMismatch {
        expected: String,
        served: String,
    },
    DigestMismatch {
        expected: Sha256Digest,
        received: Sha256Digest,
    },
    SizeMismatch {
        expected: u64,
        received: u64,
    },
    UnexpectedStatus(u16),
    AccessDenied,
    Network(String),
    Io(io::Error),
    Delete(DeleteFailure),
}

impl From<io::Error> for InstallError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl fmt::Display for InstallError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InsufficientSpace { required, available } => write!(
                formatter,
                "the model needs {required} free bytes, the volume has {available}"
            ),
            Self::RevisionMismatch { expected, served } => {
                write!(
                    formatter,
                    "the source serves revision {served}, the catalog pins {expected}"
                )
            }
            Self::DigestMismatch { expected, received } => {
                write!(formatter, "SHA-256 is {received}, the catalog expects {expected}")
            }
            Self::SizeMismatch { expected, received } => {
                write!(
                    formatter,
                    "the source serves {received} bytes, the catalog expects {expected}"
                )
            }
            Self::UnexpectedStatus(status) => write!(formatter, "the source answered HTTP {status}"),
            Self::AccessDenied => formatter.write_str("the source keeps refusing fresh download links"),
            Self::Network(description) => write!(formatter, "the transfer failed: {description}"),
            Self::Io(error) => write!(formatter, "the model file cannot be written: {error}"),
            Self::Delete(failure) => write!(formatter, "the file cannot be deleted: {failure:?}"),
        }
    }
}

impl std::error::Error for InstallError {}
