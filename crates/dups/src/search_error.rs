use std::io;

#[derive(Debug)]
pub enum SearchError {
    Cancelled,
    Io(io::Error),
}
