use std::io;

use cssparser::{
    AtRuleParser, CowRcStr, DeclarationParser, ParseError, Parser, ParserState, QualifiedRuleParser,
    RuleBodyItemParser, RuleBodyParser, StyleSheetParser,
};

use crate::theme_value::TokenValue;

const DARK_SELECTOR: &str = ":root";
const LIGHT_SELECTOR: &str = ":root[data-theme=\"light\"]";
const CUSTOM_PROPERTY_PREFIX: &str = "--";

#[derive(Debug, PartialEq)]
pub(crate) struct Declaration {
    pub(crate) name: String,
    pub(crate) value: TokenValue,
}

#[derive(Debug, Default)]
pub(crate) struct ThemeBlocks {
    pub(crate) dark: Vec<Declaration>,
    pub(crate) light: Vec<Declaration>,
}

#[derive(Clone, Copy, Debug)]
enum ThemeBlock {
    Dark,
    Light,
}

type ThemeRule = Option<(ThemeBlock, Vec<Declaration>)>;

pub(crate) fn theme_blocks(stylesheet: &str) -> io::Result<ThemeBlocks> {
    let mut parser = Parser::new(stylesheet);
    let mut blocks = ThemeBlocks::default();
    for rule in StyleSheetParser::new(&mut parser, &mut StylesheetRules) {
        match rule {
            Ok(Some((ThemeBlock::Dark, declarations))) => blocks.dark.extend(declarations),
            Ok(Some((ThemeBlock::Light, declarations))) => blocks.light.extend(declarations),
            Ok(None) => {}
            Err((error, text, location)) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("<style> line {}: {error} in `{}`", location.line + 1, text.trim()),
                ));
            }
        }
    }
    Ok(blocks)
}

fn skip_remaining(input: &mut Parser<'_>) {
    while input.next().is_ok() {}
}

struct StylesheetRules;

impl<'i> QualifiedRuleParser<'i> for StylesheetRules {
    type Prelude = Option<ThemeBlock>;
    type QualifiedRule = ThemeRule;
    type Error = String;

    fn parse_prelude(&mut self, input: &mut Parser<'i>) -> Result<Self::Prelude, ParseError<String>> {
        let start = input.position();
        skip_remaining(input);
        Ok(match input.slice_from(start).trim() {
            DARK_SELECTOR => Some(ThemeBlock::Dark),
            LIGHT_SELECTOR => Some(ThemeBlock::Light),
            _ => None,
        })
    }

    fn parse_block(
        &mut self,
        prelude: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i>,
    ) -> Result<ThemeRule, ParseError<String>> {
        let Some(block) = prelude else {
            skip_remaining(input);
            return Ok(None);
        };
        let mut declarations = Vec::new();
        for item in RuleBodyParser::new(input, &mut TokenDeclarations) {
            match item {
                Ok(Some(declaration)) => declarations.push(declaration),
                Ok(None) => {}
                Err((error, _, location)) => {
                    return Err(ParseError::custom(format!("{error} (line {})", location.line + 1)));
                }
            }
        }
        Ok(Some((block, declarations)))
    }
}

impl<'i> AtRuleParser<'i> for StylesheetRules {
    type Prelude = ();
    type AtRule = ThemeRule;
    type Error = String;

    fn parse_prelude(&mut self, _name: CowRcStr<'i>, input: &mut Parser<'i>) -> Result<(), ParseError<String>> {
        skip_remaining(input);
        Ok(())
    }

    fn rule_without_block(&mut self, _prelude: (), _start: &ParserState) -> Result<ThemeRule, ()> {
        Ok(None)
    }

    fn parse_block(
        &mut self,
        _prelude: (),
        _start: &ParserState,
        input: &mut Parser<'i>,
    ) -> Result<ThemeRule, ParseError<String>> {
        skip_remaining(input);
        Ok(None)
    }
}

struct TokenDeclarations;

impl<'i> DeclarationParser<'i> for TokenDeclarations {
    type Declaration = Option<Declaration>;
    type Error = String;

    fn parse_value(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i>,
        _declaration_start: &ParserState,
    ) -> Result<Option<Declaration>, ParseError<String>> {
        let Some(token_name) = name.strip_prefix(CUSTOM_PROPERTY_PREFIX) else {
            skip_remaining(input);
            return Ok(None);
        };
        let value_start = input.state();
        skip_remaining(input);
        let value_text = input.slice_from(value_start.position()).trim().to_owned();
        input.reset(&value_start);
        let value = TokenValue::parse(input)
            .map_err(|_| ParseError::custom(format!("unsupported value of --{token_name}: `{value_text}`")))?;
        Ok(Some(Declaration {
            name: token_name.to_owned(),
            value,
        }))
    }
}

impl AtRuleParser<'_> for TokenDeclarations {
    type Prelude = ();
    type AtRule = Option<Declaration>;
    type Error = String;
}

impl QualifiedRuleParser<'_> for TokenDeclarations {
    type Prelude = ();
    type QualifiedRule = Option<Declaration>;
    type Error = String;
}

impl RuleBodyItemParser<'_, Option<Declaration>, String> for TokenDeclarations {
    fn parse_declarations(&self) -> bool {
        true
    }

    fn parse_qualified(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::theme_blocks;
    use crate::theme_value::TokenValue;

    #[test]
    fn collects_dark_and_light_blocks_and_skips_everything_else() {
        let stylesheet = "/* note */ :root{--r-s:6px; color-scheme:dark;}\n\
            @media (prefers-color-scheme: light){:root:not([data-theme=\"dark\"]){--r-s:1px;}}\n\
            :root[data-theme=\"light\"]{--r-s:8px;}\nbody{color:var(--text)}";
        let blocks = theme_blocks(stylesheet).expect("stylesheet parses");
        let names_and_values = |declarations: &[super::Declaration]| {
            declarations
                .iter()
                .map(|declaration| (declaration.name.clone(), declaration.value.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names_and_values(&blocks.dark),
            vec![("r-s".to_owned(), TokenValue::Length(6.0))]
        );
        assert_eq!(
            names_and_values(&blocks.light),
            vec![("r-s".to_owned(), TokenValue::Length(8.0))]
        );
    }

    #[test]
    fn fails_on_an_unsupported_token_value() {
        let error = theme_blocks(":root{--wax:#abc;}").expect_err("short hex is rejected");
        assert!(error.to_string().contains("--wax"), "{error}");
    }
}
