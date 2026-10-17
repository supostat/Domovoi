use domovoi_downloader::ModelSource;
use objc2_foundation::{NSString, NSUserDefaults};

use crate::strings::SOURCE_NOT_HTTPS;
use crate::unload_delay::UnloadDelay;

const ENABLED: &str = "AssistantEnabled";
const UNLOAD: &str = "ModelUnloadDelay";
const MIRROR: &str = "ModelMirror";
const ACTIVE: &str = "ActiveModel";
const HUGGING_FACE_HOST: &str = "huggingface.co";
const SECURE_SCHEME: &str = "https://";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssistantSettings {
    pub enabled: bool,
    pub unload: UnloadDelay,
    pub mirror: Option<String>,
    pub active_entry: Option<String>,
}

impl AssistantSettings {
    pub fn load() -> Self {
        let defaults = NSUserDefaults::standardUserDefaults();
        let text = |key: &str| {
            defaults
                .stringForKey(&NSString::from_str(key))
                .map(|value| value.to_string())
        };
        Self {
            enabled: defaults.boolForKey(&NSString::from_str(ENABLED)),
            unload: text(UNLOAD)
                .as_deref()
                .and_then(UnloadDelay::from_key)
                .unwrap_or_default(),
            mirror: text(MIRROR),
            active_entry: text(ACTIVE),
        }
    }

    pub fn save(&self) {
        let defaults = NSUserDefaults::standardUserDefaults();
        defaults.setBool_forKey(self.enabled, &NSString::from_str(ENABLED));
        store(&defaults, UNLOAD, Some(self.unload.key()));
        store(&defaults, MIRROR, self.mirror.as_deref());
        store(&defaults, ACTIVE, self.active_entry.as_deref());
    }

    pub fn source(&self) -> ModelSource {
        self.mirror
            .as_deref()
            .map_or_else(ModelSource::hugging_face, ModelSource::mirror)
    }

    pub fn source_host(&self) -> String {
        self.mirror.as_deref().map_or_else(
            || HUGGING_FACE_HOST.to_owned(),
            |mirror| {
                mirror
                    .strip_prefix(SECURE_SCHEME)
                    .unwrap_or(mirror)
                    .trim_end_matches('/')
                    .to_owned()
            },
        )
    }

    pub fn parse_mirror(text: &str) -> Result<Option<String>, &'static str> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        match trimmed.strip_prefix(SECURE_SCHEME) {
            Some(host) if !host.trim_matches('/').is_empty() && !host.contains(char::is_whitespace) => {
                Ok(Some(trimmed.trim_end_matches('/').to_owned()))
            }
            _ => Err(SOURCE_NOT_HTTPS),
        }
    }
}

fn store(defaults: &NSUserDefaults, key: &str, value: Option<&str>) {
    let key = NSString::from_str(key);
    match value {
        Some(value) => unsafe { defaults.setObject_forKey(Some(&NSString::from_str(value)), &key) },
        None => defaults.removeObjectForKey(&key),
    }
}
