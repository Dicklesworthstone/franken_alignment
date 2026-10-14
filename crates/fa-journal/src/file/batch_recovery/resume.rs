//! Complete only the missing records of an independently retained original group.
//! Verification, accounting and framing remain in the original journal owners.
use super::{BatchAnchor, BatchLimits, Error, Event, FileJournal, Head};
use std::fs::File;
use std::io::{self, Seek, SeekFrom, Write};

impl FileJournal {
    /// Complete an interrupted accounting group after strict original recovery.
    /// Retain the PRE-group anchor and the entire ordered batch independently;
    /// reopen a poisoned writer before calling this method. A torn record still
    /// refuses at reopen: this operation never truncates or repairs journal data.
    ///
    /// The original inspector first verifies the exact anchored prefix, every
    /// already stored group byte, the current rights/head/extent, and ALL events
    /// and limits of the original group. Only then are the missing records
    /// encoded from the acknowledged state and appended under the SAME lock.
    /// One final file sync precedes the new head or usable accounting. A read,
    /// preparation or I/O failure after verification begins leaves quarantine
    /// in force, including unwinding; the old head remains a recovery minimum.
    ///
    /// When the complete group is already acknowledged, exact verification
    /// returns that head without writing or syncing again. An unrelated later
    /// event is a conflict, not a completed retry. Matching only the stored
    /// prefix cannot authenticate the supplied, previously unwritten suffix.
    ///
    /// These are rights-journal records, NOT external sends or remote outcome
    /// discovery. This method cannot determine whether an external effect ran,
    /// authorize its retry, or manufacture nonexecution evidence. All original
    /// epoch, effect-identity, unknown-liability and reserved-tail laws apply.
    /// Work is synchronous prefix replay plus bounded batch preparation and I/O;
    /// no constant-time, nonblocking or multi-record disk-atomicity claim follows.
    pub fn resume_batch(&mut self, anchor: BatchAnchor, events: &[Event], limits: BatchLimits)
        -> Result<Head, Error>
    {
        self.resume_batch_with(anchor, events, limits, |file, bytes| {
            file.write_all(bytes)?;
            file.sync_all()
        })
    }

    // Fault seam only. Public callers cannot supply a persistence callback,
    // replacement ledger, progress assertion or forged successful inspection.
    fn resume_batch_with(&mut self, anchor: BatchAnchor, events: &[Event], limits: BatchLimits,
        persist: impl FnOnce(&mut File, &[u8]) -> io::Result<()>) -> Result<Head, Error>
    {
        // The original verifier latches poison BEFORE reads. Do not use the
        // public inspector here: it clears quarantine on successful inspection.
        let status = self.verify_batch(anchor, events, limits)?;
        if status.is_complete() {
            // This owner already acknowledged the full group, either at its
            // original commit or through strict sync-before-return recovery.
            self.poisoned = false;
            return Ok(self.journal.head());
        }
        let remaining = events.get(status.persisted_records()..).ok_or(crate::Error::Binding)?;
        // Whole-group ceilings were checked above, including the stored prefix.
        // Re-encoding only missing events starts at the verified current rights,
        // sequence and CRC. Never reapply stored records to this candidate.
        let (candidate, bytes) = self.journal.prepare_batch(remaining, limits)?;
        if bytes.len() != status.remaining_bytes() { return Err(crate::Error::Binding.into()); }
        let acknowledged_len = self.journal.encoded_len();
        if self.file.metadata()?.len() != acknowledged_len
            || self.file.seek(SeekFrom::End(0))? != acknowledged_len
        { return Err(extent_changed()); }
        persist(&mut self.file, &bytes)?;
        if self.file.metadata()?.len() != candidate.encoded_len() {
            return Err(extent_changed());
        }
        self.journal = candidate;
        self.poisoned = false;
        Ok(self.journal.head())
    }
}

fn extent_changed() -> Error {
    io::Error::new(io::ErrorKind::InvalidData, "journal extent changed during batch resumption").into()
}

#[cfg(test)]
mod tests;
