use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaleDelta;

impl fmt::Display for StaleDelta {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the index changed after the delta was requested")
    }
}

impl Error for StaleDelta {}
