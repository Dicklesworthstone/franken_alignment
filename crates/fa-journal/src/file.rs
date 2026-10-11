//! Unix local-file adapter with exclusive writer locking and sync-before-ack.
//!
//! The directory must be trusted, and the filesystem must honor file locks and
//! file/directory synchronization. This is not a hostile-writer, distributed,
//! remote-receipt authentication, or exactly-once external-execution boundary.

use crate::{Event, Head, Identity, Journal, Limits};
use fa_reference::Rights;
use std::fmt;
use std::fs::{File, OpenOptions, TryLockError};
use std::io::{self, BufReader, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

#[derive(Debug)]
pub enum Error {
    Journal(crate::Error),
    Io(io::Error),
    Busy,
    Poisoned,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "file rights journal: {self:?}")
    }
}

impl std::error::Error for Error {}

impl From<crate::Error> for Error {
    fn from(value: crate::Error) -> Self {
        Self::Journal(value)
    }
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

/// One locked writer. No raw file or mutable rights state is exposed.
///
/// A failed write/sync has an *unknown* persistence outcome. The object becomes
/// poisoned before I/O starts, including on unwinding. Drop it and reopen using
/// the last externally retained acknowledged head; do not refund or blindly
/// retry the attempted operation. Recovery may include its complete record.
#[derive(Debug)]
pub struct FileJournal {
    file: File,
    journal: Journal,
    poisoned: bool,
}

impl FileJournal {
    /// Create a new journal without replacing an existing path. The header and
    /// parent directory are synced before success. A failure may leave a partial
    /// file for diagnosis; it is never silently deleted or replaced.
    pub fn create(
        path: impl AsRef<Path>,
        identity: Identity,
        limits: Limits,
    ) -> Result<Self, Error> {
        let journal = Journal::new(identity, limits)?;
        let path = path.as_ref();
        let directory = parent_directory(path)?;
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        lock(&file)?;
        file.write_all(&journal.header())?;
        file.sync_all()?;
        directory.sync_all()?;
        Ok(Self { file, journal, poisoned: false })
    }

    /// Reopen without creating, truncating, repairing, refunding, or dispatching.
    /// `minimum` must be obtained independently of the file being replayed.
    /// Any complete suffix after it is replayed and synced before state is usable.
    pub fn open(
        path: impl AsRef<Path>,
        identity: Identity,
        minimum: Head,
        limits: Limits,
    ) -> Result<Self, Error> {
        // Validate limits/context before opening or reading the supplied path.
        Journal::new(identity, limits)?;
        let path = path.as_ref();
        let directory = parent_directory(path)?;
        let mut file = OpenOptions::new().read(true).write(true).open(path)?;
        lock(&file)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a regular journal file").into());
        }
        if metadata.len() > limits.max_total_bytes {
            return Err(crate::Error::Limit.into());
        }
        let journal = Journal::replay(BufReader::new(&mut file), identity, minimum, limits)?;
        file.sync_all()?;
        directory.sync_all()?;
        Ok(Self { file, journal, poisoned: false })
    }

    /// Persist one checked transition before returning its new recovery anchor.
    /// A dispatch record is accounting, not authorization or an external send.
    pub fn commit(&mut self, event: &Event) -> Result<Head, Error> {
        self.commit_with(event, |file, bytes| {
            file.write_all(bytes)?;
            file.sync_all()
        })
    }

    /// The last successfully published head, usable as a *minimum* on recovery.
    /// After an I/O error the file may contain a complete later record.
    pub fn acknowledged_head(&self) -> Head {
        self.journal.head()
    }

    /// Never expose stale accounting after an uncertain persistence outcome.
    pub fn rights(&self) -> Result<&Rights, Error> {
        if self.poisoned {
            return Err(Error::Poisoned);
        }
        Ok(self.journal.rights())
    }

    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    fn commit_with(
        &mut self,
        event: &Event,
        persist: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
    ) -> Result<Head, Error> {
        if self.poisoned {
            return Err(Error::Poisoned);
        }
        let mut candidate = self.journal.clone();
        let record = candidate.append(event)?;
        // Set this before any I/O: even a panic must not leave a usable stale
        // accounting view. Validation errors above do not poison the writer.
        self.poisoned = true;
        if self.file.metadata()?.len() != self.journal.encoded_len()
            || self.file.seek(SeekFrom::End(0))? != self.journal.encoded_len()
        {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "journal extent changed").into());
        }
        persist(&mut self.file, &record)?;
        self.journal = candidate;
        self.poisoned = false;
        Ok(self.journal.head())
    }
}

fn parent_directory(path: &Path) -> io::Result<File> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let directory = File::open(parent)?;
    if !directory.metadata()?.is_dir() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "journal parent is not a directory"));
    }
    Ok(directory)
}

fn lock(file: &File) -> Result<(), Error> {
    match file.try_lock() {
        Ok(()) => Ok(()),
        Err(TryLockError::WouldBlock) => Err(Error::Busy),
        Err(TryLockError::Error(error)) => Err(Error::Io(error)),
    }
}

#[cfg(test)]
mod tests;
