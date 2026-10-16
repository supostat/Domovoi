use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use std::ptr;

use crate::c_string::c_string;
use crate::sys::{PROC_ALL_PIDS, proc_listpids, proc_listpidspath, proc_pidpath};

const APPLICATION_BUNDLE_EXTENSION: &str = "app";
const SPARE_PROCESS_SLOTS: usize = 64;

pub fn process_path(pid: libc::pid_t) -> io::Result<PathBuf> {
    let mut path = vec![0_u8; usize::try_from(libc::PROC_PIDPATHINFO_MAXSIZE).expect("constant is positive")];
    let length = unsafe {
        proc_pidpath(
            pid,
            path.as_mut_ptr().cast(),
            u32::try_from(path.len()).expect("path buffer fits in u32"),
        )
    };
    if length <= 0 {
        return Err(io::Error::last_os_error());
    }
    path.truncate(usize::try_from(length).expect("length is positive"));
    Ok(PathBuf::from(OsStr::from_bytes(&path)))
}

pub fn running_bundle_roots() -> io::Result<BTreeSet<PathBuf>> {
    let mut roots = BTreeSet::new();
    for pid in all_pids()? {
        match process_path(pid) {
            Ok(path) => roots.extend(outermost_application_bundle(&path)),
            Err(error) if matches!(error.raw_os_error(), Some(libc::ESRCH | libc::ENOENT)) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(roots)
}

pub fn pids_with_open(path: &Path) -> io::Result<Vec<libc::pid_t>> {
    let path = c_string(path.as_os_str())?;
    let mut pids = pid_buffer()?;
    let bytes = unsafe {
        proc_listpidspath(
            PROC_ALL_PIDS,
            0,
            path.as_ptr(),
            0,
            pids.as_mut_ptr().cast(),
            buffer_bytes(&pids),
        )
    };
    collect_pids(pids, bytes)
}

fn all_pids() -> io::Result<Vec<libc::pid_t>> {
    let mut pids = pid_buffer()?;
    let bytes = unsafe { proc_listpids(PROC_ALL_PIDS, 0, pids.as_mut_ptr().cast(), buffer_bytes(&pids)) };
    collect_pids(pids, bytes)
}

fn pid_buffer() -> io::Result<Vec<libc::pid_t>> {
    let bytes = unsafe { proc_listpids(PROC_ALL_PIDS, 0, ptr::null_mut(), 0) };
    if bytes <= 0 {
        return Err(io::Error::last_os_error());
    }
    let slots = usize::try_from(bytes).expect("byte count is positive") / size_of::<libc::pid_t>();
    Ok(vec![0; slots + SPARE_PROCESS_SLOTS])
}

fn buffer_bytes(pids: &[libc::pid_t]) -> libc::c_int {
    libc::c_int::try_from(size_of_val(pids)).expect("pid buffer fits in c_int")
}

fn collect_pids(mut pids: Vec<libc::pid_t>, bytes: libc::c_int) -> io::Result<Vec<libc::pid_t>> {
    if bytes < 0 {
        return Err(io::Error::last_os_error());
    }
    pids.truncate(usize::try_from(bytes).expect("byte count is non-negative") / size_of::<libc::pid_t>());
    pids.retain(|pid| *pid > 0);
    Ok(pids)
}

fn outermost_application_bundle(executable: &Path) -> Option<PathBuf> {
    let mut root = PathBuf::new();
    for component in executable.components() {
        root.push(component);
        if let Component::Normal(name) = component
            && Path::new(name).extension() == Some(OsStr::new(APPLICATION_BUNDLE_EXTENSION))
        {
            return Some(root);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::outermost_application_bundle;

    #[test]
    fn finds_the_outermost_application_bundle() {
        let helper =
            Path::new("/Applications/Slack.app/Contents/Frameworks/Slack Helper.app/Contents/MacOS/Slack Helper");
        assert_eq!(
            outermost_application_bundle(helper),
            Some(PathBuf::from("/Applications/Slack.app"))
        );
        assert_eq!(outermost_application_bundle(Path::new("/usr/libexec/syspolicyd")), None);
    }
}
