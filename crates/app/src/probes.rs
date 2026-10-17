use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Probes {
    pub window_background: [f64; 2],
    pub sidebar: Option<[f64; 2]>,
    pub accent_button: Option<[f64; 4]>,
}
