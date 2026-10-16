use std::env;
use std::fs;
use std::io;
use std::mem;
use std::path::{Path, PathBuf};

use domovoi_apfs_sys::{link_id, write_atomically};
use domovoi_rules::{TrashJournal, TrashOutcome, trash_item};
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

    pub(crate) fn settle(&mut self, temporary: &Path, trash_journal: &mut TrashJournal) -> io::Result<()> {
        let position = self
            .pending
            .iter()
            .position(|replacement| Path::new(&replacement.temporary) == temporary)
            .expect("a replacement is journaled before it starts");
        let settled = match sweep_one(&self.pending[position], trash_journal)? {
            None => true,
            Some(outcome) => outcome.result.is_ok(),
        };
        if settled {
            self.pending.remove(position);
            self.save()?;
        }
        Ok(())
    }

    pub fn sweep(&mut self, trash_journal: &mut TrashJournal) -> io::Result<Vec<TrashOutcome>> {
        let mut outcomes = Vec::new();
        let mut remaining = Vec::new();
        let mut first_error = None;
        for replacement in mem::take(&mut self.pending) {
            match sweep_one(&replacement, trash_journal) {
                Ok(Some(outcome)) => {
                    if outcome.result.is_err() {
                        remaining.push(replacement);
                    }
                    outcomes.push(outcome);
                }
                Ok(None) => {}
                Err(error) => {
                    remaining.push(replacement);
                    first_error.get_or_insert(error);
                }
            }
        }
        self.pending = remaining;
        self.save()?;
        match first_error {
            Some(error) => Err(error),
            None => Ok(outcomes),
        }
    }

    fn save(&self) -> io::Result<()> {
        let bytes = rkyv::to_bytes::<rancor::Error>(&self.pending).map_err(io::Error::other)?;
        write_atomically(&self.path, &bytes)
    }
}

fn sweep_one(replacement: &PendingReplacement, trash_journal: &mut TrashJournal) -> io::Result<Option<TrashOutcome>> {
    let temporary = Path::new(&replacement.temporary);
    let current = match link_id(temporary) {
        Ok(current) => current,
        Err(error) if error.raw_os_error() == Some(libc::ENOENT) => return Ok(None),
        Err(error) => return Err(error),
    };
    let result = if current == replacement.link_id {
        trash_journal.trash(&replacement.old_body())?
    } else {
        trash_item(temporary)?
    };
    Ok(Some(TrashOutcome {
        path: temporary.to_path_buf(),
        result,
    }))
}
