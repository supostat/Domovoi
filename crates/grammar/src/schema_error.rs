use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchemaProblem {
    NotAnObject,
    MissingType,
    UnknownType(String),
    UnsupportedKeyword(String),
    PatternProperties,
    AlternativesBesideProperties,
    NestedReference,
    ExternalReference(String),
    UnknownReference(String),
    AdditionalPropertiesAllowed,
    UndeclaredRequired(String),
    UnboundedString,
    StringTooLong(u64),
    PatternWithLength,
    UnanchoredPattern,
    UnsupportedPattern(String),
    UnboundedArray,
    InvertedBounds,
    HalfOpenRange,
    FractionalBound,
    NegativeNumberBound,
    EmptyEnum,
    UnsupportedEnumValue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaError {
    pub path: String,
    pub problem: SchemaProblem,
}

impl SchemaError {
    pub(crate) fn at(path: &str, problem: SchemaProblem) -> Self {
        Self {
            path: path.to_owned(),
            problem,
        }
    }
}

impl fmt::Display for SchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {:?}", self.path, self.problem)
    }
}

impl std::error::Error for SchemaError {}
