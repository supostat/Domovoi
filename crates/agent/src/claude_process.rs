use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::thread::{self, JoinHandle};

#[derive(Debug)]
pub(crate) struct ClaudeProcess {
    child: Child,
    stdout: Option<JoinHandle<io::Result<Vec<u8>>>>,
    stderr: Option<JoinHandle<io::Result<Vec<u8>>>>,
}

impl ClaudeProcess {
    pub(crate) fn spawn(program: &Path, workspace: &Path, arguments: &[String], prompt: &str) -> io::Result<Self> {
        let mut child = Command::new(program)
            .args(arguments)
            .current_dir(workspace)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let stdout = child.stdout.take().expect("stdout is piped");
        let stderr = child.stderr.take().expect("stderr is piped");
        let mut process = Self {
            stdout: Some(thread::spawn(move || read_all(stdout))),
            stderr: Some(thread::spawn(move || read_all(stderr))),
            child,
        };
        let mut stdin = process.child.stdin.take().expect("stdin is piped");
        match stdin.write_all(prompt.as_bytes()) {
            Err(error) if error.kind() != io::ErrorKind::BrokenPipe => Err(error),
            _ => Ok(process),
        }
    }

    pub(crate) fn finished(&mut self) -> io::Result<Option<Output>> {
        let Some(status) = self.child.try_wait()? else {
            return Ok(None);
        };
        Ok(Some(Output {
            status,
            stdout: joined(self.stdout.take())?,
            stderr: joined(self.stderr.take())?,
        }))
    }

    pub(crate) fn stop(&mut self) -> io::Result<()> {
        if self.child.try_wait()?.is_none() {
            self.child.kill()?;
            self.child.wait()?;
        }
        Ok(())
    }
}

impl Drop for ClaudeProcess {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            eprintln!("domovoi: claude did not stop: {error}");
        }
    }
}

fn read_all(mut stream: impl Read) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn joined(reader: Option<JoinHandle<io::Result<Vec<u8>>>>) -> io::Result<Vec<u8>> {
    reader
        .expect("the output is read once")
        .join()
        .map_err(|_| io::Error::other("an output reader panicked"))?
}
