use std::error::Error;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum AccessError {
    Tcc,
    Unix,
    Missing,
    Other(io::Error),
}

impl AccessError {
    pub fn from_errno(errno: i32) -> Self {
        match errno {
            libc::EPERM => Self::Tcc,
            libc::EACCES => Self::Unix,
            libc::ENOENT => Self::Missing,
            other => Self::Other(io::Error::from_raw_os_error(other)),
        }
    }
}

impl From<io::Error> for AccessError {
    fn from(error: io::Error) -> Self {
        match error.raw_os_error() {
            Some(errno @ (libc::EPERM | libc::EACCES | libc::ENOENT)) => Self::from_errno(errno),
            _ => Self::Other(error),
        }
    }
}

impl fmt::Display for AccessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tcc => formatter.write_str("access denied by privacy protection (EPERM)"),
            Self::Unix => formatter.write_str("access denied by file permissions (EACCES)"),
            Self::Missing => formatter.write_str("no such file or directory (ENOENT)"),
            Self::Other(error) => error.fmt(formatter),
        }
    }
}

impl Error for AccessError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Other(error) => Some(error),
            Self::Tcc | Self::Unix | Self::Missing => None,
        }
    }
}
