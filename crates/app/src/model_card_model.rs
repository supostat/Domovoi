use domovoi_downloader::CatalogEntry;
use domovoi_inference_proto::ModelClass;
use domovoi_theme::{format_memory, format_size};
use serde::Deserialize;

use crate::model_card_state::ModelCardState;
use crate::strings::{
    CLASS_COMPACT, CLASS_EXTENDED, CLASS_STANDARD, FROM, MINUTES_SHORT, NOTE_COMPACT, NOTE_EXTENDED, NOTE_STANDARD, OF,
    OF_MEMORY, PER_SECOND, REMAINING, REMAINING_UNDER_MINUTE,
};

const WEIGHTS_EXTENSION: &str = ".gguf";
const QUANTIZATION_PREFIX: char = 'Q';
const SECONDS_PER_MINUTE: u64 = 60;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelCardModel {
    pub entry_id: String,
    pub class_title: String,
    pub name: String,
    pub quantization: String,
    pub size: u64,
    pub min_memory_bytes: u64,
    pub note: String,
    pub state: ModelCardState,
}

impl ModelCardModel {
    pub fn of_entry(entry: &CatalogEntry, state: ModelCardState) -> Self {
        let stem = entry.file.strip_suffix(WEIGHTS_EXTENSION).unwrap_or(&entry.file);
        let (name, quantization) = match stem.rsplit_once('-') {
            Some((name, suffix)) if is_quantization(suffix) => (name, suffix.split('_').next().unwrap_or(suffix)),
            _ => (stem, ""),
        };
        let (class_title, note) = match entry.class {
            ModelClass::Compact => (CLASS_COMPACT, NOTE_COMPACT),
            ModelClass::Standard => (CLASS_STANDARD, NOTE_STANDARD),
            ModelClass::Extended => (CLASS_EXTENDED, NOTE_EXTENDED),
        };
        Self {
            entry_id: entry.id.clone(),
            class_title: class_title.to_owned(),
            name: name.to_owned(),
            quantization: quantization.to_owned(),
            size: entry.size,
            min_memory_bytes: entry.min_memory_bytes,
            note: note.to_owned(),
            state,
        }
    }

    pub fn title(&self) -> String {
        [self.class_title.as_str(), &self.name, &self.quantization]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" · ")
    }

    pub fn memory_requirement(&self) -> String {
        format!("{FROM} {} {OF_MEMORY}", format_memory(self.min_memory_bytes))
    }

    pub fn received(&self) -> Option<u64> {
        match self.state {
            ModelCardState::Downloading { received, .. } | ModelCardState::Paused { received } => Some(received),
            ModelCardState::Available
            | ModelCardState::Failed { .. }
            | ModelCardState::Checking
            | ModelCardState::Installed
            | ModelCardState::Active => None,
        }
    }

    pub fn share(&self) -> Option<f64> {
        self.received()
            .map(|received| (received as f64 / self.size.max(1) as f64).clamp(0.0, 1.0))
    }

    pub fn percent(&self) -> Option<String> {
        self.share().map(|share| format!("{}%", (share * 100.0).floor() as u64))
    }

    pub fn transfer_line(&self) -> Option<String> {
        let received = self.received()?;
        let mut parts = vec![format!("{} {OF} {}", format_size(received), format_size(self.size))];
        if let ModelCardState::Downloading { bytes_per_second, .. } = self.state
            && bytes_per_second > 0
        {
            parts.push(format!("{}{PER_SECOND}", format_size(bytes_per_second)));
            let seconds = self.size.saturating_sub(received) / bytes_per_second;
            parts.push(if seconds < SECONDS_PER_MINUTE {
                REMAINING_UNDER_MINUTE.to_owned()
            } else {
                format!("{REMAINING} ~{} {MINUTES_SHORT}", seconds.div_ceil(SECONDS_PER_MINUTE))
            });
        }
        Some(parts.join(" · "))
    }
}

fn is_quantization(suffix: &str) -> bool {
    let mut characters = suffix.chars();
    characters.next() == Some(QUANTIZATION_PREFIX) && characters.next().is_some_and(|digit| digit.is_ascii_digit())
}
