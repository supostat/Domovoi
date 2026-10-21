use std::env;
use std::fs::{self, DirBuilder};
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const OWNER_ONLY: u32 = 0o700;
const SOCKET_NAME: &str = "mcp";

#[derive(Debug)]
pub(crate) struct PrivateSocket {
    directory: PathBuf,
    path: PathBuf,
    listener: UnixListener,
}

impl PrivateSocket {
    pub(crate) fn bind() -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.subsec_nanos());
        let directory = env::temp_dir().join(format!(
            "domovoi-{}-{}-{nanos}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        DirBuilder::new().mode(OWNER_ONLY).create(&directory)?;
        let path = directory.join(SOCKET_NAME);
        let listener = match UnixListener::bind(&path) {
            Ok(listener) => listener,
            Err(error) => {
                fs::remove_dir(&directory)?;
                return Err(error);
            }
        };
        let socket = Self {
            directory,
            path,
            listener,
        };
        socket.listener.set_nonblocking(true)?;
        Ok(socket)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn accept(&self) -> io::Result<Option<UnixStream>> {
        match self.listener.accept() {
            Ok((stream, _)) => Ok(Some(stream)),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn remove(&self) -> io::Result<()> {
        fs::remove_file(&self.path)?;
        fs::remove_dir(&self.directory)
    }
}

impl Drop for PrivateSocket {
    fn drop(&mut self) {
        if let Err(error) = self.remove() {
            eprintln!(
                "domovoi: the MCP socket directory {} stays: {error}",
                self.directory.display()
            );
        }
    }
}
