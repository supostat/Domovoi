use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use domovoi_apfs_sys::{Firmlink, data_volume, firmlinks};

const SYSTEM_ROOT: &str = "/";

#[derive(Clone, Debug)]
pub(crate) enum CanonicalPaths {
    DataVolume { root: PathBuf, firmlinks: Vec<Firmlink> },
    Plain { root: PathBuf },
}

impl CanonicalPaths {
    pub(crate) fn for_root(root: &Path) -> io::Result<Self> {
        let root = fs::canonicalize(root)?;
        if root == fs::canonicalize(data_volume()?)? {
            let mut firmlinks = firmlinks()?;
            firmlinks.sort_by_key(|firmlink| std::cmp::Reverse(firmlink.data_relative.components().count()));
            Ok(Self::DataVolume { root, firmlinks })
        } else {
            Ok(Self::Plain { root })
        }
    }

    pub(crate) fn canonical(&self, relative: &Path) -> PathBuf {
        match self {
            Self::Plain { root } => root.join(relative),
            Self::DataVolume { root, firmlinks } => {
                if relative.as_os_str().is_empty() {
                    return PathBuf::from(SYSTEM_ROOT);
                }
                firmlinks
                    .iter()
                    .find_map(|firmlink| {
                        relative
                            .strip_prefix(&firmlink.data_relative)
                            .ok()
                            .map(|rest| firmlink.path.join(rest))
                    })
                    .unwrap_or_else(|| root.join(relative))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use domovoi_apfs_sys::Firmlink;

    use super::CanonicalPaths;

    fn data_volume() -> CanonicalPaths {
        let firmlink = |path: &str, data_relative: &str| Firmlink {
            path: PathBuf::from(path),
            data_relative: PathBuf::from(data_relative),
        };
        CanonicalPaths::DataVolume {
            root: PathBuf::from("/System/Volumes/Data"),
            firmlinks: vec![
                firmlink("/System/Library/Caches", "System/Library/Caches"),
                firmlink("/Users", "Users"),
                firmlink("/private", "private"),
            ],
        }
    }

    #[test]
    fn maps_data_volume_paths_onto_their_firmlink_form() {
        let paths = data_volume();
        assert_eq!(
            paths.canonical(Path::new("Users/igor/Library")),
            Path::new("/Users/igor/Library")
        );
        assert_eq!(
            paths.canonical(Path::new("System/Library/Caches/x")),
            Path::new("/System/Library/Caches/x")
        );
        assert_eq!(paths.canonical(Path::new("")), Path::new("/"));
    }

    #[test]
    fn keeps_data_volume_paths_outside_the_map_as_service_paths() {
        assert_eq!(
            data_volume().canonical(Path::new(".Spotlight-V100")),
            Path::new("/System/Volumes/Data/.Spotlight-V100")
        );
    }
}
