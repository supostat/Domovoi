use std::ffi::CStr;
use std::os::fd::OwnedFd;

use domovoi_xpc_sys::XpcDictionary;

use crate::decode_error::DecodeError;

pub(crate) fn absent_or_invalid(dictionary: &XpcDictionary, key: &'static CStr) -> DecodeError {
    if dictionary.contains(key) {
        DecodeError::InvalidField(key)
    } else {
        DecodeError::MissingField(key)
    }
}

pub(crate) fn u64_field(dictionary: &XpcDictionary, key: &'static CStr) -> Result<u64, DecodeError> {
    dictionary.u64(key).ok_or_else(|| absent_or_invalid(dictionary, key))
}

pub(crate) fn u32_field(dictionary: &XpcDictionary, key: &'static CStr) -> Result<u32, DecodeError> {
    u32::try_from(u64_field(dictionary, key)?).map_err(|_| DecodeError::InvalidField(key))
}

pub(crate) fn i32_field(dictionary: &XpcDictionary, key: &'static CStr) -> Result<i32, DecodeError> {
    let value = dictionary.i64(key).ok_or_else(|| absent_or_invalid(dictionary, key))?;
    i32::try_from(value).map_err(|_| DecodeError::InvalidField(key))
}

pub(crate) fn f64_field(dictionary: &XpcDictionary, key: &'static CStr) -> Result<f64, DecodeError> {
    dictionary.f64(key).ok_or_else(|| absent_or_invalid(dictionary, key))
}

pub(crate) fn text_field(dictionary: &XpcDictionary, key: &'static CStr) -> Result<String, DecodeError> {
    let bytes = dictionary
        .bytes(key)
        .ok_or_else(|| absent_or_invalid(dictionary, key))?;
    String::from_utf8(bytes.to_vec()).map_err(|_| DecodeError::InvalidField(key))
}

pub(crate) fn optional_text_field(
    dictionary: &XpcDictionary,
    key: &'static CStr,
) -> Result<Option<String>, DecodeError> {
    if dictionary.contains(key) {
        text_field(dictionary, key).map(Some)
    } else {
        Ok(None)
    }
}

pub(crate) fn named_field<T>(
    dictionary: &XpcDictionary,
    key: &'static CStr,
    from_name: impl Fn(&CStr) -> Option<T>,
) -> Result<T, DecodeError> {
    let name = dictionary
        .string(key)
        .ok_or_else(|| absent_or_invalid(dictionary, key))?;
    from_name(name).ok_or(DecodeError::InvalidField(key))
}

pub(crate) fn fd_field(dictionary: &XpcDictionary, key: &'static CStr) -> Result<OwnedFd, DecodeError> {
    dictionary.dup_fd(key).ok_or_else(|| absent_or_invalid(dictionary, key))
}
