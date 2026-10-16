use std::ffi::CStr;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    MissingVersion,
    UnsupportedVersion(u64),
    UnknownKind(String),
    MissingField(&'static CStr),
    InvalidField(&'static CStr),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingVersion => formatter.write_str("the message carries no protocol version"),
            Self::UnsupportedVersion(version) => write!(formatter, "protocol version {version} is not supported"),
            Self::UnknownKind(kind) => write!(formatter, "message kind {kind} is unknown"),
            Self::MissingField(field) => write!(formatter, "field {} is missing", field.to_string_lossy()),
            Self::InvalidField(field) => write!(formatter, "field {} is invalid", field.to_string_lossy()),
        }
    }
}

impl std::error::Error for DecodeError {}
