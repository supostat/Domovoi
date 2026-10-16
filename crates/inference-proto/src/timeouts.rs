use std::env;
use std::time::Duration;

use crate::timeout_error::TimeoutError;

const HANDSHAKE_VARIABLE: &str = "DOMOVOI_XPC_HANDSHAKE_TIMEOUT_MS";
const LOAD_VARIABLE: &str = "DOMOVOI_XPC_LOAD_TIMEOUT_MS";
const STALL_VARIABLE: &str = "DOMOVOI_XPC_STALL_TIMEOUT_MS";
const DEFAULT_HANDSHAKE: Duration = Duration::from_secs(5);
const DEFAULT_LOAD: Duration = Duration::from_secs(120);
const DEFAULT_STALL: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timeouts {
    pub handshake: Duration,
    pub load: Duration,
    pub stall: Duration,
}

impl Timeouts {
    pub fn from_environment() -> Result<Self, TimeoutError> {
        Self::from_variables(|name| env::var(name).ok())
    }

    pub fn from_variables(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, TimeoutError> {
        let read = |variable: &'static str, default: Duration| match lookup(variable) {
            None => Ok(default),
            Some(value) => value
                .trim()
                .parse::<u64>()
                .ok()
                .filter(|milliseconds| *milliseconds > 0)
                .map(Duration::from_millis)
                .ok_or(TimeoutError { variable, value }),
        };
        Ok(Self {
            handshake: read(HANDSHAKE_VARIABLE, DEFAULT_HANDSHAKE)?,
            load: read(LOAD_VARIABLE, DEFAULT_LOAD)?,
            stall: read(STALL_VARIABLE, DEFAULT_STALL)?,
        })
    }
}
