use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

use domovoi_apfs_sys::{data_volume, firmlinks};

const SERVICE_PREFIX: &str = "/System/Volumes/Data";
const SYSTEM_ROOT: &str = "/";

#[derive(Clone, Debug, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct FirmlinkPair {
    pub path: String,
    pub data_relative: String,
}

#[derive(Clone, Debug, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum IndexRoot {
    DataVolume { firmlinks: Vec<FirmlinkPair> },
    Plain { path: String },
}

impl IndexRoot {
    pub fn for_path(root: &Path) -> io::Result<Self> {
        let root = fs::canonicalize(root)?;
        if root == fs::canonicalize(data_volume()?)? {
            let firmlinks = firmlinks()?
                .into_iter()
                .map(|firmlink| FirmlinkPair {
                    path: firmlink.path.to_string_lossy().into_owned(),
                    data_relative: firmlink.data_relative.to_string_lossy().into_owned(),
                })
                .collect();
            Ok(Self::DataVolume { firmlinks })
        } else {
            Ok(Self::Plain {
                path: root.to_string_lossy().into_owned(),
            })
        }
    }

    pub(crate) fn scan_path(&self) -> PathBuf {
        match self {
            Self::DataVolume { .. } => PathBuf::from(SERVICE_PREFIX),
            Self::Plain { path } => PathBuf::from(path),
        }
    }

    pub(crate) fn root_components(&self) -> Vec<String> {
        match self {
            Self::DataVolume { .. } => Vec::new(),
            Self::Plain { path } => normal_components(Path::new(path)),
        }
    }

    pub(crate) fn is_service_entry(&self, top_level_name: &[u8]) -> bool {
        match self {
            Self::DataVolume { firmlinks } => !firmlinks.iter().any(|firmlink| {
                Path::new(&firmlink.data_relative)
                    .components()
                    .next()
                    .is_some_and(|first| first.as_os_str().as_bytes() == top_level_name)
            }),
            Self::Plain { .. } => false,
        }
    }

    pub(crate) fn canonical(&self, relative: &Path) -> PathBuf {
        match self {
            Self::Plain { path } => Path::new(path).join(relative),
            Self::DataVolume { firmlinks } => {
                if relative.as_os_str().is_empty() {
                    return PathBuf::from(SYSTEM_ROOT);
                }
                firmlinks
                    .iter()
                    .filter_map(|firmlink| {
                        relative
                            .strip_prefix(&firmlink.data_relative)
                            .ok()
                            .map(|rest| (firmlink.data_relative.len(), Path::new(&firmlink.path).join(rest)))
                    })
                    .max_by_key(|(prefix_length, _)| *prefix_length)
                    .map_or_else(|| Path::new(SERVICE_PREFIX).join(relative), |(_, canonical)| canonical)
            }
        }
    }

    pub(crate) fn relative(&self, canonical: &Path) -> Option<PathBuf> {
        match self {
            Self::Plain { path } => canonical.strip_prefix(path).ok().map(Path::to_path_buf),
            Self::DataVolume { firmlinks } => {
                if let Ok(service) = canonical.strip_prefix(SERVICE_PREFIX) {
                    return Some(service.to_path_buf());
                }
                firmlinks
                    .iter()
                    .find_map(|firmlink| {
                        canonical
                            .strip_prefix(&firmlink.path)
                            .ok()
                            .map(|rest| Path::new(&firmlink.data_relative).join(rest))
                    })
                    .or_else(|| (canonical == Path::new(SYSTEM_ROOT)).then(PathBuf::new))
            }
        }
    }
}

fn normal_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{FirmlinkPair, IndexRoot};

    fn data_volume() -> IndexRoot {
        let pair = |path: &str, data_relative: &str| FirmlinkPair {
            path: path.to_owned(),
            data_relative: data_relative.to_owned(),
        };
        IndexRoot::DataVolume {
            firmlinks: vec![
                pair("/Users", "Users"),
                pair("/System/Library/Caches", "System/Library/Caches"),
            ],
        }
    }

    #[test]
    fn maps_between_relative_and_canonical_paths() {
        let root = data_volume();
        for (relative, canonical) in [
            ("Users/igor/Library", "/Users/igor/Library"),
            ("System/Library/Caches/x", "/System/Library/Caches/x"),
            (".Spotlight-V100", "/System/Volumes/Data/.Spotlight-V100"),
            ("", "/"),
        ] {
            assert_eq!(root.canonical(Path::new(relative)), Path::new(canonical));
            assert_eq!(
                root.relative(Path::new(canonical)).as_deref(),
                Some(Path::new(relative))
            );
        }
        assert!(root.is_service_entry(b".Spotlight-V100"));
        assert!(!root.is_service_entry(b"System"));
    }
}
