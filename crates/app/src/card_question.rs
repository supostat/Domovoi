use domovoi_agent::CardFocus;
use domovoi_index::SafetyLevel;

use crate::card_model::CardModel;
use crate::provenance::Provenance;

pub fn card_focus(card: &CardModel) -> CardFocus {
    CardFocus {
        rule: match &card.provenance {
            Provenance::Rule(id) => Some(id.clone()),
            Provenance::Hypothesis | Provenance::Projects => None,
        },
        title: card.title.clone(),
        level: SafetyLevel::from(card.level),
        paths: card.paths.clone(),
        bytes: card.amount,
    }
}
