use domovoi_apfs_sys::sys::{
    ATTR_CMN_ERROR, ATTR_CMNEXT_CLONE_REFCNT, ATTR_CMNEXT_PRIVATESIZE, DIR_MNTSTATUS_MNTPOINT, EF_IS_PURGEABLE,
    EF_IS_SPARSE, EF_MAY_SHARE_BLOCKS, EF_SHARES_ALL_BLOCKS, FSOPT_ATTR_CMN_EXTENDED, SF_DATALESS, SF_FIRMLINK,
    SF_RESTRICTED,
};

#[test]
fn extended_flags_match_the_pinned_sdk() {
    assert_eq!(EF_SHARES_ALL_BLOCKS, 0x40);
    assert_eq!(EF_MAY_SHARE_BLOCKS, 0x1);
    assert_eq!(EF_IS_PURGEABLE, 0x8);
    assert_eq!(EF_IS_SPARSE, 0x10);
}

#[test]
fn attribute_bits_match_the_pinned_sdk() {
    assert_eq!(ATTR_CMNEXT_CLONE_REFCNT, 0x1000);
    assert_eq!(ATTR_CMNEXT_PRIVATESIZE, 0x8);
    assert_eq!(ATTR_CMN_ERROR, 0x2000_0000);
    assert_eq!(FSOPT_ATTR_CMN_EXTENDED, 0x20);
    assert_eq!(DIR_MNTSTATUS_MNTPOINT, 0x1);
}

#[test]
fn system_file_flags_match_the_pinned_sdk() {
    assert_eq!(SF_FIRMLINK, 0x0080_0000);
    assert_eq!(SF_RESTRICTED, 0x0008_0000);
    assert_eq!(SF_DATALESS, 0x4000_0000);
}
