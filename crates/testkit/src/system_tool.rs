use std::ffi::OsStr;
use std::io;
use std::process::Command;

pub(crate) fn run<I, S>(program: &str, arguments: I) -> io::Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new(program).args(arguments).output()?;
    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{program} printed non-UTF-8 output"),
            )
        })
    } else {
        Err(io::Error::other(format!(
            "{program} failed with {}: {} {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim(),
            String::from_utf8_lossy(&output.stdout).trim()
        )))
    }
}
