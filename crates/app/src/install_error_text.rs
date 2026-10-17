use domovoi_downloader::InstallError;

use crate::strings::{
    INSTALL_DIGEST_MISMATCH, INSTALL_DISK, INSTALL_NETWORK, INSTALL_NO_SPACE, INSTALL_REFUSED, INSTALL_SIZE_MISMATCH,
    INSTALL_SOURCE_CHANGED,
};

pub(crate) fn install_error_text(error: &InstallError) -> &'static str {
    match error {
        InstallError::InsufficientSpace { .. } => INSTALL_NO_SPACE,
        InstallError::RevisionMismatch { .. } => INSTALL_SOURCE_CHANGED,
        InstallError::DigestMismatch { .. } => INSTALL_DIGEST_MISMATCH,
        InstallError::SizeMismatch { .. } => INSTALL_SIZE_MISMATCH,
        InstallError::UnexpectedStatus(_) | InstallError::AccessDenied => INSTALL_REFUSED,
        InstallError::Network(_) => INSTALL_NETWORK,
        InstallError::Io(_) | InstallError::Trash(_) => INSTALL_DISK,
    }
}
