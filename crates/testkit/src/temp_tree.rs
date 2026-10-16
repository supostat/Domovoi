use std::ffi::{CString, OsString};
use std::fs::{self, File};
use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::directory::Directory;

const TEMP_BASE: &str = "/private/tmp";
const TEMPLATE_NAME: &str = "domovoi-tree-XXXXXX";
const RESTORED_MODE: u32 = 0o700;

#[derive(Debug)]
pub struct TempTree {
    path: PathBuf,
    root: OwnedFd,
    restricted: Mutex<Vec<PathBuf>>,
}

impl TempTree {
    pub fn new() -> io::Result<Self> {
        let template = Path::new(TEMP_BASE).join(TEMPLATE_NAME);
        let mut template = CString::new(template.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "temp directory path contains NUL"))?
            .into_bytes_with_nul();
        let created = unsafe { libc::mkdtemp(template.as_mut_ptr().cast()) };
        if created.is_null() {
            return Err(io::Error::last_os_error());
        }
        template.pop();
        let path = PathBuf::from(OsString::from_vec(template));
        let root = OwnedFd::from(File::open(&path)?);
        Ok(Self {
            path,
            root,
            restricted: Mutex::new(Vec::new()),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn root(&self) -> io::Result<Directory<'_>> {
        Ok(Directory::new(self, self.root.try_clone()?, self.path.clone()))
    }

    pub(crate) fn remember_restricted(&self, path: PathBuf) {
        self.restricted
            .lock()
            .expect("restricted path list is not poisoned")
            .push(path);
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let restricted = self.restricted.get_mut().expect("restricted path list is not poisoned");
        for path in restricted.iter() {
            if let Err(error) = fs::set_permissions(path, fs::Permissions::from_mode(RESTORED_MODE)) {
                eprintln!("permissions of {} were not restored: {error}", path.display());
            }
        }
        if let Err(error) = fs::remove_dir_all(&self.path) {
            eprintln!("temp tree {} was not removed: {error}", self.path.display());
        }
    }
}
