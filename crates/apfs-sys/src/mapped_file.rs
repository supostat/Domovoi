use std::fs::File;
use std::io;
use std::ops::Deref;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::ptr::{self, NonNull};
use std::slice;

#[derive(Debug)]
pub struct MappedFile {
    start: NonNull<u8>,
    length: usize,
}

unsafe impl Send for MappedFile {}
unsafe impl Sync for MappedFile {}

pub fn mmap_readonly(path: &Path) -> io::Result<MappedFile> {
    let file = File::open(path)?;
    let length = usize::try_from(file.metadata()?.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "file is larger than the address space"))?;
    if length == 0 {
        return Ok(MappedFile {
            start: NonNull::dangling(),
            length,
        });
    }
    let mapping = unsafe {
        libc::mmap(
            ptr::null_mut(),
            length,
            libc::PROT_READ,
            libc::MAP_PRIVATE,
            file.as_raw_fd(),
            0,
        )
    };
    if mapping == libc::MAP_FAILED {
        return Err(io::Error::last_os_error());
    }
    Ok(MappedFile {
        start: NonNull::new(mapping.cast()).expect("a successful mapping is not null"),
        length,
    })
}

impl Deref for MappedFile {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.start.as_ptr(), self.length) }
    }
}

impl Drop for MappedFile {
    fn drop(&mut self) {
        if self.length > 0 {
            unsafe { libc::munmap(self.start.as_ptr().cast(), self.length) };
        }
    }
}
