use crate::response_head::ResponseHead;

#[derive(Debug)]
pub(crate) enum TransferEvent {
    Response(ResponseHead),
    Data(Vec<u8>),
    Finished(Option<String>),
}
