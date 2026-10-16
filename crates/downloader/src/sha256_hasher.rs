use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use crate::sha256_digest::Sha256Digest;

const READ_CHUNK: usize = 1 << 20;

#[derive(Debug)]
#[repr(C)]
struct CommonCryptoContext {
    count: [u32; 2],
    hash: [u32; 8],
    wbuf: [u32; 16],
}

#[link(name = "System", kind = "dylib")]
unsafe extern "C" {
    fn CC_SHA256_Init(context: *mut CommonCryptoContext) -> i32;
    fn CC_SHA256_Update(context: *mut CommonCryptoContext, data: *const u8, length: u32) -> i32;
    fn CC_SHA256_Final(digest: *mut u8, context: *mut CommonCryptoContext) -> i32;
}

#[derive(Debug)]
pub struct Sha256Hasher {
    context: Box<CommonCryptoContext>,
}

impl Default for Sha256Hasher {
    fn default() -> Self {
        let mut context = Box::new(CommonCryptoContext {
            count: [0; 2],
            hash: [0; 8],
            wbuf: [0; 16],
        });
        unsafe { CC_SHA256_Init(&raw mut *context) };
        Self { context }
    }
}

impl Sha256Hasher {
    pub fn update(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(u32::MAX as usize) {
            let length = u32::try_from(chunk.len()).expect("a chunk fits in u32");
            unsafe { CC_SHA256_Update(&raw mut *self.context, chunk.as_ptr(), length) };
        }
    }

    pub fn finish(mut self) -> Sha256Digest {
        let mut digest = [0_u8; 32];
        unsafe { CC_SHA256_Final(digest.as_mut_ptr(), &raw mut *self.context) };
        Sha256Digest::from_bytes(digest)
    }

    pub fn update_from_reader(&mut self, reader: &mut impl Read, mut length: u64) -> io::Result<()> {
        let mut buffer = vec![0_u8; READ_CHUNK];
        while length > 0 {
            let wanted = usize::try_from(length.min(READ_CHUNK as u64)).expect("a chunk fits in usize");
            reader.read_exact(&mut buffer[..wanted])?;
            self.update(&buffer[..wanted]);
            length -= wanted as u64;
        }
        Ok(())
    }

    pub fn digest_of_file(path: &Path) -> io::Result<Sha256Digest> {
        let mut file = File::open(path)?;
        let length = file.metadata()?.len();
        let mut hasher = Self::default();
        hasher.update_from_reader(&mut file, length)?;
        Ok(hasher.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::Sha256Hasher;

    #[test]
    fn known_vectors_match_across_chunk_boundaries() {
        let empty = Sha256Hasher::default().finish();
        assert_eq!(
            empty.to_string(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        let mut split = Sha256Hasher::default();
        split.update(b"ab");
        split.update(b"c");
        assert_eq!(
            split.finish().to_string(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
