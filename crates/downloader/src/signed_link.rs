use std::time::{Duration, SystemTime, UNIX_EPOCH};

const EXPIRES_PARAMETER: &str = "Expires=";
const EXPIRY_MARGIN: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SignedLink {
    pub(crate) url: String,
    expires_at: Option<SystemTime>,
}

impl SignedLink {
    pub(crate) fn new(url: String) -> Self {
        let expires_at = url
            .split_once('?')
            .and_then(|(_, query)| {
                query
                    .split('&')
                    .find_map(|parameter| parameter.strip_prefix(EXPIRES_PARAMETER))
            })
            .and_then(|seconds| seconds.parse::<u64>().ok())
            .map(|seconds| UNIX_EPOCH + Duration::from_secs(seconds));
        Self { url, expires_at }
    }

    pub(crate) fn has_expired(&self) -> bool {
        self.expires_at
            .is_some_and(|expires_at| SystemTime::now() + EXPIRY_MARGIN >= expires_at)
    }
}
