//! Cooperative NEW batch writes under the original owner lock, never a replay
//! or suffix repair. Only final synchronization acknowledges new accounting.
use super::{Error, FileJournal};
use crate::{Event, Head, Journal};
use crate::batch::{BatchLimits, BatchPreparation, BatchPreparationError, BatchPreparationStage};
use std::fmt;
use std::fs::File;
use std::io::{self, Seek, SeekFrom, Write};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BatchCommitBudget {
    pub prepared_records: usize,
    /// Write attempts, including Interrupted returns. No write_all retry loop.
    pub write_calls: u64,
    /// Bytes reported written, not physical-media traffic or synchronization.
    pub write_bytes: u64,
}
impl BatchCommitBudget {
    pub const UNLIMITED: Self = Self { prepared_records: usize::MAX,
        write_calls: u64::MAX, write_bytes: u64::MAX };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchCommitStage {
    Preparing,
    Writing,
    /// All bytes were written, but finish MUST still synchronize before ack.
    Ready,
    Failed,
}

/// Non-authoritative progress. An errored write may have persisted bytes beyond
/// written_bytes; a complete file prefix is not a returned accounting receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BatchCommitReport {
    pub stage: BatchCommitStage,
    pub total_records: usize,
    pub prepared_records: usize,
    pub preparation_attempts: usize,
    pub write_calls: u64,
    pub written_bytes: u64,
}

/// Consume the ONLY original FileJournal. No rights, file, cloned owner or
/// prepared-ledger importer is exposed while validation/writing can be partial.
/// The same descriptor and advisory lock survive preparation, writes and sync.
///
/// ```compile_fail,E0599
/// use fa_journal::file::FileBatchCommit;
/// fn partial(run: &FileBatchCommit) { let _ = run.rights(); }
/// ```
/// ```compile_fail,E0599
/// use fa_journal::file::FileBatchCommit;
/// fn fork(run: FileBatchCommit) { let _ = run.clone(); }
/// ```
#[must_use = "finish with synchronization, or cancel/drop and strictly reopen the original file"]
pub struct FileBatchCommit {
    owner: FileJournal,
    preparation: Option<BatchPreparation>,
    candidate: Option<Journal>,
    bytes: Vec<u8>,
    written: usize,
    report: BatchCommitReport,
}
impl fmt::Debug for FileBatchCommit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileBatchCommit").field("progress", &self.progress()).finish_non_exhaustive()
    }
}

impl FileJournal {
    /// Start a NEW explicit group; this does not match or complete old batches.
    /// Freeze inputs and do cheap whole-group admission before the private clone.
    /// Invalid admission consumes/drops this owner without a write. Every event
    /// must validate before any file I/O; further native commits cannot interleave.
    pub fn begin_commit_batch(self, events: Vec<Event>, limits: BatchLimits)
        -> Result<FileBatchCommit, Error>
    {
        if self.poisoned { return Err(Error::Poisoned); }
        let preparation = self.journal.begin_append_batch(events, limits)?;
        let total_records = preparation.progress().total_records;
        Ok(FileBatchCommit { owner: self, preparation: Some(preparation), candidate: None,
            bytes: Vec::new(), written: 0, report: BatchCommitReport {
                stage: BatchCommitStage::Preparing, total_records, prepared_records: 0,
                preparation_attempts: 0, write_calls: 0, written_bytes: 0,
            } })
    }
}

impl FileBatchCommit {
    /// Run only the CURRENT stage, yielding before the next. No poll syncs or
    /// acknowledges accounting. Budgets bound logical work, not syscall latency;
    /// metadata/seek operations and the initial clone are outside these counts.
    pub fn advance(&mut self, budget: BatchCommitBudget) -> Result<BatchCommitReport, Error> {
        self.advance_with(budget, |file, bytes| file.write(bytes))
    }

    /// Only the ORIGINAL acknowledged minimum; not the candidate's newer head.
    pub fn acknowledged_head(&self) -> Head { self.owner.acknowledged_head() }

    pub fn progress(&self) -> BatchCommitReport {
        let mut report = self.report;
        if let Some(preparation) = &self.preparation {
            let work = preparation.progress();
            report.prepared_records = work.prepared_records;
            report.preparation_attempts = work.attempted_records;
        }
        report
    }

    /// After complete writes, check extent and sync the SAME file before returning
    /// the original owner with its complete candidate installed. Failure/unwind
    /// returns no owner or new head; recovery must retain the old minimum.
    pub fn finish(self) -> Result<FileJournal, Error> {
        self.finish_with(|file| file.sync_all())
    }

