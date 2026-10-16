use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GbnfGrammar {
    text: String,
}

impl GbnfGrammar {
    pub(crate) fn new(text: String) -> Self {
        Self { text }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl fmt::Display for GbnfGrammar {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.text)
    }
}
