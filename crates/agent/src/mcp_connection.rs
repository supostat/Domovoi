use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use crate::mcp_session::McpSession;

const LONGEST_MESSAGE: usize = 1024 * 1024;

#[derive(Debug)]
pub(crate) struct McpConnection {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
    pending: Vec<u8>,
}

impl McpConnection {
    pub(crate) fn new(stream: UnixStream, wait: Duration) -> io::Result<Self> {
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(wait))?;
        let writer = stream.try_clone()?;
        Ok(Self {
            reader: BufReader::new(stream),
            writer,
            pending: Vec::new(),
        })
    }

    pub(crate) fn serve(&mut self, session: &mut McpSession<'_>) -> io::Result<bool> {
        let room = (LONGEST_MESSAGE + 1 - self.pending.len()) as u64;
        match (&mut self.reader).take(room).read_until(b'\n', &mut self.pending) {
            Ok(0) => Ok(false),
            Ok(_) if self.pending.ends_with(b"\n") => {
                let message = String::from_utf8_lossy(&self.pending).trim().to_owned();
                self.pending.clear();
                if !message.is_empty()
                    && let Some(reply) = session.respond(&message)
                {
                    self.writer.write_all(reply.as_bytes())?;
                    self.writer.write_all(b"\n")?;
                    self.writer.flush()?;
                }
                Ok(true)
            }
            Ok(_) if self.pending.len() > LONGEST_MESSAGE => {
                Err(io::Error::new(io::ErrorKind::InvalidData, "an MCP message is too long"))
            }
            Ok(_) => Ok(true),
            Err(error) if is_wait_over(&error) => Ok(true),
            Err(error) => Err(error),
        }
    }
}

fn is_wait_over(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted
    )
}
