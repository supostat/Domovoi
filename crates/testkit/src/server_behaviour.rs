use std::time::Duration;

#[derive(Clone, Debug)]
pub struct ServerBehaviour {
    pub content: Vec<u8>,
    pub revision: String,
    pub linked_etag: String,
    pub expired_links: u32,
    pub ignore_ranges: bool,
    pub chunk: usize,
    pub chunk_delay: Duration,
}
