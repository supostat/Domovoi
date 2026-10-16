use std::ops::RangeInclusive;

use serde_json::{Map, Value};

use crate::pattern::Pattern;
use crate::schema_error::{SchemaError, SchemaProblem};
use crate::schema_violation::SchemaViolation;

const STRING_LENGTH_LIMIT: u64 = 2000;
const ANNOTATIONS: [&str; 2] = ["title", "description"];
const DEFINITION_PREFIX: &str = "#/$defs/";

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SchemaNode {
    Object {
        properties: Vec<(String, SchemaNode)>,
        required: Vec<String>,
    },
    BoundedString {
        minimum_length: u32,
        maximum_length: u32,
    },
    PatternString(Pattern),
    Integer(Option<(i64, i64)>),
    Number(Option<(i64, i64)>),
    Boolean,
    Enum(Vec<Value>),
    Array {
        items: Box<SchemaNode>,
        minimum_items: u32,
        maximum_items: u32,
    },
    Alternatives {
        exclusive: bool,
        options: Vec<SchemaNode>,
    },
    Reference(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Placement {
    Root,
    Definition,
}

impl SchemaNode {
    pub(crate) fn parse(value: &Value, path: &str, placement: Placement) -> Result<Self, SchemaError> {
        let fail = |problem| Err(SchemaError::at(path, problem));
        let Some(schema) = value.as_object() else {
            return fail(SchemaProblem::NotAnObject);
        };
        if schema.contains_key("patternProperties") {
            return fail(SchemaProblem::PatternProperties);
        }
        if let Some(reference) = schema.get("$ref") {
            return parse_reference(schema, reference, path, placement);
        }
        if let Some((keyword, options)) = ["oneOf", "anyOf"]
            .iter()
            .find_map(|keyword| schema.get(*keyword).map(|options| (*keyword, options)))
        {
            if schema.contains_key("properties") {
                return fail(SchemaProblem::AlternativesBesideProperties);
            }
            only_keywords(schema, &[keyword], path)?;
            let options = options
                .as_array()
                .ok_or_else(|| SchemaError::at(path, SchemaProblem::NotAnObject))?
                .iter()
                .enumerate()
                .map(|(position, option)| Self::parse(option, &format!("{path}/{keyword}/{position}"), placement))
                .collect::<Result<_, _>>()?;
            return Ok(Self::Alternatives {
                exclusive: keyword == "oneOf",
                options,
            });
        }
        if let Some(values) = schema.get("enum") {
            only_keywords(schema, &["enum", "type"], path)?;
            return parse_enum(values, path);
        }
        match schema.get("type").map(Value::as_str) {
            None => fail(SchemaProblem::MissingType),
            Some(Some("object")) => parse_object(schema, path, placement),
            Some(Some("string")) => parse_string(schema, path),
            Some(Some("integer")) => {
                only_keywords(schema, &["type", "minimum", "maximum"], path)?;
                Ok(Self::Integer(bounds(schema, path)?))
            }
            Some(Some("number")) => {
                only_keywords(schema, &["type", "minimum", "maximum"], path)?;
                let range = bounds(schema, path)?;
                if range.is_some_and(|(minimum, _)| minimum < 0) {
                    return fail(SchemaProblem::NegativeNumberBound);
                }
                Ok(Self::Number(range))
            }
            Some(Some("boolean")) => {
                only_keywords(schema, &["type"], path)?;
                Ok(Self::Boolean)
            }
            Some(Some("array")) => parse_array(schema, path, placement),
            Some(Some(other)) => fail(SchemaProblem::UnknownType(other.to_owned())),
            Some(None) => fail(SchemaProblem::UnknownType(schema["type"].to_string())),
        }
    }

    pub(crate) fn validate(
        &self,
        value: &Value,
        path: &str,
        definitions: &[(String, SchemaNode)],
    ) -> Result<(), SchemaViolation> {
        let violation = |reason: &str| Err(SchemaViolation::at(path, reason));
        match self {
            Self::Object { properties, required } => validate_object(properties, required, value, path, definitions),
            Self::BoundedString {
                minimum_length,
                maximum_length,
            } => {
                let Some(text) = value.as_str() else {
                    return violation("expected a string");
                };
                let length = text.chars().count();
                if length < *minimum_length as usize || length > *maximum_length as usize {
                    return violation(&format!(
                        "length {length} is outside {minimum_length}..={maximum_length}"
                    ));
                }
                Ok(())
            }
            Self::PatternString(pattern) => match value.as_str() {
                Some(text) if pattern.matches(text) => Ok(()),
                Some(_) => violation("the string does not match the pattern"),
                None => violation("expected a string"),
            },
            Self::Integer(range) => match value.as_i64() {
                Some(number) if range.is_none_or(|(minimum, maximum)| (minimum..=maximum).contains(&number)) => Ok(()),
                Some(number) => violation(&format!("{number} is out of range")),
                None => violation("expected an integer"),
            },
            Self::Number(range) => match value.as_f64() {
                Some(number)
                    if range.is_none_or(|(minimum, maximum)| (minimum as f64..=maximum as f64).contains(&number)) =>
                {
                    Ok(())
                }
                Some(number) => violation(&format!("{number} is out of range")),
                None => violation("expected a number"),
            },
            Self::Boolean => {
                if value.is_boolean() {
                    Ok(())
                } else {
                    violation("expected a boolean")
                }
            }
            Self::Enum(values) => {
                if values.contains(value) {
                    Ok(())
                } else {
                    violation(&format!("{value} is not one of the allowed values"))
                }
            }
            Self::Array {
                items,
                minimum_items,
                maximum_items,
            } => validate_array(items, *minimum_items..=*maximum_items, value, path, definitions),
            Self::Alternatives { exclusive, options } => {
                let matching = options
                    .iter()
                    .filter(|option| option.validate(value, path, definitions).is_ok())
                    .count();
                match (matching, exclusive) {
                    (0, _) => violation("no alternative matches"),
                    (1, _) | (_, false) => Ok(()),
                    (_, true) => violation("more than one alternative matches"),
                }
            }
            Self::Reference(name) => definitions
                .iter()
                .find(|(definition, _)| definition == name)
                .expect("references are resolved when the schema compiles")
                .1
                .validate(value, path, definitions),
        }
    }

    pub(crate) fn references(&self) -> Vec<&str> {
        match self {
            Self::Reference(name) => vec![name.as_str()],
            Self::Object { properties, .. } => properties.iter().flat_map(|(_, node)| node.references()).collect(),
            Self::Array { items, .. } => items.references(),
            Self::Alternatives { options, .. } => options.iter().flat_map(Self::references).collect(),
            _ => Vec::new(),
        }
    }
}

fn validate_object(
    properties: &[(String, SchemaNode)],
    required: &[String],
    value: &Value,
    path: &str,
    definitions: &[(String, SchemaNode)],
) -> Result<(), SchemaViolation> {
    let Some(object) = value.as_object() else {
        return Err(SchemaViolation::at(path, "expected an object"));
    };
    if let Some(unexpected) = object
        .keys()
        .find(|key| !properties.iter().any(|(name, _)| name == *key))
    {
        return Err(SchemaViolation::at(path, format!("unexpected property {unexpected}")));
    }
    if let Some(missing) = required.iter().find(|name| !object.contains_key(*name)) {
        return Err(SchemaViolation::at(path, format!("missing property {missing}")));
    }
    for (name, node) in properties {
        if let Some(member) = object.get(name) {
            node.validate(member, &format!("{path}/{name}"), definitions)?;
        }
    }
    Ok(())
}

fn validate_array(
    items: &SchemaNode,
    allowed_count: RangeInclusive<u32>,
    value: &Value,
    path: &str,
    definitions: &[(String, SchemaNode)],
) -> Result<(), SchemaViolation> {
    let Some(elements) = value.as_array() else {
        return Err(SchemaViolation::at(path, "expected an array"));
    };
    if !u32::try_from(elements.len()).is_ok_and(|count| allowed_count.contains(&count)) {
        return Err(SchemaViolation::at(
            path,
            format!("{} items is outside {allowed_count:?}", elements.len()),
        ));
    }
    for (position, element) in elements.iter().enumerate() {
        items.validate(element, &format!("{path}/{position}"), definitions)?;
    }
    Ok(())
}

fn parse_reference(
    schema: &Map<String, Value>,
    reference: &Value,
    path: &str,
    placement: Placement,
) -> Result<SchemaNode, SchemaError> {
    if placement == Placement::Definition {
        return Err(SchemaError::at(path, SchemaProblem::NestedReference));
    }
    only_keywords(schema, &["$ref"], path)?;
    let text = reference.as_str().unwrap_or_default();
    text.strip_prefix(DEFINITION_PREFIX)
        .filter(|name| !name.is_empty() && !name.contains('/'))
        .map(|name| SchemaNode::Reference(name.to_owned()))
        .ok_or_else(|| SchemaError::at(path, SchemaProblem::ExternalReference(text.to_owned())))
}

fn parse_enum(values: &Value, path: &str) -> Result<SchemaNode, SchemaError> {
    let values = values
        .as_array()
        .ok_or_else(|| SchemaError::at(path, SchemaProblem::EmptyEnum))?;
    if values.is_empty() {
        return Err(SchemaError::at(path, SchemaProblem::EmptyEnum));
    }
    if values.iter().any(|value| value.is_object() || value.is_array()) {
        return Err(SchemaError::at(path, SchemaProblem::UnsupportedEnumValue));
    }
    Ok(SchemaNode::Enum(values.clone()))
}

fn parse_object(schema: &Map<String, Value>, path: &str, placement: Placement) -> Result<SchemaNode, SchemaError> {
    only_keywords(
        schema,
        &["type", "properties", "required", "additionalProperties"],
        path,
    )?;
    if schema.get("additionalProperties") != Some(&Value::Bool(false)) {
        return Err(SchemaError::at(path, SchemaProblem::AdditionalPropertiesAllowed));
    }
    let empty = Map::new();
    let declared = match schema.get("properties") {
        None => &empty,
        Some(Value::Object(declared)) => declared,
        Some(_) => {
            return Err(SchemaError::at(
                &format!("{path}/properties"),
                SchemaProblem::NotAnObject,
            ));
        }
    };
    let properties = declared
        .iter()
        .map(|(name, node)| {
            SchemaNode::parse(node, &format!("{path}/properties/{name}"), placement).map(|node| (name.clone(), node))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let required: Vec<String> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|names| names.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default();
    if let Some(undeclared) = required.iter().find(|name| !declared.contains_key(*name)) {
        return Err(SchemaError::at(
            path,
            SchemaProblem::UndeclaredRequired(undeclared.clone()),
        ));
    }
    Ok(SchemaNode::Object { properties, required })
}

fn parse_string(schema: &Map<String, Value>, path: &str) -> Result<SchemaNode, SchemaError> {
    only_keywords(schema, &["type", "minLength", "maxLength", "pattern"], path)?;
    if let Some(pattern) = schema.get("pattern") {
        if schema.contains_key("minLength") || schema.contains_key("maxLength") {
            return Err(SchemaError::at(path, SchemaProblem::PatternWithLength));
        }
        let source = pattern.as_str().unwrap_or_default();
        if !(source.starts_with('^') && source.ends_with('$')) {
            return Err(SchemaError::at(path, SchemaProblem::UnanchoredPattern));
        }
        return Pattern::parse(source)
            .map(SchemaNode::PatternString)
            .map_err(|reason| SchemaError::at(path, SchemaProblem::UnsupportedPattern(reason)));
    }
    let Some(maximum) = schema.get("maxLength").and_then(Value::as_u64) else {
        return Err(SchemaError::at(path, SchemaProblem::UnboundedString));
    };
    if maximum >= STRING_LENGTH_LIMIT {
        return Err(SchemaError::at(path, SchemaProblem::StringTooLong(maximum)));
    }
    let minimum = schema.get("minLength").and_then(Value::as_u64).unwrap_or(0);
    if minimum > maximum {
        return Err(SchemaError::at(path, SchemaProblem::InvertedBounds));
    }
    Ok(SchemaNode::BoundedString {
        minimum_length: u32::try_from(minimum).expect("a length below the limit fits in u32"),
        maximum_length: u32::try_from(maximum).expect("a length below the limit fits in u32"),
    })
}

fn parse_array(schema: &Map<String, Value>, path: &str, placement: Placement) -> Result<SchemaNode, SchemaError> {
    only_keywords(schema, &["type", "items", "minItems", "maxItems"], path)?;
    let items = schema
        .get("items")
        .ok_or_else(|| SchemaError::at(path, SchemaProblem::MissingType))?;
    let Some(maximum) = schema.get("maxItems").and_then(Value::as_u64) else {
        return Err(SchemaError::at(path, SchemaProblem::UnboundedArray));
    };
    let minimum = schema.get("minItems").and_then(Value::as_u64).unwrap_or(0);
    if minimum > maximum {
        return Err(SchemaError::at(path, SchemaProblem::InvertedBounds));
    }
    let count = |value: u64| u32::try_from(value).map_err(|_| SchemaError::at(path, SchemaProblem::UnboundedArray));
    Ok(SchemaNode::Array {
        items: Box::new(SchemaNode::parse(items, &format!("{path}/items"), placement)?),
        minimum_items: count(minimum)?,
        maximum_items: count(maximum)?,
    })
}

fn bounds(schema: &Map<String, Value>, path: &str) -> Result<Option<(i64, i64)>, SchemaError> {
    let bound = |keyword: &str| {
        schema
            .get(keyword)
            .map(|value| {
                value
                    .as_i64()
                    .ok_or_else(|| SchemaError::at(path, SchemaProblem::FractionalBound))
            })
            .transpose()
    };
    match (bound("minimum")?, bound("maximum")?) {
        (None, None) => Ok(None),
        (Some(minimum), Some(maximum)) if minimum <= maximum => Ok(Some((minimum, maximum))),
        (Some(_), Some(_)) => Err(SchemaError::at(path, SchemaProblem::InvertedBounds)),
        _ => Err(SchemaError::at(path, SchemaProblem::HalfOpenRange)),
    }
}

fn only_keywords(schema: &Map<String, Value>, allowed: &[&str], path: &str) -> Result<(), SchemaError> {
    match schema
        .keys()
        .find(|key| !allowed.contains(&key.as_str()) && !ANNOTATIONS.contains(&key.as_str()))
    {
        Some(keyword) => Err(SchemaError::at(
            path,
            SchemaProblem::UnsupportedKeyword(keyword.clone()),
        )),
        None => Ok(()),
    }
}
