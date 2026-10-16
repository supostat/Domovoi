use std::fmt::Write;

use crate::digit_range::integer_range_expression;
use crate::gbnf_text::{literal, repeated, rule_name};
use crate::schema_node::SchemaNode;

const ROOT: &str = "root";
const SPACE: &str = "space ::= \" \"?";
const CHARACTER: &str =
    r#"char ::= [^"\\\x7F\x00-\x1F\uD800-\uDFFF\U00110000-\U003FFFFF] | [\\] (["\\bfnrt] | "u" [0-9a-fA-F]{4})"#;
const INTEGER_DIGITS: &str = r#"("0" | [1-9] [0-9]{0,15})"#;
const FRACTION: &str = r#"("." [0-9]{1,15})?"#;
const ZERO_FRACTION: &str = r#"("." "0"{1,15})?"#;

pub(crate) struct GrammarBuilder<'schema> {
    definitions: &'schema [(String, SchemaNode)],
    rules: Vec<(String, String)>,
    uses_space: bool,
    uses_character: bool,
}

impl<'schema> GrammarBuilder<'schema> {
    pub(crate) fn new(definitions: &'schema [(String, SchemaNode)]) -> Self {
        Self {
            definitions,
            rules: Vec::new(),
            uses_space: false,
            uses_character: false,
        }
    }

    pub(crate) fn build(mut self, root: &SchemaNode) -> String {
        let root_body = self.expression(root, ROOT);
        let mut text = format!("{ROOT} ::= {root_body}\n");
        for (name, body) in &self.rules {
            writeln!(text, "{name} ::= {body}").expect("writing to a string succeeds");
        }
        if self.uses_character {
            text.push_str(CHARACTER);
            text.push('\n');
        }
        if self.uses_space {
            text.push_str(SPACE);
            text.push('\n');
        }
        text
    }

    fn rule(&mut self, node: &SchemaNode, name: &str) -> String {
        if let SchemaNode::Reference(definition) = node {
            return self.definition(definition);
        }
        let body = self.expression(node, name);
        self.rules.push((name.to_owned(), body));
        name.to_owned()
    }

    fn definition(&mut self, definition: &str) -> String {
        let name = rule_name(definition);
        if !self.rules.iter().any(|(existing, _)| *existing == name) {
            let node = &self
                .definitions
                .iter()
                .find(|(candidate, _)| candidate == definition)
                .expect("references are resolved when the schema compiles")
                .1;
            self.rules.push((name.clone(), String::new()));
            let body = self.expression(node, &name);
            self.rules
                .iter_mut()
                .find(|(existing, _)| *existing == name)
                .expect("the definition rule was reserved")
                .1 = body;
        }
        name
    }

