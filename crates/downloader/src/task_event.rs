use crate::transfer_event::TransferEvent;

#[derive(Debug)]
pub(crate) struct TaskEvent {
    pub(crate) task: usize,
    pub(crate) event: TransferEvent,
}
