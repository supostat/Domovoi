use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsFd;
use std::os::macos::fs::MetadataExt;
use std::os::unix::fs::{FileExt, OpenOptionsExt};
use std::path::Path;

use domovoi_apfs_sys::set_nocache;
use domovoi_apfs_sys::sys::SF_DATALESS;

use crate::digest_kind::DigestKind;
use crate::skip_reason::SkipReason;

const EDGE_BYTES: u64 = 64 * 1024;
const READ_CHUNK_BYTES: u64 = 4 * 1024 * 1024;
const PARALLEL_HASHING_BYTES: usize = 128 * 1024;

pub(crate) fn edges_cover_whole_file(size: u64) -> bool {
    size <= 2 * EDGE_BYTES
}

pub(crate) fn digest(path: &Path, size: u64, kind: DigestKind) -> Result<blake3::Hash, SkipReason> {
    let file = open_unchanged(path, size)?;
    let mut hasher = blake3::Hasher::new();
    let read = match kind {
        DigestKind::Edges if !edges_cover_whole_file(size) => hash_range(&file, 0, EDGE_BYTES, &mut hasher)
            .and_then(|()| hash_range(&file, size - EDGE_BYTES, EDGE_BYTES, &mut hasher)),
        DigestKind::Edges | DigestKind::Contents => hash_range(&file, 0, size, &mut hasher),
    };
    read.map_err(|error| SkipReason::of(&error))?;
    Ok(hasher.finalize())
}

pub(crate) fn open_without_cache(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    set_nocache(file.as_fd())?;
    Ok(file)
}

pub(crate) fn same_contents(first: &File, second: &File, size: u64) -> io::Result<bool> {
    let chunk = usize::try_from(size.min(READ_CHUNK_BYTES)).expect("a read chunk fits in usize");
    let mut first_buffer = vec![0_u8; chunk];
    let mut second_buffer = vec![0_u8; chunk];
    let mut offset = 0;
    while offset < size {
        let length = usize::try_from((size - offset).min(READ_CHUNK_BYTES)).expect("a read chunk fits in usize");
        first.read_exact_at(&mut first_buffer[..length], offset)?;
        second.read_exact_at(&mut second_buffer[..length], offset)?;
        if first_buffer[..length] != second_buffer[..length] {
            return Ok(false);
        }
        offset += length as u64;
    }
    Ok(true)
}

fn open_unchanged(path: &Path, size: u64) -> Result<File, SkipReason> {
    let file = open_without_cache(path).map_err(|error| SkipReason::of(&error))?;
    let metadata = file.metadata().map_err(|error| SkipReason::of(&error))?;
    if metadata.st_flags() & SF_DATALESS != 0 {
        return Err(SkipReason::InCloud);
    }
    if metadata.len() != size {
        return Err(SkipReason::Changed);
    }
    Ok(file)
}

fn hash_range(file: &File, start: u64, length: u64, hasher: &mut blake3::Hasher) -> io::Result<()> {
    let mut buffer = vec![0_u8; usize::try_from(length.min(READ_CHUNK_BYTES)).expect("a read chunk fits in usize")];
    let end = start + length;
    let mut offset = start;
    while offset < end {
        let chunk_length = usize::try_from((end - offset).min(READ_CHUNK_BYTES)).expect("a read chunk fits in usize");
        let chunk = &mut buffer[..chunk_length];
        file.read_exact_at(chunk, offset)?;
        if chunk_length >= PARALLEL_HASHING_BYTES {
            hasher.update_rayon(chunk);
        } else {
            hasher.update(chunk);
        }
        offset += chunk_length as u64;
    }
    Ok(())
}
