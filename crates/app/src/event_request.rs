use crate::pending_delta::PendingDelta;

#[derive(Clone, Debug)]
pub enum EventRequest {
    Delta(PendingDelta),
    RescanRequired,
}
