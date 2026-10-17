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
        enrichment.record_model_event(event);
        self.derive(enrichment);
    }
}

impl Enrichment {
    pub fn record_model_event(&mut self, event: &ModelEvent) {
        match event {
            ModelEvent::Downloaded { path } => {
                if !self.model_paths.contains(path) {
                    self.model_paths.push(path.clone());
                }
            }
            ModelEvent::Removed { path } => self.model_paths.retain(|known| known != path),
        }
    }
}
