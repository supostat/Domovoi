use std::path::PathBuf;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FocusContext {
    pub folder: Option<PathBuf>,
    pub selected: Vec<String>,
}

impl FocusContext {
    pub fn none() -> &'static Self {
        static NONE: FocusContext = FocusContext {
            folder: None,
            selected: Vec::new(),
        };
        &NONE
    }
}
