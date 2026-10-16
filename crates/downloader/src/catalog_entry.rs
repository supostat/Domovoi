use std::ffi::CString;

use domovoi_grammar::ChatTemplate;
use domovoi_inference_proto::ModelClass;
use serde::{Deserialize, Deserializer};

use crate::sha256_digest::Sha256Digest;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogEntry {
    pub id: String,
    #[serde(deserialize_with = "class_named")]
    pub class: ModelClass,
    pub repo: String,
    pub revision: String,
    pub file: String,
    pub size: u64,
    pub sha256: Sha256Digest,
    pub license: String,
    pub license_source: String,
    pub min_memory_bytes: u64,
    pub context_length: u32,
    pub primary: bool,
    pub template: ChatTemplate,
    pub license_text: String,
}

fn class_named<'de, D: Deserializer<'de>>(deserializer: D) -> Result<ModelClass, D::Error> {
    let name = String::deserialize(deserializer)?;
    CString::new(name.as_str())
        .ok()
        .and_then(|name| ModelClass::from_name(&name))
        .ok_or_else(|| serde::de::Error::custom(format!("unknown model class {name:?}")))
}
