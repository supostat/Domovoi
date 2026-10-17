use domovoi_theme::Palette;

use crate::button_style::ButtonStyle;
use crate::shell_palette::ShellPalette;
use crate::ui_font::interface_font;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ButtonKind {
    Plain,
    Ghost,
}

impl ButtonKind {
    pub(crate) fn small_style(self, palette: &ShellPalette) -> ButtonStyle {
        let tokens: &Palette = palette.tokens;
        match self {
            Self::Plain => ButtonStyle {
                fill: Some(tokens.surface_2.ns_color()),
                border: Some(palette.line()),
                text: tokens.text.ns_color(),
                font: interface_font(12.0, 600.0),
            },
            Self::Ghost => ButtonStyle {
                fill: None,
                border: None,
                text: tokens.mute.ns_color(),
                font: interface_font(12.0, 600.0),
            },
        }
    }
}
