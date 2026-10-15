//! Yield while verifying an original batch; never return a checked prefix as rights.
mod resumption;
mod anchored;

use super::{BatchAnchor, BatchStatus, Error, FileJournal};
use crate::{Event, Head, Journal};
use crate::batch::{BatchLimits, MAX_BATCH_ENCODED_BYTES, MAX_BATCH_RECORDS};
use crate::replay::{HistoryAnchor, Replay, ReplayBudget, ReplayStatus, ReplayWork};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Take};

/// Independent per-poll ceilings for the three stages. They bound logical work,
/// not wall time: one original reducer can inspect all retained rights, and a
/// file read can block. Metadata, seeking and final owner extraction are separate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BatchInspectionBudget {
    pub replay: ReplayBudget,
    pub prepare_records: u64,
    pub compare_calls: u64,
    pub compare_bytes: u64,
}
impl BatchInspectionBudget {
    pub const UNLIMITED: Self = Self { replay: ReplayBudget::UNLIMITED,
        prepare_records: u64::MAX, compare_calls: u64::MAX, compare_bytes: u64::MAX };
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BatchInspectionWork {
    pub replay: ReplayWork,
    /// Original single-record batch-preparation attempts, including a refusal.
    pub prepared_records: u64,
    /// Reads attempted at the suffix interface, including Interrupted and EOF.
    pub compare_calls: u64,
    /// Bytes returned at that interface, including mismatching bytes.
    pub compare_bytes: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchInspectionStatus { ReplayPrefix, PrepareBatch, CompareSuffix, Ready, Failed }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BatchInspectionReport {
    pub status: BatchInspectionStatus,
    pub total: BatchInspectionWork,
}

/// Own the SAME locked descriptor and the last acknowledged journal throughout.
/// No partial status, ledger, file handle, clone or writable owner escapes.
/// Dropping or canceling releases the lock without changing any journal bytes.
/// Only finish returns the original owner after exact prefix/suffix verification.
/// An explicitly selected resumption may instead transfer to the sealed writer;
/// neither route exposes an intermediate ledger or permits interleaved commits.
///
/// The caller retains the original anchor and events independently; this does
/// not authenticate unstored intentions or observe external effect outcomes.
/// At most the original frame, bounded batch bytes, an 8 KiB comparison buffer
/// and private original ledgers are retained, plus caller-supplied event storage.
/// Exact-history inspection additionally retains its immutable pre-group prefix.
/// Each prepared record uses the original batch primitive and clones its private
/// ledger; this optional yielding path does not claim lower memory or throughput.
///
/// ```compile_fail,E0599
/// use fa_journal::file::FileBatchInspection;
/// fn no_prefix(run: &FileBatchInspection) { let _ = run.rights(); }
/// ```
/// ```compile_fail,E0599
/// use fa_journal::file::FileBatchInspection;
/// fn no_fork(run: FileBatchInspection) { let _ = run.clone(); }
/// ```
#[must_use = "finish exact verification, or cancel/drop the locked inspection"]
pub struct FileBatchInspection {
    replay: Option<Replay<Take<File>>>,
    file: Option<File>,
    acknowledged: Journal,
    candidate: Option<Journal>,
    // Only explicit resumption selects transfer of a fully verified final candidate.
    retain_candidate: bool,
    anchor: BatchAnchor,
    events: Vec<Event>,
    limits: BatchLimits,
    covered: usize,
    prepared: usize,
    records: Vec<u8>,
    compared: usize,
    stage: BatchInspectionStatus,
    failed: bool,
    total: BatchInspectionWork,
}

impl FileJournal {
    /// Consume a usable locked owner and the independently retained original
    /// batch. Validate shape/profile before file I/O, then seek the same file.
    /// No payload is read or event applied until advance. Errors consume/drop
    /// the owner; they never return its stale accounting as usable recovery.
    /// Reopen a poisoned writer strictly before calling this method.
    pub fn begin_inspect_batch(self, anchor: BatchAnchor, events: Vec<Event>,
        limits: BatchLimits) -> Result<FileBatchInspection, Error>
    {
        self.begin_inspect_batch_checked(anchor, events, limits, None)
    }

    fn begin_inspect_batch_checked(mut self, anchor: BatchAnchor, events: Vec<Event>,
        limits: BatchLimits, history: Option<HistoryAnchor>) -> Result<FileBatchInspection, Error>
    {
        if self.poisoned { return Err(Error::Poisoned); }
        if anchor.identity != self.journal.identity()
            || anchor.encoded_len < self.journal.header().len() as u64
            || anchor.encoded_len > self.journal.encoded_len()
        { return Err(crate::Error::Binding.into()); }
        if events.is_empty() { return Err(crate::Error::InvalidInput.into()); }
        if limits.max_records == 0 || limits.max_records > MAX_BATCH_RECORDS
            || limits.max_encoded_bytes == 0 || limits.max_encoded_bytes > MAX_BATCH_ENCODED_BYTES
            || events.len() > limits.max_records
        { return Err(crate::Error::Limit.into()); }
        let covered = self.journal.head().sequence.checked_sub(anchor.head.sequence)
            .ok_or(crate::Error::BehindAnchor)?;
        let covered = usize::try_from(covered).map_err(|_| crate::Error::Limit)?;
        if covered > events.len() { return Err(crate::Error::Binding.into()); }
        let initial = Journal::configured(anchor.identity, self.journal.limits,
            self.journal.recovery_reserve())?;
        if let Some(history) = &history {
            history.check_context(&initial)?;
            if history.head() != anchor.head || history.encoded_len() != anchor.encoded_len {
                return Err(crate::Error::Binding.into());
            }
        }
        self.poisoned = true;
        if self.file.metadata()?.len() != self.journal.encoded_len() {
            return Err(extent_changed());
        }
        self.file.seek(SeekFrom::Start(0))?;
        let reader = self.file.take(anchor.encoded_len);
        let replay = match history {
            Some(history) => initial.replay_cursor_anchored(reader, history)?,
            None => initial.replay_cursor(reader, anchor.head)?,
        };
        Ok(FileBatchInspection { replay: Some(replay), file: None,
            acknowledged: self.journal, candidate: None, retain_candidate: false, anchor, events, limits, covered,
            prepared: 0, records: Vec::new(), compared: 0,
            stage: BatchInspectionStatus::ReplayPrefix, failed: false,
            total: BatchInspectionWork::default() })
    }
}

impl FileBatchInspection {
    /// Advance only the current stage, yielding before the next one. Every call
    /// respects its corresponding dimensions, including interrupted reads. Zero
    /// budgets do no reads or reducer attempts. All failures/unwinds latch Failed.
    pub fn advance(&mut self, budget: BatchInspectionBudget) -> Result<BatchInspectionReport, Error> {
        self.advance_with(budget, |file, bytes| file.read(bytes))
    }
    pub fn progress(&self) -> BatchInspectionReport {
        let mut total = self.total;
        if let Some(replay) = &self.replay { total.replay = replay.progress().total; }
        BatchInspectionReport { status: if self.failed { BatchInspectionStatus::Failed } else { self.stage }, total }
    }
    /// This is only the PREEXISTING acknowledged recovery minimum, not the
    /// private replay head or a claim that the supplied batch has completed.
    pub fn acknowledged_head(&self) -> Head { self.acknowledged.head() }
    pub fn cancel(self) -> BatchInspectionWork { self.progress().total }

    /// Return the same already synchronized owner and exact historical status.
    /// No write or synchronization occurs. Pending/failed inspection returns no
    /// owner; a final extent check still runs under the original advisory lock.
    /// Same-length edits by noncooperating writers remain outside that lock's
    /// guarantee, as in original FileRecovery. No host-security claim is added.
    pub fn finish(self) -> Result<(FileJournal, BatchStatus), Error> {
        if self.failed { return Err(Error::Poisoned); }
        if self.stage != BatchInspectionStatus::Ready { return Err(Error::RecoveryIncomplete); }
        let file = self.file.ok_or(Error::RecoveryIncomplete)?;
        if file.metadata()?.len() != self.acknowledged.encoded_len() { return Err(extent_changed()); }
        let status = BatchStatus { anchor: self.anchor, observed_head: self.acknowledged.head(),
            persisted_records: self.covered, total_records: self.events.len(),
            remaining_bytes: self.records.len() - self.compared };
        Ok((FileJournal { file, journal: self.acknowledged, poisoned: false }, status))
    }

    // Private fault seam only; no public reader callback or alternate data source.
    fn advance_with(&mut self, budget: BatchInspectionBudget,
        mut read: impl FnMut(&mut File, &mut [u8]) -> io::Result<usize>)
        -> Result<BatchInspectionReport, Error>
    {
        if self.failed { return Err(Error::Poisoned); }
        if self.stage == BatchInspectionStatus::Ready { return Ok(self.progress()); }
        self.failed = true;
        match self.stage {
            BatchInspectionStatus::ReplayPrefix => self.replay_prefix(budget.replay)?,
            BatchInspectionStatus::PrepareBatch => self.prepare(budget.prepare_records)?,
            BatchInspectionStatus::CompareSuffix => self.compare(budget, &mut read)?,
            _ => return Err(Error::RecoveryIncomplete),
        }
        self.failed = false;
        Ok(self.progress())
    }
    fn replay_prefix(&mut self, budget: ReplayBudget) -> Result<(), Error> {
        let replay = self.replay.as_mut().ok_or(Error::RecoveryIncomplete)?;
        let result = replay.advance(budget);
        self.total.replay = replay.progress().total;
        if result?.status != ReplayStatus::Ready { return Ok(()); }
        let (reader, before) = self.replay.take().ok_or(Error::RecoveryIncomplete)?.into_parts()?;
        self.file = Some(reader.into_inner());
        if before.head() != self.anchor.head || before.encoded_len() != self.anchor.encoded_len {
            return Err(crate::Error::Binding.into());
        }
        if self.covered == 0 { self.check_prefix(&before)?; }
        self.candidate = Some(before);
        self.stage = BatchInspectionStatus::PrepareBatch;
        Ok(())
    }
    fn check_prefix(&self, prefix: &Journal) -> Result<(), Error> {
        if prefix.head() != self.acknowledged.head()
            || prefix.encoded_len() != self.acknowledged.encoded_len()
            || prefix.rights() != self.acknowledged.rights()
        { return Err(crate::Error::Binding.into()); }
        Ok(())
    }
    fn prepare(&mut self, allowance: u64) -> Result<(), Error> {
        let mut spent = 0;
        while self.prepared < self.events.len() && spent < allowance {
            self.total.prepared_records = self.total.prepared_records.checked_add(1).ok_or(crate::Error::Limit)?;
            spent += 1;
            let remaining = self.limits.max_encoded_bytes.checked_sub(self.records.len()).ok_or(crate::Error::Limit)?;
            let limit = BatchLimits { max_records: 1, max_encoded_bytes: remaining };
            let (next, record) = self.candidate.as_ref().ok_or(Error::RecoveryIncomplete)?
                .prepare_batch(&self.events[self.prepared..self.prepared + 1], limit)?;
            self.records.try_reserve_exact(record.len()).map_err(|_| crate::Error::Limit)?;
            self.records.extend_from_slice(&record);
            self.prepared += 1;
            if self.prepared == self.covered { self.check_prefix(&next)?; }
            self.candidate = Some(next);
        }
        if self.prepared == self.events.len() {
            // Ordinary inspection retains no reconstructed ledger. Explicit
            // resumption keeps the final candidate private until full byte/EOF
            // verification transfers it into the original bounded writer.
            if !self.retain_candidate { self.candidate = None; }
            self.stage = BatchInspectionStatus::CompareSuffix;
        }
        Ok(())
    }
    fn compare(&mut self, budget: BatchInspectionBudget,
        read: &mut impl FnMut(&mut File, &mut [u8]) -> io::Result<usize>) -> Result<(), Error>
    {
        let suffix_len = usize::try_from(self.acknowledged.encoded_len() - self.anchor.encoded_len)
            .map_err(|_| crate::Error::Limit)?;
        if suffix_len > self.records.len() { return Err(crate::Error::Binding.into()); }
        let mut buffer = [0; 8192];
        let (mut calls, mut bytes) = (0, 0);
        while calls < budget.compare_calls && bytes < budget.compare_bytes {
            let eof = self.compared == suffix_len;
            let left = if eof { 1 } else { suffix_len - self.compared };
            let amount = (budget.compare_bytes - bytes).min(left.min(buffer.len()) as u64) as usize;
            self.total.compare_calls = self.total.compare_calls.checked_add(1).ok_or(crate::Error::Limit)?;
            calls += 1;
            let count = match read(self.file.as_mut().ok_or(Error::RecoveryIncomplete)?, &mut buffer[..amount]) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                other => other?,
            };
            if count > amount { return Err(crate::Error::Corrupt("reader overreported length").into()); }
            self.total.compare_bytes = self.total.compare_bytes.checked_add(count as u64).ok_or(crate::Error::Limit)?;
            bytes += count as u64;
            if eof && count == 0 {
                if self.file.as_ref().ok_or(Error::RecoveryIncomplete)?.metadata()?.len()
                    != self.acknowledged.encoded_len() { return Err(extent_changed()); }
                self.stage = BatchInspectionStatus::Ready;
                return Ok(());
            }
            if eof || count == 0 || buffer[..count] != self.records[self.compared..self.compared + count] {
                return Err(crate::Error::Binding.into());
            }
            self.compared += count;
        }
        Ok(())
    }
}
fn extent_changed() -> Error {
    io::Error::new(io::ErrorKind::InvalidData, "journal extent changed during batch inspection").into()
}

#[cfg(test)]
mod tests;
