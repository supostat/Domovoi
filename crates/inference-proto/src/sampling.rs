#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sampling {
    pub temperature: f64,
    pub top_k: u32,
    pub top_p: f64,
    pub seed: u32,
}
