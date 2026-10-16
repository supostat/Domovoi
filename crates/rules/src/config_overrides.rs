use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const PNPM_RC: &str = ".config/pnpm/rc";
const CARGO_CONFIG: &str = ".cargo/config.toml";
const PNPM_STORE_KEY: &str = "store-dir";
const CARGO_TARGET_KEY: &str = "target-dir";
const CARGO_BUILD_TABLE: &str = "[build]";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConfigOverrides {
    pub pnpm_store: Option<PathBuf>,
    pub cargo_target: Option<PathBuf>,
}

impl ConfigOverrides {
    pub fn read(home: &Path) -> io::Result<Self> {
        let pnpm_store = read_optional(&home.join(PNPM_RC))?
            .and_then(|rc| value_of(&rc, PNPM_STORE_KEY, None))
            .map(|value| expand_home(&value, home));
        let cargo_target = read_optional(&home.join(CARGO_CONFIG))?
            .and_then(|config| value_of(&config, CARGO_TARGET_KEY, Some(CARGO_BUILD_TABLE)))
            .map(|value| expand_home(&value, home));
        Ok(Self {
            pnpm_store,
            cargo_target,
        })
    }
}

fn read_optional(path: &Path) -> io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn value_of(text: &str, key: &str, table: Option<&str>) -> Option<String> {
    let mut in_table = table.is_none();
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_table = table == Some(line);
            continue;
        }
        if !in_table {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        if name.trim() == key {
            return Some(value.trim().trim_matches('"').to_owned());
        }
    }
    None
}

fn expand_home(value: &str, home: &Path) -> PathBuf {
    match value.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None => PathBuf::from(value),
    }
}

#[cfg(test)]
mod tests {
    use super::value_of;

    #[test]
    fn reads_keys_inside_their_table_only() {
        let config = "[net]\ntarget-dir = \"/nope\"\n[build]\ntarget-dir = \"/Volumes/Fast/target\"\n";
        assert_eq!(
            value_of(config, "target-dir", Some("[build]")).as_deref(),
            Some("/Volumes/Fast/target")
        );
        assert_eq!(
            value_of("store-dir=~/pnpm\n", "store-dir", None).as_deref(),
            Some("~/pnpm")
        );
    }
}
