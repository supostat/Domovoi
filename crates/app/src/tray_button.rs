pub(crate) const ESCAPE: &str = "\u{1b}";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TrayButton {
    Primary,
    Secondary,
    Cancel,
}
