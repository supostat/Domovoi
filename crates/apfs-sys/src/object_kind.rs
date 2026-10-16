use crate::sys::vtype;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectKind {
    File,
    Directory,
    Symlink,
    Other(u32),
}

impl ObjectKind {
    pub(crate) fn from_object_type(object_type: u32) -> Self {
        match object_type {
            vtype::VREG => Self::File,
            vtype::VDIR => Self::Directory,
            vtype::VLNK => Self::Symlink,
            other => Self::Other(other),
        }
    }
}
