//! Compare an independently retained group with the ORIGINAL locked journal.
//! Prefix reconstructions stay private; no partial ledger or effect key escapes.
mod cooperative;
mod resume;
pub use cooperative::{BatchInspectionBudget, BatchInspectionReport, BatchInspectionStatus,
    BatchInspectionWork, FileBatchInspection};

use super::{Error, FileJournal};
use crate::{Event, Head, Identity, Journal};
use crate::batch::{BatchLimits, MAX_BATCH_RECORDS};
use std::io::{self, Read, Seek, SeekFrom};

/// Capture BEFORE the original group attempt and retain independently alongside
/// its exact ordered events. Fields are data for the caller's recovery context,
/// not an authenticator or permission to issue/reissue an external effect.
/// The offset is checked against full original replay, never trusted as a cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BatchAnchor {
    pub identity: Identity,
    pub head: Head,
    pub encoded_len: u64,
}

/// Exact matching records in this acknowledged local file, not external outcomes.
/// `persisted_records` is relative to the supplied batch, not a journal sequence.
/// An incomplete group cannot acquire the final group's acknowledgment here.
///
/// ```compile_fail,E0308
/// use fa_journal::file::{BatchStatus, FileJournal};
/// fn writer(status: BatchStatus) -> FileJournal { status }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BatchStatus {
    anchor: BatchAnchor,
    observed_head: Head,
    persisted_records: usize,
    total_records: usize,
    remaining_bytes: usize,
}
impl BatchStatus {
    pub fn anchor(&self) -> BatchAnchor { self.anchor }
    pub fn observed_head(&self) -> Head { self.observed_head }
    pub fn persisted_records(&self) -> usize { self.persisted_records }
    pub fn remaining_records(&self) -> usize { self.total_records - self.persisted_records }
    pub fn remaining_bytes(&self) -> usize { self.remaining_bytes }
    pub fn is_complete(&self) -> bool { self.persisted_records == self.total_records }
}

impl FileJournal {
    /// Last acknowledged pre-group boundary. A poisoned owner's older head is
    /// still a recovery minimum, but cannot establish a new batch origin.
    pub fn batch_anchor(&self) -> Result<BatchAnchor, Error> {
        if self.poisoned { return Err(Error::Poisoned); }
        Ok(BatchAnchor { identity: self.journal.identity(), head: self.journal.head(),
            encoded_len: self.journal.encoded_len() })
    }

    /// After strict reopen, determine exactly how much of a supplied original
    /// batch survived an uncertain commit. This replays the complete anchored
    /// prefix privately, validates the WHOLE proposed group with original limits
    /// and reducers, and compares every persisted suffix byte. No write, repair,
    /// sync, truncation, effect execution or fresh authority occurs.
    ///
    /// The same writer lock covers inspection. A later unrelated record or a
    /// differing persisted record is a conflict, not permission to overwrite it.
    /// Offsets ending inside a record cannot manufacture a valid earlier state.
    /// Once reads begin, any error/unwind quarantines the owner until reopened;
    /// parameter refusals before reads leave it usable. No unverified private
    /// prefix replaces its last acknowledged head or accounting.
    ///
    /// The caller MUST retain the original batch independently. Unwritten events
    /// are not in the journal: matching its prefix cannot authenticate those
    /// supplied events or establish that an external operation did not occur.
    /// Work includes one original prefix replay and bounded group preparation;
    /// this is not a constant-time lookup, physical-work cap or remote receipt.
    pub fn inspect_batch(&mut self, anchor: BatchAnchor, events: &[Event], limits: BatchLimits)
        -> Result<BatchStatus, Error>
    {
        let status = self.verify_batch(anchor, events, limits)?;
        self.poisoned = false;
        Ok(status)
    }

    // Keep the original reducer and encoder as sole authorities. Only a completely
    // checked result is returned, and the caller must explicitly clear quarantine.
    fn verify_batch(&mut self, anchor: BatchAnchor, events: &[Event], limits: BatchLimits)
        -> Result<BatchStatus, Error>
    {
        if self.poisoned { return Err(Error::Poisoned); }
        if anchor.identity != self.journal.identity()
            || anchor.encoded_len < self.journal.header().len() as u64
            || anchor.encoded_len > self.journal.encoded_len()
        { return Err(crate::Error::Binding.into()); }
        if events.is_empty() { return Err(crate::Error::InvalidInput.into()); }
        if events.len() > MAX_BATCH_RECORDS { return Err(crate::Error::Limit.into()); }
        let covered = self.journal.head().sequence.checked_sub(anchor.head.sequence)
            .ok_or(crate::Error::BehindAnchor)?;
        let covered = usize::try_from(covered).map_err(|_| crate::Error::Limit)?;
        if covered > events.len() { return Err(crate::Error::Binding.into()); }

        self.poisoned = true;
        let actual_len = self.file.metadata()?.len();
        if actual_len != self.journal.encoded_len() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "journal extent changed").into());
        }
        self.file.seek(SeekFrom::Start(0))?;
        // Take is a *bounded private input*, not acceptance of arbitrary prefix
        // accounting. Require its exact head/extent and compare the entire rest
        // against this owner's already acknowledged state before any result.
        let reader = (&mut self.file).take(anchor.encoded_len);
        // The existing owner already pins the profile independently. Preserve
        // it; never autodetect a reserve or downgrade its header from disk bytes.
        let before = match self.journal.recovery_reserve() {
            Some(reserve) => Journal::replay_with_recovery_reserve(reader,
                anchor.identity, anchor.head, self.journal.limits, reserve),
            None => Journal::replay(reader, anchor.identity, anchor.head, self.journal.limits),
        }?;
        if before.head() != anchor.head || before.encoded_len() != anchor.encoded_len {
            return Err(crate::Error::Binding.into());
        }
        let (_, records) = before.prepare_batch(events, limits)?;
        let prefix = if covered == 0 { before } else {
            before.prepare_batch(&events[..covered], limits)?.0
        };
        if prefix.head() != self.journal.head()
            || prefix.encoded_len() != self.journal.encoded_len()
            || prefix.rights() != self.journal.rights()
        { return Err(crate::Error::Binding.into()); }
        let suffix_len = usize::try_from(actual_len - anchor.encoded_len)
            .map_err(|_| crate::Error::Limit)?;
        let expected = records.get(..suffix_len).ok_or(crate::Error::Binding)?;
        // Check full bytes, not just the CRC head. No clone of the complete log.
        let mut buffer = [0; 8192];
        for chunk in expected.chunks(buffer.len()) {
            self.file.read_exact(&mut buffer[..chunk.len()])?;
            if &buffer[..chunk.len()] != chunk { return Err(crate::Error::Binding.into()); }
        }
        let mut end = [0];
        loop {
            match self.file.read(&mut end) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Ok(0) => break,
                Ok(_) => return Err(crate::Error::Binding.into()),
                Err(error) => return Err(error.into()),
            }
        }
        if self.file.metadata()?.len() != actual_len {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "journal extent changed during inspection").into());
        }
        Ok(BatchStatus { anchor, observed_head: self.journal.head(), persisted_records: covered,
            total_records: events.len(), remaining_bytes: records.len() - suffix_len })
    }
}

#[cfg(test)]
mod tests;
