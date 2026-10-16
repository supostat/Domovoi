use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaViolation {
    pub path: String,
    pub reason: String,
}

impl SchemaViolation {
    pub(crate) fn at(path: &str, reason: impl Into<String>) -> Self {
        Self {
            path: path.to_owned(),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for SchemaViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.reason)
    }
}

impl std::error::Error for SchemaViolation {}
