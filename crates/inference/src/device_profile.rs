#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceProfile {
    Cpu,
    Gpu,
}

impl DeviceProfile {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "cpu" => Some(Self::Cpu),
            "gpu" => Some(Self::Gpu),
            _ => None,
        }
    }
}
