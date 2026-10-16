use std::collections::BTreeSet;

use crate::gbnf_text::{class_character, literal, repeated};

const ESCAPABLE: &str = ".-[](){}|?*+^$/";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Pattern {
    alternatives: Vec<Vec<Term>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Term {
    atom: Atom,
    minimum: u32,
    maximum: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Atom {
    Literal(char),
    Class(Vec<(char, char)>),
    Group(Vec<Vec<Term>>),
}

impl Pattern {
    pub(crate) fn parse(source: &str) -> Result<Self, String> {
        let body = source
            .strip_prefix('^')
            .and_then(|rest| rest.strip_suffix('$'))
            .filter(|body| !body.ends_with('\\'))
            .ok_or_else(|| "a pattern is anchored with ^ and $".to_owned())?;
        let characters: Vec<char> = body.chars().collect();
        let mut parser = Parser {
            characters: &characters,
            position: 0,
        };
        let alternatives = parser.alternation()?;
        if parser.position < characters.len() {
            return Err("unbalanced closing parenthesis".to_owned());
        }
        Ok(Self { alternatives })
    }

    pub(crate) fn gbnf(&self) -> String {
        alternation_expression(&self.alternatives)
    }

    pub(crate) fn matches(&self, text: &str) -> bool {
        let characters: Vec<char> = text.chars().collect();
        alternation_ends(&self.alternatives, &characters, &BTreeSet::from([0])).contains(&characters.len())
    }
}

struct Parser<'source> {
    characters: &'source [char],
    position: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.characters.get(self.position).copied()
    }

    fn next(&mut self) -> Option<char> {
        let character = self.peek();
        self.position += usize::from(character.is_some());
        character
    }

    fn alternation(&mut self) -> Result<Vec<Vec<Term>>, String> {
        let mut alternatives = vec![self.sequence()?];
        while self.peek() == Some('|') {
            self.position += 1;
            alternatives.push(self.sequence()?);
        }
        Ok(alternatives)
    }

    fn sequence(&mut self) -> Result<Vec<Term>, String> {
        let mut terms = Vec::new();
        while let Some(character) = self.peek() {
            if character == '|' || character == ')' {
                break;
            }
            let atom = self.atom()?;
            let (minimum, maximum) = self.quantifier()?;
            terms.push(Term { atom, minimum, maximum });
        }
        Ok(terms)
    }

    fn atom(&mut self) -> Result<Atom, String> {
        match self.next().expect("the caller checked for a character") {
            '(' => {
                if self.peek() == Some('?') {
                    self.position += 1;
                    if self.next() != Some(':') {
                        return Err("only non-capturing groups (?:…) are supported".to_owned());
                    }
                }
                let group = self.alternation()?;
                if self.next() != Some(')') {
                    return Err("a group is not closed".to_owned());
                }
                Ok(Atom::Group(group))
            }
            '[' => self.class(),
            '\\' => self.escape().map(|escaped| match escaped {
                Escaped::Character(character) => Atom::Literal(character),
                Escaped::Digit => Atom::Class(vec![('0', '9')]),
            }),
            '.' => Err("the any-character dot is not supported".to_owned()),
            quantifier @ ('*' | '+' | '?' | '{') => Err(format!("a quantifier {quantifier} has nothing to repeat")),
            anchor @ ('^' | '$') => Err(format!("an anchor {anchor} inside the pattern")),
            character => allowed(character).map(Atom::Literal),
        }
    }

    fn class(&mut self) -> Result<Atom, String> {
        if self.peek() == Some('^') {
            return Err("negated character classes are not supported".to_owned());
        }
        let mut ranges = Vec::new();
        loop {
            let low = match self.next() {
                None => return Err("a character class is not closed".to_owned()),
                Some(']') if !ranges.is_empty() => return Ok(Atom::Class(ranges)),
                Some(']') => return Err("an empty character class".to_owned()),
                Some('\\') => self.class_escape()?,
                Some(character) => allowed(character)?,
            };
            let is_range = self.peek() == Some('-') && self.characters.get(self.position + 1) != Some(&']');
            if is_range {
                self.position += 1;
                let high = match self.next() {
                    Some('\\') => self.class_escape()?,
                    Some(character) => allowed(character)?,
                    None => return Err("a character class is not closed".to_owned()),
                };
                if high < low {
                    return Err(format!("an inverted range {low}-{high}"));
                }
                ranges.push((low, high));
            } else {
                ranges.push((low, low));
            }
        }
    }

    fn class_escape(&mut self) -> Result<char, String> {
        match self.escape()? {
            Escaped::Character(character) => Ok(character),
            Escaped::Digit => Err("\\d inside a character class is not supported".to_owned()),
        }
    }

    fn escape(&mut self) -> Result<Escaped, String> {
        match self.next() {
            Some('d') => Ok(Escaped::Digit),
            Some(character) if ESCAPABLE.contains(character) => Ok(Escaped::Character(character)),
            Some(character) => Err(format!("the escape \\{character} is not supported")),
            None => Err("a dangling backslash".to_owned()),
        }
    }

    fn quantifier(&mut self) -> Result<(u32, u32), String> {
        match self.peek() {
            Some('?') => {
                self.position += 1;
                Ok((0, 1))
            }
            Some('*' | '+') => Err("unbounded repetition is not supported".to_owned()),
            Some('{') => {
                self.position += 1;
                let minimum = self.number()?;
                let maximum = if self.peek() == Some(',') {
                    self.position += 1;
                    if self.peek() == Some('}') {
                        return Err("unbounded repetition is not supported".to_owned());
                    }
                    self.number()?
                } else {
                    minimum
                };
                if self.next() != Some('}') {
                    return Err("a repetition is not closed".to_owned());
                }
                if maximum < minimum {
                    return Err(format!("an inverted repetition {{{minimum},{maximum}}}"));
                }
                Ok((minimum, maximum))
            }
            _ => Ok((1, 1)),
        }
    }

    fn number(&mut self) -> Result<u32, String> {
        let start = self.position;
        while self.peek().is_some_and(|character| character.is_ascii_digit()) {
            self.position += 1;
        }
        self.characters[start..self.position]
            .iter()
            .collect::<String>()
            .parse()
            .map_err(|_| "a repetition count is not a number".to_owned())
    }
}

