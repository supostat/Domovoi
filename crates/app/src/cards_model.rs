use std::cell::{Cell, RefCell};

use objc2::rc::Retained;

use crate::card_model::CardModel;
use crate::shell_actions::ShellActions;
use crate::shell_palette::ShellPalette;

#[derive(Debug)]
pub struct CardsModel {
    pub cards: RefCell<Vec<CardModel>>,
    pub inset: f64,
    pub palette: ShellPalette,
    pub actions: Retained<ShellActions>,
    pub heights: RefCell<Option<(f64, Vec<f64>)>>,
    pub measurements: Cell<usize>,
}
