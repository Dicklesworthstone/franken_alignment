//! Hold the original writer lock until strict replay AND synchronization finish.
use super::{Error, FileJournal, lock, parent_directory};
use crate::{Head, Identity, Journal, Limits};
use crate::replay::{HistoryAnchor, Replay, ReplayBudget, ReplayError, ReplayReport, ReplayWork};
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader};
use std::path::Path;

mod history;

// Explicitly bounded buffering below Replay's delivered-byte accounting.
const READ_AHEAD: usize = 8 * 1024;

/// An exclusive recovery session, NOT a usable writer or a historical ledger.
/// Dropping/cancelling closes the file and releases its lock without changing
/// journal bytes. Ready means only that replay finished; finish must still sync
/// the same file and its parent before returning the original FileJournal type.
/// No lock handoff gap, cloned descriptor, truncated tail or replacement image.
///
/// Replay budgets count the BufReader interface, not physical disk traffic.
/// This reader retains at most 8 KiB of read-ahead in addition to replay's frame.
/// Exact-history recovery also retains its independently imported prefix bytes.
/// File/metadata operations and final sync barriers are not poll-time bounds.
/// Original trusted-directory, advisory-lock and filesystem assumptions apply.
///
/// ```compile_fail,E0599
/// use fa_journal::file::FileRecovery;
/// fn no_partial_writer(run: &FileRecovery) { let _ = run.rights(); }
/// ```
#[must_use = "complete recovery and synchronization, or cancel/drop the locked session"]
pub struct FileRecovery {
    replay: Replay<BufReader<File>>,
    directory: File,
}

impl From<ReplayError> for Error {
    fn from(error: ReplayError) -> Self {
        match error {
            ReplayError::Journal(error) => Self::Journal(error),
            ReplayError::NotReady => Self::RecoveryIncomplete,
            ReplayError::Failed => Self::Poisoned,
        }
    }
}

impl FileJournal {
    /// Open and lock an existing regular file; no journal payload is read here.
    /// The independent context and lifetime limits are checked before opening.
    /// Missing paths are never created and damaged files are never repaired.
    pub fn begin_open(
        path: impl AsRef<Path>,
        identity: Identity,
        minimum: Head,
        limits: Limits,
    ) -> Result<FileRecovery, Error> {
        Self::begin_open_journal(path, minimum, Journal::new(identity, limits)?)
    }

    // Shared by legacy and explicitly reserved profiles. Configuration validation
    // precedes file opening; the cursor never infers a profile from stored bytes.
    pub(super) fn begin_open_journal(path: impl AsRef<Path>, minimum: Head, journal: Journal)
        -> Result<FileRecovery, Error>
    {
        Self::begin_open_checked(path, minimum, journal, None)
    }

    fn begin_open_checked(
        path: impl AsRef<Path>,
        minimum: Head,
        journal: Journal,
        anchor: Option<HistoryAnchor>,
    ) -> Result<FileRecovery, Error> {
        if let Some(anchor) = &anchor {
            anchor.check_context(&journal)?;
        }
        let path = path.as_ref();
        let directory = parent_directory(path)?;
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        lock(&file)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a regular journal file").into());
        }
        if metadata.len() > journal.limits.max_total_bytes {
            return Err(crate::Error::Limit.into());
        }
        let reader = BufReader::with_capacity(READ_AHEAD, file);
        let replay = match anchor {
            Some(anchor) => journal.replay_cursor_anchored(reader, anchor)?,
            None => journal.replay_cursor(reader, minimum)?,
        };
        Ok(FileRecovery { replay, directory })
    }
}

impl FileRecovery {
    pub fn advance(&mut self, budget: ReplayBudget) -> Result<ReplayReport, Error> {
        Ok(self.replay.advance(budget)?)
    }

    pub fn progress(&self) -> ReplayReport {
        self.replay.progress()
    }

    /// Release the lock and private reconstruction, retaining only logical work.
    pub fn cancel(self) -> ReplayWork {
        self.replay.cancel()
    }

    /// Consume the session exactly once. An incomplete/failed replay, changed
    /// extent, sync failure or unwind returns no writer and exposes no rights.
    /// The SAME file lock remains held through both synchronization barriers.
    pub fn finish(self) -> Result<FileJournal, Error> {
        self.finish_with(|file, directory| {
            file.sync_all()?;
            directory.sync_all()
        })
    }

    fn finish_with(self, synchronize: impl FnOnce(&File, &File) -> io::Result<()>)
        -> Result<FileJournal, Error>
    {
        let (reader, journal) = self.replay.into_parts()?;
        let file = reader.into_inner();
        if file.metadata()?.len() != journal.encoded_len() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "journal extent changed during recovery").into());
        }
        synchronize(&file, &self.directory)?;
        Ok(FileJournal { file, journal, poisoned: false })
    }
}

#[cfg(test)]
mod tests;
