use std::io;
use std::path::Path;

use objc2_foundation::{NSString, NSURL, NSURLVolumeLocalizedNameKey};

pub(crate) fn volume_name(path: &Path) -> io::Result<String> {
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    let mut value = None;
    unsafe { url.getResourceValue_forKey_error(&mut value, NSURLVolumeLocalizedNameKey) }
        .map_err(|error| io::Error::other(format!("{}: {}", path.display(), error.localizedDescription())))?;
    value
        .and_then(|name| name.downcast::<NSString>().ok())
        .map(|name| name.to_string())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{} has no volume name", path.display()),
            )
        })
}
