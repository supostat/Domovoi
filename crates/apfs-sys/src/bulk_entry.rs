use std::ffi::OsString;
use std::io;
use std::time::SystemTime;

use crate::access_error::AccessError;
use crate::attribute_reader::{AttributeReader, has, invalid};
use crate::object_kind::ObjectKind;
use crate::sys::{
    ATTR_CMN_ADDEDTIME, ATTR_CMN_CRTIME, ATTR_CMN_ERROR, ATTR_CMN_FILEID, ATTR_CMN_FLAGS, ATTR_CMN_MODTIME,
    ATTR_CMN_NAME, ATTR_CMN_OBJTYPE, ATTR_CMNEXT_CLONE_REFCNT, ATTR_CMNEXT_CLONEID, ATTR_CMNEXT_EXT_FLAGS,
    ATTR_CMNEXT_LINKID, ATTR_CMNEXT_PRIVATESIZE, ATTR_CMNEXT_REALDEVID, ATTR_DIR_MOUNTSTATUS, ATTR_FILE_ALLOCSIZE,
    ATTR_FILE_DATAALLOCSIZE, ATTR_FILE_LINKCOUNT, ATTR_FILE_TOTALSIZE,
};

#[derive(Debug)]
pub struct BulkEntry {
    pub name: OsString,
    pub attributes: Result<EntryAttributes, AccessError>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryAttributes {
    pub kind: ObjectKind,
    pub flags: u32,
    pub file_id: u64,
    pub created: SystemTime,
    pub modified: SystemTime,
    pub added: Option<SystemTime>,
    pub mount_status: Option<u32>,
    pub link_count: Option<u32>,
    pub total_size: Option<u64>,
    pub alloc_size: Option<u64>,
    pub data_alloc_size: Option<u64>,
    pub private_size: Option<u64>,
    pub link_id: Option<u64>,
    pub real_dev: Option<u32>,
    pub clone_id: Option<u64>,
    pub ext_flags: Option<u64>,
    pub clone_refcount: Option<u32>,
}

impl BulkEntry {
    pub(crate) fn parse(group: &[u8]) -> io::Result<Self> {
        let mut reader = AttributeReader::new(group);
        let returned = reader.attribute_set()?;
        let error = optional(returned.commonattr, ATTR_CMN_ERROR, || reader.u32())?.unwrap_or(0);
        let name = required(returned.commonattr, ATTR_CMN_NAME, "name", || reader.name())?;
        let attributes = match i32::try_from(error) {
            Ok(0) => Ok(EntryAttributes::read(&mut reader, &returned)?),
            Ok(errno) => Err(AccessError::from_errno(errno)),
            Err(_) => return Err(invalid("entry error code is out of range")),
        };
        Ok(Self { name, attributes })
    }
}

impl EntryAttributes {
    fn read(reader: &mut AttributeReader<'_>, returned: &libc::attribute_set_t) -> io::Result<Self> {
        let common = returned.commonattr;
        let object_type = required(common, ATTR_CMN_OBJTYPE, "object type", || reader.u32())?;
        let created = required(common, ATTR_CMN_CRTIME, "creation time", || reader.timestamp())?;
        let modified = required(common, ATTR_CMN_MODTIME, "modification time", || reader.timestamp())?;
        let flags = required(common, ATTR_CMN_FLAGS, "flags", || reader.u32())?;
        let file_id = required(common, ATTR_CMN_FILEID, "file id", || reader.u64())?;
        let added = optional(common, ATTR_CMN_ADDEDTIME, || reader.timestamp())?;
        let mount_status = optional(returned.dirattr, ATTR_DIR_MOUNTSTATUS, || reader.u32())?;
        let file = returned.fileattr;
        let link_count = optional(file, ATTR_FILE_LINKCOUNT, || reader.u32())?;
        let total_size = optional(file, ATTR_FILE_TOTALSIZE, || reader.size())?;
        let alloc_size = optional(file, ATTR_FILE_ALLOCSIZE, || reader.size())?;
        let data_alloc_size = optional(file, ATTR_FILE_DATAALLOCSIZE, || reader.size())?;
        let extended = returned.forkattr;
        let private_size = optional(extended, ATTR_CMNEXT_PRIVATESIZE, || reader.size())?;
        let link_id = optional(extended, ATTR_CMNEXT_LINKID, || reader.u64())?;
        let real_dev = optional(extended, ATTR_CMNEXT_REALDEVID, || reader.u32())?;
        let clone_id = optional(extended, ATTR_CMNEXT_CLONEID, || reader.u64())?;
        let ext_flags = optional(extended, ATTR_CMNEXT_EXT_FLAGS, || reader.u64())?;
        let clone_refcount = optional(extended, ATTR_CMNEXT_CLONE_REFCNT, || reader.u32())?;
        Ok(Self {
            kind: ObjectKind::from_object_type(object_type),
            flags,
            file_id,
            created,
            modified,
            added,
            mount_status,
            link_count,
            total_size,
            alloc_size,
            data_alloc_size,
            private_size,
            link_id,
            real_dev,
            clone_id,
            ext_flags,
            clone_refcount,
        })
    }
}

fn optional<T>(
    group: libc::attrgroup_t,
    attribute: u32,
    read: impl FnOnce() -> io::Result<T>,
) -> io::Result<Option<T>> {
    if has(group, attribute) {
        read().map(Some)
    } else {
        Ok(None)
    }
}

fn required<T>(
    group: libc::attrgroup_t,
    attribute: u32,
    description: &str,
    read: impl FnOnce() -> io::Result<T>,
) -> io::Result<T> {
    if has(group, attribute) {
        read()
    } else {
        Err(invalid(&format!("kernel did not return the entry {description}")))
    }
}
