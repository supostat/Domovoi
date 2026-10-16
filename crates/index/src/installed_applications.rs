use std::collections::HashMap;
use std::io;
use std::path::PathBuf;

use crate::app::Installation;
use crate::ffi::{application_path, spotlight_applications};

const SYSTEM_BUNDLE_PREFIX: &str = "com.apple.";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InstalledApplications {
    by_bundle_id: HashMap<String, PathBuf>,
}

impl InstalledApplications {
    pub fn new(by_bundle_id: HashMap<String, PathBuf>) -> Self {
        Self { by_bundle_id }
    }

    pub fn discover() -> io::Result<Self> {
        Ok(Self::new(spotlight_applications()?.into_iter().collect()))
    }

    pub fn resolve(&self, bundle_id: &str) -> Installation {
        if bundle_id.starts_with(SYSTEM_BUNDLE_PREFIX) {
            return Installation::System;
        }
        let owner = self
            .by_bundle_id
            .iter()
            .filter(|(known, _)| is_same_or_nested(bundle_id, known))
            .max_by_key(|(known, _)| known.len())
            .map(|(_, path)| path.clone())
            .or_else(|| application_path(bundle_id));
        match owner {
            Some(path) => Installation::Installed {
                bundle_path: path.to_string_lossy().into_owned(),
            },
            None => Installation::Missing,
        }
    }
}

fn is_same_or_nested(bundle_id: &str, application_id: &str) -> bool {
    bundle_id == application_id
        || bundle_id
            .strip_prefix(application_id)
            .is_some_and(|rest| rest.starts_with('.'))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use super::InstalledApplications;
    use crate::app::Installation;

    #[test]
    fn extensions_belong_to_their_installed_application() {
        let applications = InstalledApplications {
            by_bundle_id: HashMap::from([(
                "com.tinyspeck.slackmacgap".to_owned(),
                PathBuf::from("/Applications/Slack.app"),
            )]),
        };
        assert_eq!(
            applications.resolve("com.tinyspeck.slackmacgap.SlackShare"),
            Installation::Installed {
                bundle_path: "/Applications/Slack.app".to_owned()
            }
        );
        assert_eq!(applications.resolve("com.apple.Notes"), Installation::System);
        assert_eq!(applications.resolve("com.fake.app"), Installation::Missing);
    }
}