    fn expression(&mut self, node: &SchemaNode, name: &str) -> String {
        match node {
            SchemaNode::Object { properties, required } => self.object(properties, required, name),
            SchemaNode::BoundedString {
                minimum_length,
                maximum_length,
            } => {
                self.uses_space = true;
                if *maximum_length == 0 {
                    return r#""\"\"" space"#.to_owned();
                }
                self.uses_character = true;
                format!(
                    r#""\"" {} "\"" space"#,
                    repeated("char", *minimum_length, *maximum_length)
                )
            }
            SchemaNode::PatternString(pattern) => {
                self.uses_space = true;
                format!(r#""\"" {} "\"" space"#, pattern.gbnf())
            }
            SchemaNode::Integer(range) => {
                self.uses_space = true;
                match range {
                    None => format!(r#""-"? {INTEGER_DIGITS} space"#),
                    Some((minimum, maximum)) => format!("{} space", integer_range_expression(*minimum, *maximum)),
                }
            }
            SchemaNode::Number(range) => {
                self.uses_space = true;
                match range {
                    None => format!(r#""-"? {INTEGER_DIGITS} {FRACTION} space"#),
                    Some((minimum, maximum)) if minimum == maximum => {
                        format!(r#""{maximum}" {ZERO_FRACTION} space"#)
                    }
                    Some((minimum, maximum)) => format!(
                        r#"({} {FRACTION} | "{maximum}" {ZERO_FRACTION}) space"#,
                        integer_range_expression(*minimum, maximum - 1)
                    ),
                }
            }
            SchemaNode::Boolean => {
                self.uses_space = true;
                r#"("true" | "false") space"#.to_owned()
            }
            SchemaNode::Enum(values) => {
                self.uses_space = true;
                let literals: Vec<String> = values.iter().map(|value| literal(&value.to_string())).collect();
                if literals.len() == 1 {
                    format!("{} space", literals[0])
                } else {
                    format!("({}) space", literals.join(" | "))
                }
            }
            SchemaNode::Array {
                items,
                minimum_items,
                maximum_items,
            } => self.array(items, *minimum_items, *maximum_items, name),
            SchemaNode::Alternatives { options, .. } => {
                let names: Vec<String> = options
                    .iter()
                    .enumerate()
                    .map(|(position, option)| self.rule(option, &format!("{name}-{position}")))
                    .collect();
                format!("({})", names.join(" | "))
            }
            SchemaNode::Reference(definition) => self.definition(definition),
        }
    }

    fn object(&mut self, properties: &[(String, SchemaNode)], required: &[String], name: &str) -> String {
        self.uses_space = true;
        let pairs: Vec<(String, bool)> = properties
            .iter()
            .map(|(property, node)| {
                let value_name = format!("{name}-{}", rule_name(property));
                let pair_name = format!("{value_name}-kv");
                let value = self.rule(node, &value_name);
                let key = literal(&serde_json::Value::String(property.clone()).to_string());
                self.rules
                    .push((pair_name.clone(), format!("{key} space \":\" space {value}")));
                (pair_name, required.contains(property))
            })
            .collect();
        if pairs.is_empty() {
            return r#""{" space "}" space"#.to_owned();
        }
        if pairs.iter().all(|(_, is_required)| *is_required) {
            let members: Vec<&str> = pairs.iter().map(|(pair, _)| pair.as_str()).collect();
            return format!(r#""{{" space {} "}}" space"#, members.join(r#" "," space "#));
        }
        let tails = self.tails(&pairs, name);
        let head = head_expression(&pairs, &tails);
        format!(r#""{{" space {head} "}}" space"#)
    }

    fn tails(&mut self, pairs: &[(String, bool)], name: &str) -> Vec<String> {
        let mut tails = vec![String::new(); pairs.len() + 1];
        for position in (1..pairs.len()).rev() {
            let (pair, is_required) = &pairs[position];
            let member = format!(r#""," space {pair}"#);
            let member = if *is_required { member } else { format!("({member})?") };
            let body = if tails[position + 1].is_empty() {
                member
            } else {
                format!("{member} {}", tails[position + 1])
            };
            let tail_name = format!("{name}-tail-{position}");
            self.rules.push((tail_name.clone(), body));
            tails[position] = tail_name;
        }
        tails
    }

    fn array(&mut self, items: &SchemaNode, minimum: u32, maximum: u32, name: &str) -> String {
        self.uses_space = true;
        if maximum == 0 {
            return r#""[" space "]" space"#.to_owned();
        }
        let item = self.rule(items, &format!("{name}-item"));
        let rest = format!(r#"("," space {item})"#);
        let tail = if maximum == 1 {
            String::new()
        } else {
            format!(" {}", repeated(&rest, minimum.saturating_sub(1), maximum - 1))
        };
        if minimum == 0 {
            format!(r#""[" space ({item}{tail})? "]" space"#)
        } else {
            format!(r#""[" space {item}{tail} "]" space"#)
        }
    }
}

fn head_expression(pairs: &[(String, bool)], tails: &[String]) -> String {
    let mut options = Vec::new();
    for (position, (pair, is_required)) in pairs.iter().enumerate() {
        let tail = &tails[position + 1];
        options.push(if tail.is_empty() {
            pair.clone()
        } else {
            format!("{pair} {tail}")
        });
        if *is_required {
            return format!("({})", options.join(" | "));
        }
    }
    format!("({})?", options.join(" | "))
}
