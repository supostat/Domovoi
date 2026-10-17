use domovoi_dups::{CloneRefusal, CloneReplaceError};

use crate::strings::{
    REFUSED_ALREADY_CLONES, REFUSED_BUSY, REFUSED_CHANGED, REFUSED_DENIED, REFUSED_HARD_LINKED, REFUSED_NOT_OWNED,
    REFUSED_NOT_REGULAR, REFUSED_NOT_WRITABLE, REFUSED_OPEN, REFUSED_OTHER_VOLUME, REFUSED_PACKAGE, REFUSED_PROTECTED,
    REFUSED_PURGEABLE, REFUSED_SYNCED, REFUSED_UNREADABLE, REFUSED_VOLUME_CANNOT_CLONE, REFUSED_VOLUME_CANNOT_SWAP,
};

pub(crate) fn clone_refusal_text(error: &CloneReplaceError) -> &'static str {
    match error {
        CloneReplaceError::Refused(refusal) => match refusal {
            CloneRefusal::NotRegularFile(_) => REFUSED_NOT_REGULAR,
            CloneRefusal::OtherVolume => REFUSED_OTHER_VOLUME,
            CloneRefusal::VolumeCannotClone => REFUSED_VOLUME_CANNOT_CLONE,
            CloneRefusal::VolumeCannotSwap => REFUSED_VOLUME_CANNOT_SWAP,
            CloneRefusal::HardLinked => REFUSED_HARD_LINKED,
            CloneRefusal::NotOwned => REFUSED_NOT_OWNED,
            CloneRefusal::NotWritable(_) => REFUSED_NOT_WRITABLE,
            CloneRefusal::ProtectedFlags(_) => REFUSED_PROTECTED,
            CloneRefusal::Purgeable => REFUSED_PURGEABLE,
            CloneRefusal::Busy => REFUSED_BUSY,
            CloneRefusal::Denied(_) => REFUSED_DENIED,
            CloneRefusal::InsidePackage(_) => REFUSED_PACKAGE,
            CloneRefusal::SyncedFolder => REFUSED_SYNCED,
            CloneRefusal::OpenElsewhere(_) => REFUSED_OPEN,
            CloneRefusal::AlreadyClones => REFUSED_ALREADY_CLONES,
            CloneRefusal::ContentChanged => REFUSED_CHANGED,
        },
        CloneReplaceError::OldBodyNotTrashed { .. } | CloneReplaceError::Io(_) => REFUSED_UNREADABLE,
    }
}
