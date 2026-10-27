use crate::delete_failure::DeleteFailure;
use crate::delete_subject::DeleteSubject;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct DeleteReport {
    pub deleted: Vec<DeleteSubject>,
    pub interrupted: Vec<DeleteSubject>,
    pub untouched: Vec<DeleteSubject>,
    pub failed: Vec<(DeleteSubject, DeleteFailure)>,
    pub freed_bytes: u64,
}
