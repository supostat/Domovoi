use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

const TRASH: &str = ".Trash";
const TCC_DATABASE_DIRECTORY: &str = "/Library/Application Support/com.apple.TCC";
const SAFARI_DATA: &str = "Library/Safari";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FullDiskAccess {
    Granted,
    Denied,
    Unknown,
}

impl FullDiskAccess {
    pub fn probe() -> Self {
        let home = env::home_dir();
        let probes = [
            home.as_ref().map(|home| home.join(TRASH)),
            Some(PathBuf::from(TCC_DATABASE_DIRECTORY)),
            home.as_ref().map(|home| home.join(SAFARI_DATA)),
        ];
        Self::from_outcomes(probes.iter().flatten().map(|probe| fs::read_dir(probe).map(drop)))
    }

    fn from_outcomes(outcomes: impl IntoIterator<Item = io::Result<()>>) -> Self {
        for outcome in outcomes {
            match outcome {
                Ok(()) => return Self::Granted,
                Err(error) if error.raw_os_error() == Some(libc::EPERM) => return Self::Denied,
                Err(_) => {}
            }
        }
        Self::Unknown
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::FullDiskAccess;

    fn errno(code: i32) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(code))
    }

    #[test]
    fn first_decisive_probe_wins() {
        assert_eq!(
            FullDiskAccess::from_outcomes([errno(libc::ENOENT), Ok(())]),
            FullDiskAccess::Granted
        );
        assert_eq!(
            FullDiskAccess::from_outcomes([errno(libc::EACCES), errno(libc::EPERM)]),
            FullDiskAccess::Denied
        );
        assert_eq!(
            FullDiskAccess::from_outcomes([errno(libc::ENOENT)]),
            FullDiskAccess::Unknown
        );
    }
}
