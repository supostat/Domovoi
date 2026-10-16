use domovoi_apfs_sys::ObjectKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum NodeKind {
    File,
    Directory,
    Symlink,
    Other,
    Unknown,
}

impl NodeKind {
    pub(crate) fn of(kind: Option<ObjectKind>) -> Self {
        match kind {
            Some(ObjectKind::File) => Self::File,
            Some(ObjectKind::Directory) => Self::Directory,
            Some(ObjectKind::Symlink) => Self::Symlink,
            Some(ObjectKind::Other(_)) => Self::Other,
            None => Self::Unknown,
        }
    }
}
