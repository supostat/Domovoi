use std::collections::BTreeSet;
use std::io;

use crate::theme_stylesheet::{Declaration, ThemeBlocks};
use crate::theme_value::{Color, Shadow, TokenValue};

const RADIUS_PREFIX: &str = "r-";
const FONT_STACK_PREFIX: &str = "f-";
const WINDOW_MOCKUP_PREFIX: &str = "tl-";

#[derive(Debug, PartialEq)]
pub(crate) struct ThemeTokens {
    pub(crate) palette: Vec<PaletteToken>,
    pub(crate) radii: Vec<(String, f32)>,
    pub(crate) font_stacks: Vec<(String, Vec<String>)>,
}

#[derive(Debug, PartialEq)]
pub(crate) struct PaletteToken {
    pub(crate) name: String,
    pub(crate) dark: PaletteValue,
    pub(crate) light: PaletteValue,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PaletteValue {
    Color(Color),
    Shadow(Shadow),
}

impl ThemeTokens {
    pub(crate) fn from_blocks(blocks: &ThemeBlocks) -> io::Result<Self> {
        let mut problems = Vec::new();
        problems.extend(duplicated_names(&blocks.dark, "dark"));
        problems.extend(duplicated_names(&blocks.light, "light"));
        let mut tokens = Self {
            palette: Vec::new(),
            radii: Vec::new(),
            font_stacks: Vec::new(),
        };
        for dark in &blocks.dark {
            let light = blocks.light.iter().find(|light| light.name == dark.name);
            if let Err(problem) = tokens.add(dark, light) {
                problems.push(problem);
            }
        }
        for light in &blocks.light {
            if !blocks.dark.iter().any(|dark| dark.name == light.name) {
                problems.push(format!("--{} is defined only in the light theme", light.name));
            }
        }
        if problems.is_empty() {
            Ok(tokens)
        } else {
            Err(io::Error::new(io::ErrorKind::InvalidData, problems.join("\n")))
        }
    }

    fn add(&mut self, dark: &Declaration, light: Option<&Declaration>) -> Result<(), String> {
        let name = dark.name.as_str();
        if !is_token_name(name) {
            return Err(format!("--{name} is not a lowercase hyphenated token name"));
        }
        if name.starts_with(WINDOW_MOCKUP_PREFIX) {
            return Ok(());
        }
        if let Some(radius) = name.strip_prefix(RADIUS_PREFIX) {
            let TokenValue::Length(length) = shared_value(dark, light)? else {
                return Err(format!("--{name} must be a px length"));
            };
            self.radii.push((radius.to_owned(), *length));
            return Ok(());
        }
        if let Some(role) = name.strip_prefix(FONT_STACK_PREFIX) {
            let TokenValue::FontStack(families) = shared_value(dark, light)? else {
                return Err(format!("--{name} must be a font family list"));
            };
            self.font_stacks.push((role.to_owned(), families.clone()));
            return Ok(());
        }
        let light = light.ok_or_else(|| format!("--{name} is missing in the light theme"))?;
        match (palette_value(&dark.value), palette_value(&light.value)) {
            (Some(dark_value @ PaletteValue::Color(_)), Some(light_value @ PaletteValue::Color(_)))
            | (Some(dark_value @ PaletteValue::Shadow(_)), Some(light_value @ PaletteValue::Shadow(_))) => {
                self.palette.push(PaletteToken {
                    name: name.to_owned(),
                    dark: dark_value,
                    light: light_value,
                });
                Ok(())
            }
            _ => Err(format!(
                "--{name} must be a color or a shadow of the same kind in both themes"
            )),
        }
    }
}

fn shared_value<'a>(dark: &'a Declaration, light: Option<&Declaration>) -> Result<&'a TokenValue, String> {
    match light {
        Some(_) => Err(format!(
            "--{} is shared by both themes and must not be redefined in the light theme",
            dark.name
        )),
        None => Ok(&dark.value),
    }
}

fn palette_value(value: &TokenValue) -> Option<PaletteValue> {
    match value {
        TokenValue::Color(color) => Some(PaletteValue::Color(*color)),
        TokenValue::Shadow(shadow) => Some(PaletteValue::Shadow(*shadow)),
        TokenValue::Length(_) | TokenValue::FontStack(_) => None,
    }
}

fn duplicated_names(declarations: &[Declaration], theme: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    declarations
        .iter()
        .filter(|declaration| !seen.insert(declaration.name.as_str()))
        .map(|declaration| format!("--{} is defined twice in the {theme} theme", declaration.name))
        .collect()
}

fn is_token_name(name: &str) -> bool {
    name.starts_with(|character: char| character.is_ascii_lowercase())
        && name
            .chars()
            .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-')
}

#[cfg(test)]
mod tests {
    use super::{PaletteValue, ThemeTokens};
    use crate::theme_stylesheet::theme_blocks;
    use crate::theme_value::Color;

    fn tokens(stylesheet: &str) -> Result<ThemeTokens, String> {
        let blocks = theme_blocks(stylesheet).expect("stylesheet parses");
        ThemeTokens::from_blocks(&blocks).map_err(|error| error.to_string())
    }

    #[test]
    fn splits_palette_from_shared_tokens_and_drops_window_mockup_colors() {
        let tokens = tokens(
            ":root{--wax:#E3AE52; --r-m:10px; --f-ui:\"Onest\",sans-serif; --tl-r:#FF5F57;}\
             :root[data-theme=\"light\"]{--wax:#A66C10;}",
        )
        .expect("tokens are consistent");
        assert_eq!(tokens.palette.len(), 1);
        assert_eq!(
            tokens.palette[0].light,
            PaletteValue::Color(Color {
                red: 0xA6,
                green: 0x6C,
                blue: 0x10,
                alpha: None,
            })
        );
        assert_eq!(tokens.radii, vec![("m".to_owned(), 10.0)]);
        assert_eq!(
            tokens.font_stacks,
            vec![("ui".to_owned(), vec!["Onest".to_owned(), "sans-serif".to_owned()])]
        );
    }

    #[test]
    fn fails_when_a_token_is_missing_in_one_theme() {
        let missing_light =
            tokens(":root{--wax:#E3AE52; --berry:#C9566E;} :root[data-theme=\"light\"]{--wax:#A66C10;}");
        assert!(missing_light.is_err_and(|error| error.contains("--berry is missing in the light theme")));
        let missing_dark = tokens(":root{--wax:#E3AE52;} :root[data-theme=\"light\"]{--wax:#A66C10; --mute:#535A6F;}");
        assert!(missing_dark.is_err_and(|error| error.contains("--mute is defined only in the light theme")));
    }

    #[test]
    fn fails_when_a_shared_token_is_redefined_or_kinds_differ() {
        let redefined = tokens(":root{--r-s:6px;} :root[data-theme=\"light\"]{--r-s:4px;}");
        assert!(redefined.is_err_and(|error| error.contains("--r-s is shared")));
        let mismatched =
            tokens(":root{--shadow:0 1px 2px rgba(0,0,0,.5);} :root[data-theme=\"light\"]{--shadow:#FFFFFF;}");
        assert!(mismatched.is_err_and(|error| error.contains("--shadow must be a color or a shadow")));
    }
}