enum Escaped {
    Character(char),
    Digit,
}

fn allowed(character: char) -> Result<char, String> {
    if character == '"' || character == '\\' || character.is_control() {
        Err(format!("the character {character:?} needs JSON escaping"))
    } else {
        Ok(character)
    }
}

fn alternation_expression(alternatives: &[Vec<Term>]) -> String {
    let rendered: Vec<String> = alternatives.iter().map(|terms| sequence_expression(terms)).collect();
    if rendered.len() == 1 {
        rendered.into_iter().next().expect("one alternative")
    } else {
        format!("({})", rendered.join(" | "))
    }
}

fn sequence_expression(terms: &[Term]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut pending_literal = String::new();
    for term in terms {
        if let (Atom::Literal(character), 1, 1) = (&term.atom, term.minimum, term.maximum) {
            pending_literal.push(*character);
            continue;
        }
        if !pending_literal.is_empty() {
            parts.push(literal(&pending_literal));
            pending_literal.clear();
        }
        let atom = match &term.atom {
            Atom::Literal(character) => literal(&character.to_string()),
            Atom::Class(ranges) => class_expression(ranges),
            Atom::Group(alternatives) => {
                let inner = alternation_expression(alternatives);
                if inner.starts_with('(') {
                    inner
                } else {
                    format!("({inner})")
                }
            }
        };
        parts.push(repeated(&atom, term.minimum, term.maximum));
    }
    if !pending_literal.is_empty() {
        parts.push(literal(&pending_literal));
    }
    if parts.is_empty() {
        "\"\"".to_owned()
    } else {
        parts.join(" ")
    }
}

fn class_expression(ranges: &[(char, char)]) -> String {
    let mut expression = String::from("[");
    for (low, high) in ranges {
        expression.push_str(&class_character(*low));
        if low != high {
            expression.push('-');
            expression.push_str(&class_character(*high));
        }
    }
    expression.push(']');
    expression
}

fn alternation_ends(alternatives: &[Vec<Term>], text: &[char], starts: &BTreeSet<usize>) -> BTreeSet<usize> {
    alternatives
        .iter()
        .flat_map(|terms| sequence_ends(terms, text, starts))
        .collect()
}

fn sequence_ends(terms: &[Term], text: &[char], starts: &BTreeSet<usize>) -> BTreeSet<usize> {
    let mut current = starts.clone();
    for term in terms {
        current = term_ends(term, text, &current);
        if current.is_empty() {
            break;
        }
    }
    current
}

fn term_ends(term: &Term, text: &[char], starts: &BTreeSet<usize>) -> BTreeSet<usize> {
    let mut ends = if term.minimum == 0 {
        starts.clone()
    } else {
        BTreeSet::new()
    };
    let mut frontier = starts.clone();
    for count in 1..=term.maximum {
        frontier = atom_ends(&term.atom, text, &frontier);
        if frontier.is_empty() {
            break;
        }
        if count >= term.minimum {
            ends.extend(&frontier);
        }
    }
    ends
}

fn atom_ends(atom: &Atom, text: &[char], starts: &BTreeSet<usize>) -> BTreeSet<usize> {
    match atom {
        Atom::Literal(expected) => starts
            .iter()
            .filter(|position| text.get(**position) == Some(expected))
            .map(|position| position + 1)
            .collect(),
        Atom::Class(ranges) => starts
            .iter()
            .filter(|position| {
                text.get(**position)
                    .is_some_and(|character| ranges.iter().any(|(low, high)| (low..=high).contains(&character)))
            })
            .map(|position| position + 1)
            .collect(),
        Atom::Group(alternatives) => alternation_ends(alternatives, text, starts),
    }
}

#[cfg(test)]
mod tests {
    use super::Pattern;

    #[test]
    fn anchored_bounded_patterns_compile_and_match() {
        let rule_id = Pattern::parse("^[a-z][a-z0-9.-]{0,63}$").expect("rule id pattern compiles");
        assert_eq!(rule_id.gbnf(), "[a-z] [a-z0-9.\\-]{0,63}");
        assert!(rule_id.matches("cache.homebrew"));
        assert!(!rule_id.matches("Cache"));
        assert!(!rule_id.matches(""));
        let version = Pattern::parse("^v(?:\\d{1,3}\\.){2}\\d{1,3}(-beta)?$").expect("version pattern compiles");
        assert!(version.matches("v1.20.3"));
        assert!(version.matches("v1.20.3-beta"));
        assert!(!version.matches("v1.20"));
    }

    #[test]
    fn unsafe_constructs_are_rejected() {
        for source in [
            "[a-z]{0,3}",
            "^.{0,5}$",
            "^a*$",
            "^a+$",
            "^a{2,}$",
            "^[^a]$",
            "^\"$",
            "^a\\$",
            "^(a$",
            "^\\w$",
        ] {
            assert!(Pattern::parse(source).is_err(), "{source}");
        }
    }
}
