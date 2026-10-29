use domovoi_index::{RecoveryHandle, RecoveryOutcome};

pub trait RecoveryRequest {
    fn outcome(&mut self) -> Option<RecoveryOutcome>;
}

impl RecoveryRequest for RecoveryHandle {
    fn outcome(&mut self) -> Option<RecoveryOutcome> {
        self.poll().cloned()
    }
}
