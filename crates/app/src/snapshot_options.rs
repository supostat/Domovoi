use std::path::PathBuf;

use crate::screen::Screen;

#[derive(Clone, Debug, PartialEq)]
pub struct SnapshotOptions {
    pub screen: Screen,
    pub dark: bool,
    pub scale: f64,
    pub high_contrast: bool,
    pub directory: PathBuf,
}

impl SnapshotOptions {
    pub fn file_stem(&self) -> String {
        let theme = if self.dark { "dark" } else { "light" };
        let contrast = if self.high_contrast { "-contrast" } else { "" };
        let variant = self.screen.variant().map(|name| format!("-{name}")).unwrap_or_default();
        format!(
            "{}{variant}-{theme}@{}x{contrast}",
            self.screen.number(),
            self.scale as u32
        )
    }
}
