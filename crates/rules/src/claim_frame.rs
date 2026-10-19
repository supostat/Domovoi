pub(crate) enum ClaimFrame {
    Claimed(usize),
    Open(Option<usize>),
}
