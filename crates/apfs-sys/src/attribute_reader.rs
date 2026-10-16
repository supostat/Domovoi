use std::ffi::OsString;
use std::io;
use std::os::unix::ffi::OsStringExt;
use std::time::SystemTime;

use crate::timestamp::system_time;

const LENGTH_FIELD: usize = 4;

pub(crate) struct AttributeReader<'group> {
    group: &'group [u8],
    position: usize,
}

impl<'group> AttributeReader<'group> {
    pub(crate) fn new(group: &'group [u8]) -> Self {
        Self {
            group,
            position: LENGTH_FIELD,
        }
    }

    pub(crate) fn attribute_set(&mut self) -> io::Result<libc::attribute_set_t> {
        Ok(libc::attribute_set_t {
            commonattr: self.u32()?,
            volattr: self.u32()?,
            dirattr: self.u32()?,
            fileattr: self.u32()?,
            forkattr: self.u32()?,
        })
    }

    pub(crate) fn u32(&mut self) -> io::Result<u32> {
        self.bytes().map(u32::from_ne_bytes)
    }

    pub(crate) fn u64(&mut self) -> io::Result<u64> {
        self.bytes().map(u64::from_ne_bytes)
    }

    pub(crate) fn size(&mut self) -> io::Result<u64> {
        let size = self.bytes().map(i64::from_ne_bytes)?;
        u64::try_from(size).map_err(|_| invalid("negative size attribute"))
    }

    pub(crate) fn timestamp(&mut self) -> io::Result<SystemTime> {
        let seconds = self.bytes().map(i64::from_ne_bytes)?;
        let nanoseconds = self.bytes().map(i64::from_ne_bytes)?;
        system_time(seconds, nanoseconds)
    }

    pub(crate) fn capabilities(&mut self) -> io::Result<libc::vol_capabilities_attr_t> {
        let mut capabilities = [0; 4];
        let mut valid = [0; 4];
        for capability in &mut capabilities {
            *capability = self.u32()?;
        }
        for validity in &mut valid {
            *validity = self.u32()?;
        }
        Ok(libc::vol_capabilities_attr_t { capabilities, valid })
    }

    pub(crate) fn name(&mut self) -> io::Result<OsString> {
        let reference_start = self.position;
        let offset = self.bytes().map(i32::from_ne_bytes)?;
        let length = self.u32()?;
        let start = isize::try_from(offset)
            .ok()
            .and_then(|offset| reference_start.checked_add_signed(offset))
            .ok_or_else(|| invalid("name offset points outside the entry"))?;
        let end = start
            .checked_add(usize::try_from(length).map_err(|_| invalid("name length overflows"))?)
            .ok_or_else(|| invalid("name length overflows"))?;
        let stored = self
            .group
            .get(start..end)
            .ok_or_else(|| invalid("name points outside the entry"))?;
        let name_length = stored.iter().position(|byte| *byte == 0).unwrap_or(stored.len());
        Ok(OsString::from_vec(stored[..name_length].to_vec()))
    }

    fn bytes<const LENGTH: usize>(&mut self) -> io::Result<[u8; LENGTH]> {
        let end = self.position + LENGTH;
        let bytes = self
            .group
            .get(self.position..end)
            .ok_or_else(|| invalid("attribute buffer ends inside an attribute"))?;
        self.position = end;
        Ok(bytes.try_into().expect("the slice has exactly LENGTH bytes"))
    }
}

pub(crate) fn packed_group(buffer: &[u8], start: usize) -> io::Result<&[u8]> {
    let length_bytes = buffer
        .get(start..start + LENGTH_FIELD)
        .ok_or_else(|| invalid("entry count exceeds the filled buffer"))?;
    let length = u32::from_ne_bytes(length_bytes.try_into().expect("four length bytes"));
    let length = usize::try_from(length).map_err(|_| invalid("entry length overflows"))?;
    if length < LENGTH_FIELD {
        return Err(invalid("kernel returned an empty entry"));
    }
    buffer
        .get(start..start + length)
        .ok_or_else(|| invalid("entry extends past the buffer"))
}

pub(crate) fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_owned())
}

pub(crate) fn has(group: libc::attrgroup_t, attribute: u32) -> bool {
    group & attribute != 0
}
