use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use domovoi_agent::ClaudeCode;

use crate::session_paths::SessionPaths;

const PROGRAM: &str = "claude";
const HOME_INSTALL: &str = ".local/bin";
const SYSTEM_INSTALLS: [&str; 2] = ["/opt/homebrew/bin", "/usr/local/bin"];
const EXECUTABLE_BITS: u32 = 0o111;
const MODEL: &str = "sonnet";
const MAX_TURNS: u32 = 8;
const WORKSPACE: &str = "claude";

pub fn claude_directories(home: &Path) -> Vec<PathBuf> {
    let mut directories = vec![home.join(HOME_INSTALL)];
    directories.extend(SYSTEM_INSTALLS.map(PathBuf::from));
    directories
}

pub fn find_claude(directories: &[PathBuf]) -> Option<PathBuf> {
    directories
        .iter()
        .map(|directory| directory.join(PROGRAM))
        .find(|candidate| is_executable(candidate))
}

pub(crate) fn installed_claude_code(paths: &SessionPaths) -> Option<ClaudeCode> {
    Some(ClaudeCode {
        program: find_claude(&claude_directories(&paths.home))?,
        proxy: env::current_exe().ok()?,
        workspace: paths.models.with_file_name(WORKSPACE),
        model: MODEL.to_owned(),
        max_turns: MAX_TURNS,
        keep_sessions: true,
    })
}

fn is_executable(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & EXECUTABLE_BITS != 0)
}
