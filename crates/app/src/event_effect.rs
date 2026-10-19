#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventEffect {
    Applied,
    Ignored,
    Heavy,
    RescanRequired,
}
