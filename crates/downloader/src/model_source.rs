use crate::catalog_entry::CatalogEntry;

const HUGGING_FACE: &str = "https://huggingface.co";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelSource {
    base: String,
}

impl ModelSource {
    pub fn hugging_face() -> Self {
        Self::mirror(HUGGING_FACE)
    }

    pub fn mirror(base: &str) -> Self {
        Self {
            base: base.trim_end_matches('/').to_owned(),
        }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    pub fn resolve_url(&self, entry: &CatalogEntry) -> String {
        format!("{}/{}/resolve/{}/{}", self.base, entry.repo, entry.revision, entry.file)
    }
}
