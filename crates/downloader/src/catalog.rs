use std::collections::HashSet;
use std::ffi::CStr;
use std::fs;
use std::path::Path;

use domovoi_inference_proto::{MemoryBudget, ModelClass};
use serde::Deserialize;

use crate::catalog_entry::CatalogEntry;
use crate::catalog_error::CatalogError;

const BUNDLED_CATALOG: &str = include_str!("../../../assets/catalog.json");
const GIBIBYTE: u64 = 1024 * 1024 * 1024;
const STANDARD_FROM: u64 = 16 * GIBIBYTE;
const EXTENDED_FROM: u64 = 32 * GIBIBYTE;
const REVISION_DIGITS: usize = 40;
const PHYSICAL_MEMORY: &CStr = c"hw.memsize";

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub entries: Vec<CatalogEntry>,
}

impl Catalog {
    pub fn bundled() -> Self {
        Self::parse(BUNDLED_CATALOG).expect("the bundled catalog is valid")
    }

    pub fn from_path(path: &Path) -> Result<Self, CatalogError> {
        Self::parse(&fs::read_to_string(path).map_err(CatalogError::Unreadable)?)
    }

    pub fn parse(text: &str) -> Result<Self, CatalogError> {
        let catalog: Self = serde_json::from_str(text).map_err(CatalogError::Malformed)?;
        let mut identifiers = HashSet::new();
        for entry in &catalog.entries {
            let invalid = |problem: &str| CatalogError::Invalid {
                id: entry.id.clone(),
                problem: problem.to_owned(),
            };
            if !identifiers.insert(entry.id.as_str()) {
                return Err(invalid("the id repeats"));
            }
            if entry.size == 0 {
                return Err(invalid("the size is zero"));
            }
            if !is_revision(&entry.revision) {
                return Err(invalid("the revision is not a 40-digit commit hash"));
            }
            if entry.file.contains('/') || entry.file.starts_with('.') {
                return Err(invalid("the file name is not a plain name"));
            }
            if entry.license_text.trim().is_empty() {
                return Err(invalid("the license text is empty"));
            }
        }
        Ok(catalog)
    }

    pub fn entry(&self, id: &str) -> Option<&CatalogEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    pub fn primary(&self, class: ModelClass) -> Option<&CatalogEntry> {
        self.entries.iter().find(|entry| entry.class == class && entry.primary)
    }

    pub fn default_entry(&self, memory_bytes: u64, budget: Option<MemoryBudget>) -> Option<&CatalogEntry> {
        let offered = class_for_memory(memory_bytes);
        let offered_rank = ModelClass::ALL
            .iter()
            .position(|class| *class == offered)
            .expect("every class is listed");
        ModelClass::ALL[..=offered_rank]
            .iter()
            .rev()
            .copied()
            .filter(|class| budget.is_none_or(|budget| budget.fits(*class)))
            .find_map(|class| self.primary(class))
    }

    pub fn default_entry_for_this_mac(&self) -> Option<&CatalogEntry> {
        self.default_entry(physical_memory(), MemoryBudget::of_default_device())
    }
}

fn class_for_memory(memory_bytes: u64) -> ModelClass {
    if memory_bytes < STANDARD_FROM {
        ModelClass::Compact
    } else if memory_bytes < EXTENDED_FROM {
        ModelClass::Standard
    } else {
        ModelClass::Extended
    }
}

fn is_revision(revision: &str) -> bool {
    revision.len() == REVISION_DIGITS
        && revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn physical_memory() -> u64 {
    let mut memory: u64 = 0;
    let mut size = size_of::<u64>();
    let status = unsafe {
        libc::sysctlbyname(
            PHYSICAL_MEMORY.as_ptr(),
            (&raw mut memory).cast(),
            &raw mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    assert_eq!(status, 0, "hw.memsize is always readable");
    memory
}
