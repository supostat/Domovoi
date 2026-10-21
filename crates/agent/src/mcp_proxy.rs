use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::thread;

pub const MCP_PROXY_FLAG: &str = "--mcp-proxy";
const CHUNK: usize = 8192;

pub fn relay_mcp_stdio(socket: &Path) -> io::Result<()> {
    let stream = UnixStream::connect(socket)?;
    let inbound = stream.try_clone()?;
    thread::spawn(move || forward_stdin(inbound));
    let mut outbound = stream;
    let mut stdout = io::stdout().lock();
    let mut chunk = [0; CHUNK];
    loop {
        let read = outbound.read(&mut chunk)?;
        if read == 0 {
            return Ok(());
        }
        stdout.write_all(&chunk[..read])?;
        stdout.flush()?;
    }
}

fn forward_stdin(mut socket: UnixStream) -> io::Result<()> {
    io::copy(&mut io::stdin().lock(), &mut socket)?;
    socket.shutdown(Shutdown::Write)
}
