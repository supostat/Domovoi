use std::fs;
use std::io;
use std::os::unix::fs::symlink;
use std::path::Path;

pub(crate) fn copy_tree(source: &Path, target: &Path, skipped_names: &[&str]) -> io::Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name();
        if skipped_names.iter().any(|skipped| name == *skipped) {
            continue;
        }
        let from = entry.path();
        let to = target.join(&name);
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            let destination = fs::read_link(&from)?;
            match fs::read_link(&to) {
                Ok(existing) if existing == destination => {}
                Ok(existing) => {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        format!(
                            "{} points to {} instead of {}",
                            to.display(),
                            existing.display(),
                            destination.display()
                        ),
                    ));
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => symlink(&destination, &to)?,
                Err(error) => return Err(error),
            }
        } else if kind.is_dir() {
            copy_tree(&from, &to, skipped_names)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}
