use std::env;
use std::time::Duration;

const TIMEOUT_VARIABLE: &str = "DOMOVOI_TEST_TIMEOUT_SECS";
const DEFAULT_TIMEOUT_SECONDS: u64 = 10;

pub fn test_timeout() -> Duration {
    let seconds = env::var(TIMEOUT_VARIABLE)
        .ok()
        .map_or(DEFAULT_TIMEOUT_SECONDS, |value| {
            value
                .parse()
                .unwrap_or_else(|_| panic!("{TIMEOUT_VARIABLE} must be a whole number of seconds, got {value:?}"))
        });
    Duration::from_secs(seconds)
}
