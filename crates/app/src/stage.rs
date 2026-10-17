use crate::access_state::AccessState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    Access(AccessState),
    Scanning,
    Main,
}
