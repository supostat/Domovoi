use domovoi_theme::format_memory;
use serde::Deserialize;

use crate::model_card_model::ModelCardModel;
use crate::model_card_state::ModelCardState;
use crate::orphan_row::OrphanRow;
use crate::strings::{AND, CATALOG_NOTE, FITS_ANY, FITS_NONE, FITS_ONE, FITS_SEVERAL, SHARED_MEMORY, THIS_MAC};
use crate::unload_delay::UnloadDelay;

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelScreenModel {
    pub enabled: bool,
    pub memory_bytes: u64,
    pub cards: Vec<ModelCardModel>,
    pub unload: UnloadDelay,
    pub source: String,
    #[serde(default)]
    pub orphans: Vec<OrphanRow>,
}

impl ModelScreenModel {
    pub fn fits(&self, card: &ModelCardModel) -> bool {
        card.min_memory_bytes <= self.memory_bytes
    }

    pub fn active(&self) -> Option<&ModelCardModel> {
        self.cards.iter().find(|card| card.state == ModelCardState::Active)
    }

    pub fn memory_line(&self) -> String {
        let mut fitting: Vec<String> = Vec::new();
        for card in self.cards.iter().filter(|card| self.fits(card)) {
            let class = card.class_title.to_lowercase();
            if !fitting.contains(&class) {
                fitting.push(class);
            }
        }
        let fit = if fitting.is_empty() {
            FITS_NONE.to_owned()
        } else if self.cards.iter().all(|card| self.fits(card)) {
            FITS_ANY.to_owned()
        } else if let [single] = fitting.as_slice() {
            format!("{FITS_ONE} {single}")
        } else {
            let (last, first) = fitting.split_last().expect("several classes fit");
            format!("{FITS_SEVERAL} {} {AND} {last}", first.join(", "))
        };
        format!(
            "{THIS_MAC} {} {SHARED_MEMORY} · {fit}",
            format_memory(self.memory_bytes)
        )
    }

    pub fn catalog_note(&self) -> String {
        let models: Vec<String> = self
            .cards
            .iter()
            .map(|card| format!("{} {}", card.class_title.to_lowercase(), card.name))
            .collect();
        format!("{CATALOG_NOTE} {}.", models.join(", "))
    }
}
