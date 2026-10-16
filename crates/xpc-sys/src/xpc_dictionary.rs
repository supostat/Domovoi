use std::ffi::CStr;
use std::io;
use std::os::fd::{BorrowedFd, FromRawFd, OwnedFd};
use std::slice;

use crate::sys::{
    _xpc_type_bool, _xpc_type_data, _xpc_type_dictionary, _xpc_type_double, _xpc_type_fd, _xpc_type_int64, _xpc_type_s,
    _xpc_type_string, _xpc_type_uint64, xpc_copy, xpc_dictionary_create_empty, xpc_dictionary_create_reply,
    xpc_dictionary_dup_fd, xpc_dictionary_get_bool, xpc_dictionary_get_data, xpc_dictionary_get_double,
    xpc_dictionary_get_int64, xpc_dictionary_get_string, xpc_dictionary_get_uint64, xpc_dictionary_get_value,
    xpc_dictionary_set_bool, xpc_dictionary_set_data, xpc_dictionary_set_double, xpc_dictionary_set_int64,
    xpc_dictionary_set_string, xpc_dictionary_set_uint64, xpc_dictionary_set_value, xpc_object_t,
};
use crate::xpc_object::XpcObject;

#[derive(Debug)]
pub struct XpcDictionary {
    object: XpcObject,
}

impl Clone for XpcDictionary {
    fn clone(&self) -> Self {
        Self {
            object: unsafe { XpcObject::from_owned(xpc_copy(self.raw())) },
        }
    }
}

impl XpcDictionary {
    pub fn new() -> Self {
        Self {
            object: unsafe { XpcObject::from_owned(xpc_dictionary_create_empty()) },
        }
    }

    pub fn from_object(object: XpcObject) -> Option<Self> {
        object
            .has_type(unsafe { &_xpc_type_dictionary })
            .then_some(Self { object })
    }

    pub fn as_object(&self) -> &XpcObject {
        &self.object
    }

    pub fn reply(&self) -> Option<Self> {
        let raw = unsafe { xpc_dictionary_create_reply(self.raw()) };
        (!raw.is_null()).then(|| Self {
            object: unsafe { XpcObject::from_owned(raw) },
        })
    }

    pub fn contains(&self, key: &CStr) -> bool {
        !unsafe { xpc_dictionary_get_value(self.raw(), key.as_ptr()) }.is_null()
    }

    pub fn set_i64(&mut self, key: &CStr, value: i64) {
        unsafe { xpc_dictionary_set_int64(self.raw(), key.as_ptr(), value) };
    }

    pub fn set_u64(&mut self, key: &CStr, value: u64) {
        unsafe { xpc_dictionary_set_uint64(self.raw(), key.as_ptr(), value) };
    }

    pub fn set_bool(&mut self, key: &CStr, value: bool) {
        unsafe { xpc_dictionary_set_bool(self.raw(), key.as_ptr(), value) };
    }

    pub fn set_f64(&mut self, key: &CStr, value: f64) {
        unsafe { xpc_dictionary_set_double(self.raw(), key.as_ptr(), value) };
    }

    pub fn set_string(&mut self, key: &CStr, value: &CStr) {
        unsafe { xpc_dictionary_set_string(self.raw(), key.as_ptr(), value.as_ptr()) };
    }

    pub fn set_bytes(&mut self, key: &CStr, value: &[u8]) {
        unsafe { xpc_dictionary_set_data(self.raw(), key.as_ptr(), value.as_ptr().cast(), value.len()) };
    }

    pub fn set_fd(&mut self, key: &CStr, file: BorrowedFd<'_>) -> io::Result<()> {
        let descriptor = XpcObject::file_descriptor(file)?;
        self.set_object(key, &descriptor);
        Ok(())
    }

    pub fn set_dictionary(&mut self, key: &CStr, value: &XpcDictionary) {
        self.set_object(key, &value.object);
    }

    pub fn set_object(&mut self, key: &CStr, value: &XpcObject) {
        unsafe { xpc_dictionary_set_value(self.raw(), key.as_ptr(), value.as_raw()) };
    }

    pub fn i64(&self, key: &CStr) -> Option<i64> {
        self.typed(key, unsafe { &_xpc_type_int64 })
            .then(|| unsafe { xpc_dictionary_get_int64(self.raw(), key.as_ptr()) })
    }

    pub fn u64(&self, key: &CStr) -> Option<u64> {
        self.typed(key, unsafe { &_xpc_type_uint64 })
            .then(|| unsafe { xpc_dictionary_get_uint64(self.raw(), key.as_ptr()) })
    }

    pub fn bool(&self, key: &CStr) -> Option<bool> {
        self.typed(key, unsafe { &_xpc_type_bool })
            .then(|| unsafe { xpc_dictionary_get_bool(self.raw(), key.as_ptr()) })
    }

    pub fn f64(&self, key: &CStr) -> Option<f64> {
        self.typed(key, unsafe { &_xpc_type_double })
            .then(|| unsafe { xpc_dictionary_get_double(self.raw(), key.as_ptr()) })
    }

    pub fn string(&self, key: &CStr) -> Option<&CStr> {
        self.typed(key, unsafe { &_xpc_type_string })
            .then(|| unsafe { CStr::from_ptr(xpc_dictionary_get_string(self.raw(), key.as_ptr())) })
    }

    pub fn bytes(&self, key: &CStr) -> Option<&[u8]> {
        if !self.typed(key, unsafe { &_xpc_type_data }) {
            return None;
        }
        let mut length = 0;
        let start = unsafe { xpc_dictionary_get_data(self.raw(), key.as_ptr(), &raw mut length) };
        Some(if length == 0 {
            &[]
        } else {
            unsafe { slice::from_raw_parts(start.cast::<u8>(), length) }
        })
    }

    pub fn dup_fd(&self, key: &CStr) -> Option<OwnedFd> {
        if !self.typed(key, unsafe { &_xpc_type_fd }) {
            return None;
        }
        let descriptor = unsafe { xpc_dictionary_dup_fd(self.raw(), key.as_ptr()) };
        (descriptor >= 0).then(|| unsafe { OwnedFd::from_raw_fd(descriptor) })
    }

    pub fn dictionary(&self, key: &CStr) -> Option<XpcDictionary> {
        let value = self.value(key)?;
        Self::from_object(value)
    }

    pub fn value(&self, key: &CStr) -> Option<XpcObject> {
        let raw = unsafe { xpc_dictionary_get_value(self.raw(), key.as_ptr()) };
        (!raw.is_null()).then(|| unsafe { XpcObject::from_borrowed(raw) })
    }

    fn typed(&self, key: &CStr, kind: &_xpc_type_s) -> bool {
        self.value(key).is_some_and(|value| value.has_type(kind))
    }

    fn raw(&self) -> xpc_object_t {
        self.object.as_raw()
    }
}

impl Default for XpcDictionary {
    fn default() -> Self {
        Self::new()
    }
}
