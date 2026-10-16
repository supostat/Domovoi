use std::ffi::CStr;

use domovoi_xpc_sys::XpcDictionary;

const COUNT: &CStr = c"count";

#[test]
fn a_cloned_dictionary_changes_independently() {
    let mut original = XpcDictionary::new();
    original.set_i64(COUNT, 1);
    let mut copy = original.clone();
    copy.set_i64(COUNT, 2);
    assert_eq!(original.i64(COUNT), Some(1));
    assert_eq!(copy.i64(COUNT), Some(2));
}
