use std::io;
use std::path::Path;
use std::process::Command;

const GIT: &str = "/usr/bin/git";

pub(crate) fn git(root: &Path, arguments: &[&str]) -> io::Result<String> {
    let output = Command::new(GIT).current_dir(root).args(arguments).output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "git {} failed: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "git printed non-UTF-8 output"))
}
