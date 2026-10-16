#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Getattrlist,
    MacOsEstimate,
    Scan,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Measured<T> {
    pub value: T,
    pub source: Source,
}

impl<T> Measured<T> {
    pub(crate) fn new(value: T, source: Source) -> Self {
        Self { value, source }
    }
}
