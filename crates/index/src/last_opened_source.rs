#[derive(Clone, Copy, Debug, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum LastOpenedSource {
    Unknown,
    Xattr,
    SpotlightIndex,
    Inherited,
}
