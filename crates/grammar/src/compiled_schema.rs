use serde_json::Value;

use crate::gbnf_grammar::GbnfGrammar;
use crate::grammar_builder::GrammarBuilder;
use crate::schema_error::{SchemaError, SchemaProblem};
use crate::schema_node::{Placement, SchemaNode};
use crate::schema_violation::SchemaViolation;

const DEFINITIONS: &str = "$defs";
const DIALECT: &str = "$schema";
const ROOT_PATH: &str = "#";

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledSchema {
    root: SchemaNode,
    definitions: Vec<(String, SchemaNode)>,
}

impl CompiledSchema {
    pub fn compile(document: &Value) -> Result<Self, SchemaError> {
        let Some(object) = document.as_object() else {
            return Err(SchemaError::at(ROOT_PATH, SchemaProblem::NotAnObject));
        };
        let definitions = match object.get(DEFINITIONS) {
            None => Vec::new(),
            Some(Value::Object(definitions)) => definitions
                .iter()
                .map(|(name, node)| {
                    SchemaNode::parse(
                        node,
                        &format!("{ROOT_PATH}/{DEFINITIONS}/{name}"),
                        Placement::Definition,
                    )
                    .map(|node| (name.clone(), node))
                })
                .collect::<Result<_, _>>()?,
            Some(_) => {
                return Err(SchemaError::at(
                    &format!("{ROOT_PATH}/{DEFINITIONS}"),
                    SchemaProblem::NotAnObject,
                ));
            }
        };
        let mut root = object.clone();
        root.remove(DEFINITIONS);
        root.remove(DIALECT);
        let root = SchemaNode::parse(&Value::Object(root), ROOT_PATH, Placement::Root)?;
        if let Some(unknown) = root
            .references()
            .into_iter()
            .find(|reference| !definitions.iter().any(|(name, _)| name == reference))
        {
            return Err(SchemaError::at(
                ROOT_PATH,
                SchemaProblem::UnknownReference(unknown.to_owned()),
            ));
        }
        Ok(Self { root, definitions })
    }

    pub fn grammar(&self) -> GbnfGrammar {
        GbnfGrammar::new(GrammarBuilder::new(&self.definitions).build(&self.root))
    }

    pub fn validate(&self, value: &Value) -> Result<(), SchemaViolation> {
        self.root.validate(value, ROOT_PATH, &self.definitions)
    }
}
