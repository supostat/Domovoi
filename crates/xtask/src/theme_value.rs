use cssparser::{ParseError, Parser, Token};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TokenValue {
    Color(Color),
    Length(f32),
    Shadow(Shadow),
    FontStack(Vec<String>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Color {
    pub(crate) red: u8,
    pub(crate) green: u8,
    pub(crate) blue: u8,
    pub(crate) alpha: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Shadow {
    pub(crate) offset_x: f32,
    pub(crate) offset_y: f32,
    pub(crate) blur: f32,
    pub(crate) color: Color,
}

type ValueResult<T> = Result<T, ParseError<()>>;

impl TokenValue {
    pub(crate) fn parse(input: &mut Parser<'_>) -> ValueResult<Self> {
        if let Ok(color) = input.try_parse(|input| entire_value(input, color)) {
            return Ok(Self::Color(color));
        }
        if let Ok(length) = input.try_parse(|input| entire_value(input, pixels)) {
            return Ok(Self::Length(length));
        }
        if let Ok(shadow) = input.try_parse(|input| entire_value(input, shadow)) {
            return Ok(Self::Shadow(shadow));
        }
        entire_value(input, |input| input.parse_comma_separated(font_family)).map(Self::FontStack)
    }
}

fn entire_value<'i, T>(
    input: &mut Parser<'i>,
    parse: impl FnOnce(&mut Parser<'i>) -> ValueResult<T>,
) -> ValueResult<T> {
    let value = parse(input)?;
    input.expect_exhausted()?;
    Ok(value)
}

fn color(input: &mut Parser<'_>) -> ValueResult<Color> {
    match input.next()?.clone() {
        Token::Hash(digits) | Token::IDHash(digits) => hex_color(&digits).ok_or_else(|| ParseError::custom(())),
        Token::Function(name) if name.eq_ignore_ascii_case("rgba") => input.parse_nested_block(rgba_arguments),
        _ => Err(ParseError::unexpected_token()),
    }
}

fn hex_color(digits: &str) -> Option<Color> {
    if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |range: std::ops::Range<usize>| u8::from_str_radix(&digits[range], 16).ok();
    Some(Color {
        red: channel(0..2)?,
        green: channel(2..4)?,
        blue: channel(4..6)?,
        alpha: None,
    })
}

fn rgba_arguments(input: &mut Parser<'_>) -> ValueResult<Color> {
    let red = channel(input)?;
    input.expect_comma()?;
    let green = channel(input)?;
    input.expect_comma()?;
    let blue = channel(input)?;
    input.expect_comma()?;
    let alpha = input.expect_number()?;
    if !(0.0..=1.0).contains(&alpha) {
        return Err(ParseError::custom(()));
    }
    Ok(Color {
        red,
        green,
        blue,
        alpha: Some(alpha),
    })
}

fn channel(input: &mut Parser<'_>) -> ValueResult<u8> {
    let value = input.expect_integer()?;
    u8::try_from(value).map_err(|_| ParseError::custom(()))
}

fn pixels(input: &mut Parser<'_>) -> ValueResult<f32> {
    match input.next()?.clone() {
        Token::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("px") => Ok(value),
        _ => Err(ParseError::unexpected_token()),
    }
}

fn pixels_or_zero(input: &mut Parser<'_>) -> ValueResult<f32> {
    if input.try_parse(unitless_zero).is_ok() {
        return Ok(0.0);
    }
    pixels(input)
}

fn unitless_zero(input: &mut Parser<'_>) -> ValueResult<()> {
    if input.expect_number()? == 0.0 {
        Ok(())
    } else {
        Err(ParseError::custom(()))
    }
}

fn shadow(input: &mut Parser<'_>) -> ValueResult<Shadow> {
    Ok(Shadow {
        offset_x: pixels_or_zero(input)?,
        offset_y: pixels_or_zero(input)?,
        blur: pixels_or_zero(input)?,
        color: color(input)?,
    })
}

fn font_family(input: &mut Parser<'_>) -> ValueResult<String> {
    if let Ok(quoted) = input.try_parse(Parser::expect_string_cloned) {
        return Ok(quoted.to_string());
    }
    let mut words = vec![input.expect_ident_cloned()?.to_string()];
    while let Ok(word) = input.try_parse(Parser::expect_ident_cloned) {
        words.push(word.to_string());
    }
    Ok(words.join(" "))
}

#[cfg(test)]
mod tests {
    use cssparser::Parser;

    use super::{Color, Shadow, TokenValue};

    fn parse(source: &str) -> Option<TokenValue> {
        TokenValue::parse(&mut Parser::new(source)).ok()
    }

    #[test]
    fn parses_hex_and_rgba_colors() {
        let wax = Color {
            red: 0xE3,
            green: 0xAE,
            blue: 0x52,
            alpha: None,
        };
        assert_eq!(parse("#E3AE52"), Some(TokenValue::Color(wax)));
        assert_eq!(
            parse("rgba(227,174,82,.14)"),
            Some(TokenValue::Color(Color {
                alpha: Some(0.14),
                ..wax
            }))
        );
    }

    #[test]
    fn parses_lengths_shadows_and_font_stacks() {
        assert_eq!(parse("10px"), Some(TokenValue::Length(10.0)));
        assert_eq!(
            parse("0 22px 60px rgba(0,0,0,.5)"),
            Some(TokenValue::Shadow(Shadow {
                offset_x: 0.0,
                offset_y: 22.0,
                blur: 60.0,
                color: Color {
                    red: 0,
                    green: 0,
                    blue: 0,
                    alpha: Some(0.5),
                },
            }))
        );
        assert_eq!(
            parse("\"Onest\",-apple-system,Times New Roman,sans-serif"),
            Some(TokenValue::FontStack(vec![
                "Onest".to_owned(),
                "-apple-system".to_owned(),
                "Times New Roman".to_owned(),
                "sans-serif".to_owned(),
            ]))
        );
    }

    #[test]
    fn rejects_unsupported_values() {
        assert_eq!(parse("#abc"), None);
        assert_eq!(parse("rgba(300,0,0,.5)"), None);
        assert_eq!(parse("1.5em"), None);
    }
}
