use std::path::PathBuf;

use crate::enrichment::Enrichment;
use crate::index::Index;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelEvent {
    Downloaded { path: PathBuf },
    Removed { path: PathBuf },
}

impl Index {
    pub fn apply_model_event(&mut self, event: &ModelEvent, enrichment: &mut Enrichment) {
        match event {
            ModelEvent::Downloaded { path } => {
                if !enrichment.model_paths.contains(path) {
                    enrichment.model_paths.push(path.clone());
                }
            }
            ModelEvent::Removed { path } => enrichment.model_paths.retain(|known| known != path),
        }
        self.derive(enrichment);
    }
}
