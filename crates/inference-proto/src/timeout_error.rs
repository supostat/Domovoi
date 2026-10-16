use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeoutError {
    pub variable: &'static str,
    pub value: String,
}

impl fmt::Display for TimeoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} = {:?} is not a positive number of milliseconds",
            self.variable, self.value
        )
    }
}

impl std::error::Error for TimeoutError {}