    /// No cancellation rollback or refund. Before writing, bytes are unchanged;
    /// afterwards any complete prefix or torn record can remain. Drop the lock,
    /// then use the ORIGINAL strict open path with an independent recovery head.
    pub fn cancel(self) -> BatchCommitReport { self.progress() }

    // Private hooks inject real file failures in tests, not alternate public I/O.
    fn advance_with(&mut self, budget: BatchCommitBudget,
        mut write: impl FnMut(&mut File, &[u8]) -> io::Result<usize>)
        -> Result<BatchCommitReport, Error>
    {
        match self.report.stage {
            BatchCommitStage::Ready => return Ok(self.progress()),
            BatchCommitStage::Failed => return Err(Error::Poisoned),
            BatchCommitStage::Preparing => {
                self.report.stage = BatchCommitStage::Failed;
                let preparation = self.preparation.as_mut().ok_or(Error::CommitIncomplete)?;
                let progress = preparation.advance(budget.prepared_records).map_err(preparation_error)?;
                if progress.stage == BatchPreparationStage::Ready {
                    let preparation = self.preparation.take().ok_or(Error::CommitIncomplete)?;
                    let (candidate, bytes) = preparation.finish().map_err(preparation_error)?;
                    self.report.prepared_records = progress.prepared_records;
                    self.report.preparation_attempts = progress.attempted_records;
                    self.candidate = Some(candidate);
                    self.bytes = bytes;
                    self.report.stage = BatchCommitStage::Writing;
                } else { self.report.stage = BatchCommitStage::Preparing; }
                return Ok(self.progress());
            }
            BatchCommitStage::Writing => {}
        }
        if budget.write_calls == 0 || budget.write_bytes == 0 { return Ok(self.progress()); }
        self.report.stage = BatchCommitStage::Failed;
        self.owner.poisoned = true; // BEFORE extent checks, writes and caught unwinds.
        let mut calls = 0;
        let mut bytes = 0;
        while self.written < self.bytes.len() && calls < budget.write_calls && bytes < budget.write_bytes {
            let expected = self.owner.journal.encoded_len().checked_add(self.written as u64)
                .ok_or(crate::Error::Limit)?;
            self.check_extent(expected)?;
            let amount = (budget.write_bytes - bytes).min((self.bytes.len() - self.written) as u64) as usize;
            self.report.write_calls = self.report.write_calls.checked_add(1).ok_or(crate::Error::Limit)?;
            calls += 1;
            let count = match write(&mut self.owner.file, &self.bytes[self.written..self.written + amount]) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                result => result?,
            };
            if count == 0 { return Err(io::Error::from(io::ErrorKind::WriteZero).into()); }
            if count > amount {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "write overreported length").into());
            }
            self.written += count;
            bytes += count as u64;
            self.report.written_bytes = self.written as u64;
        }
        self.report.stage = if self.written == self.bytes.len() { BatchCommitStage::Ready }
            else { BatchCommitStage::Writing };
        Ok(self.progress())
    }

    fn check_extent(&mut self, expected: u64) -> Result<(), Error> {
        if self.owner.file.metadata()?.len() != expected
            || self.owner.file.seek(SeekFrom::End(0))? != expected {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "journal extent changed during batch").into());
        }
        Ok(())
    }

    fn finish_with(mut self, sync: impl FnOnce(&File) -> io::Result<()>) -> Result<FileJournal, Error> {
        if self.report.stage == BatchCommitStage::Failed { return Err(Error::Poisoned); }
        if self.report.stage != BatchCommitStage::Ready { return Err(Error::CommitIncomplete); }
        self.report.stage = BatchCommitStage::Failed;
        self.owner.poisoned = true;
        let candidate = self.candidate.take().ok_or(Error::CommitIncomplete)?;
        self.check_extent(candidate.encoded_len())?;
        sync(&self.owner.file)?;
        self.owner.journal = candidate;
        self.owner.poisoned = false;
        Ok(self.owner)
    }
}

fn preparation_error(error: BatchPreparationError) -> Error {
    match error {
        BatchPreparationError::Journal(error) => Error::Journal(error),
        BatchPreparationError::NotReady => Error::CommitIncomplete,
        BatchPreparationError::Failed => Error::Poisoned,
    }
}

#[cfg(test)]
mod tests;
