use std::fs::File;
use std::io;
use std::os::unix::fs::FileExt;

use domovoi_scanner::CancelToken;

const READ_CHUNK_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentComparison {
    Same,
    Different,
    Stopped,
}

impl ContentComparison {
    pub fn of(
        first: &File,
        second: &File,
        size: u64,
        cancel: &CancelToken,
        compared: &dyn Fn(u64),
    ) -> io::Result<Self> {
        let chunk = usize::try_from(size.min(READ_CHUNK_BYTES)).expect("a read chunk fits in usize");
        let mut first_buffer = vec![0_u8; chunk];
        let mut second_buffer = vec![0_u8; chunk];
        let mut offset = 0;
        while offset < size {
            if cancel.is_cancelled() {
                return Ok(Self::Stopped);
            }
            let length = usize::try_from((size - offset).min(READ_CHUNK_BYTES)).expect("a read chunk fits in usize");
            first.read_exact_at(&mut first_buffer[..length], offset)?;
            second.read_exact_at(&mut second_buffer[..length], offset)?;
            if first_buffer[..length] != second_buffer[..length] {
                return Ok(Self::Different);
            }
            offset += length as u64;
            compared(offset);
        }
        Ok(Self::Same)
    }
}
