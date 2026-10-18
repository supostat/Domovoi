use std::io::{self, Read};
use std::thread;
use std::time::Duration;

pub(crate) struct ThrottledBody {
    pub(crate) content: Vec<u8>,
    pub(crate) position: usize,
    pub(crate) chunk: usize,
    pub(crate) delay: Duration,
}

impl Read for ThrottledBody {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.position >= self.content.len() {
            return Ok(0);
        }
        if self.position > 0 {
            thread::sleep(self.delay);
        }
        let length = buffer.len().min(self.chunk).min(self.content.len() - self.position);
        buffer[..length].copy_from_slice(&self.content[self.position..self.position + length]);
        self.position += length;
        Ok(length)
    }
}
