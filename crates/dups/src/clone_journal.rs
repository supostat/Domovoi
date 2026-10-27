use std::env;
use std::fs;
use std::io;
use std::mem;
use std::path::{Path, PathBuf};

use domovoi_apfs_sys::{link_id, write_atomically};
use domovoi_rules::{DeleteFailure, DeleteReport, DeleteSubject, delete};
use domovoi_scanner::CancelToken;
use rkyv::rancor;

use crate::pending_replacement::PendingReplacement;

const JOURNAL_PATH: &str = "Library/Application Support/com.domovoi.app/clone-journal";

#[derive(Debug)]
pub struct CloneJournal {
    path: PathBuf,
    pending: Vec<PendingReplacement>,
}

impl CloneJournal {
    pub fn default_path() -> io::Result<PathBuf> {
        env::home_dir()
            .map(|home| home.join(JOURNAL_PATH))
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "home directory is unknown"))
    }

    pub fn open(path: &Path) -> io::Result<Self> {
        let pending = match fs::read(path) {
            Ok(bytes) => {
                let mut aligned = rkyv::util::AlignedVec::<16>::new();
                aligned.extend_from_slice(&bytes);
                rkyv::from_bytes::<Vec<PendingReplacement>, rancor::Error>(&aligned)
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error),
        };
        Ok(Self {
            path: path.to_path_buf(),
            pending,
        })
    }

    pub fn pending(&self) -> &[PendingReplacement] {
        &self.pending
    }

    pub fn begin(&mut self, replacement: PendingReplacement) -> io::Result<()> {
        self.pending.push(replacement);
        self.save()
    }

    pub(crate) fn settle(&mut self, temporary: &Path) -> io::Result<()> {
        let position = self
            .pending
            .iter()
            .position(|replacement| Path::new(&replacement.temporary) == temporary)
            .expect("a replacement is journaled before it starts");
        let settled = match temporary_subject(&self.pending[position])? {
            None => true,
            Some(subject) => !needs_another_try(&delete_temporaries(vec![subject]), temporary),
        };
        if settled {
            self.pending.remove(position);
            self.save()?;
        }
        Ok(())
    }

    pub fn sweep(&mut self) -> io::Result<DeleteReport> {
        let mut subjects = Vec::new();
        let mut remaining = Vec::new();
        let mut first_error = None;
        let mut swept = Vec::new();
        for replacement in mem::take(&mut self.pending) {
            match temporary_subject(&replacement) {
                Ok(Some(subject)) => {
                    subjects.push(subject);
                    swept.push(replacement);
                }
                Ok(None) => {}
                Err(error) => {
                    remaining.push(replacement);
                    first_error.get_or_insert(error);
                }
            }
        }
        let report = delete_temporaries(subjects);
        remaining.extend(
            swept
                .into_iter()
                .filter(|replacement| needs_another_try(&report, Path::new(&replacement.temporary))),
        );
        self.pending = remaining;
        self.save()?;
        match first_error {
            Some(error) => Err(error),
            None => Ok(report),
        }
    }

    fn save(&self) -> io::Result<()> {
        let bytes = rkyv::to_bytes::<rancor::Error>(&self.pending).map_err(io::Error::other)?;
        write_atomically(&self.path, &bytes)
    }
}

fn temporary_subject(replacement: &PendingReplacement) -> io::Result<Option<DeleteSubject>> {
    match link_id(Path::new(&replacement.temporary)) {
        Ok(current) => Ok(Some(DeleteSubject {
            link_id: current,
            ..replacement.old_body()
        })),
        Err(error) if error.raw_os_error() == Some(libc::ENOENT) => Ok(None),
        Err(error) => Err(error),
    }
}

fn delete_temporaries(subjects: Vec<DeleteSubject>) -> DeleteReport {
    delete(subjects, &CancelToken::default(), &|_| {})
}

fn needs_another_try(report: &DeleteReport, temporary: &Path) -> bool {
    report
        .failed
        .iter()
        .any(|(subject, failure)| subject.physical == temporary && *failure != DeleteFailure::Missing)
}
