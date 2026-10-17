#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontFamily {
    Interface,
    Monospace,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    pub family: FontFamily,
    pub weight: f32,
    pub size: f32,
}

impl TextStyle {
    #[must_use]
    pub fn sized(self, size: f32) -> Self {
        Self { size, ..self }
    }
}
