use std::path::PathBuf;

use crate::card_focus::CardFocus;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FocusContext {
    pub folder: Option<PathBuf>,
    pub selected: Vec<String>,
    pub card: Option<CardFocus>,
}

impl FocusContext {
    pub fn none() -> &'static Self {
        static NONE: FocusContext = FocusContext {
            folder: None,
            selected: Vec::new(),
            card: None,
        };
        &NONE
    }
}
