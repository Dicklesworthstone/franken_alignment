//! Transfer one fully verified original group directly to the original writer.
//! No public candidate import, second prefix replay, or external effect retry.
use super::{BatchAnchor, BatchInspectionStatus, BatchLimits, Error, Event,
    FileBatchInspection, FileJournal, extent_changed};
use crate::file::FileBatchCommit;
use std::fs::File;
use std::io;

impl FileJournal {
    /// Select cooperative resumption BEFORE inspection starts. Retain the exact
    /// original pre-group anchor and complete ordered events independently. A
    /// poisoned or torn journal must first pass the original strict reopen.
    ///
    /// Advance the returned inspector with BatchInspectionBudget, then consume
    /// its Ready result through resume(). This uses the original prefix replay,
    /// whole-group preparation and exact stored-suffix/EOF comparison once. No
    /// write occurs during inspection or handoff. The original lifetime limits
    /// and reserved-tail profile remain fixed across both stages.
    ///
    /// Compared with ordinary inspection, the final private candidate remains
    /// retained until handoff/drop. The original per-record preparation still
    /// clones its private ledger. This is bounded logical scheduling, not a
    /// memory reduction, physical-work receipt or latency guarantee.
    pub fn begin_resume_batch(self, anchor: BatchAnchor, events: Vec<Event>, limits: BatchLimits)
        -> Result<FileBatchInspection, Error>
    {
        let mut inspection = self.begin_inspect_batch(anchor, events, limits)?;
        inspection.retain_candidate = true;
        Ok(inspection)
    }
}

impl FileBatchInspection {
    /// Transfer the SAME locked file and fully prepared candidate into the
    /// original bounded writer, without replaying history or encoding again.
    /// Only explicit resumption or direct anchored batch recovery selects this
    /// path. Ordinary read-only inspection cannot upgrade after observing a result.
    ///
    /// Every original record and every stored byte must already have verified,
    /// including EOF. The consumed inspector returns no owner on refusal.
    /// Its completed work is in progress(); retain that report before handoff.
    /// The returned writer reports only MISSING records, zero new preparation
    /// attempts, and its own subsequent write work. No counters are refunded.
    ///
    /// Removing the already stored prefix can move at most the bounded group
    /// buffer in memory. File extent is checked under the uninterrupted original
    /// advisory lock; malicious same-size writes remain outside its contract.
    /// A complete acknowledged group yields a Ready writer whose finish needs
    /// neither write nor sync. Otherwise finish must sync before exposing the
    /// new head/accounting. Direct recovery MUST first synchronize the verified
    /// stored prefix and its parent, including a complete group, before this
    /// handoff can use the original already-acknowledged shortcut. No source
    /// bytes are reread. Dropping either stage never truncates or repairs.
    ///
    /// ```compile_fail,E0382
    /// use fa_journal::file::FileBatchInspection;
    /// fn one_handoff(run: FileBatchInspection) {
    ///     let _ = run.resume();
    ///     let _ = run.resume();
    /// }
    /// ```
    pub fn resume(self) -> Result<FileBatchCommit, Error> {
        self.resume_with_sync(|file, directory| {
            file.sync_all()?;
            directory.sync_all()
        })
    }

    pub(super) fn resume_with_sync(mut self,
        sync: impl FnOnce(&File, &File) -> io::Result<()>) -> Result<FileBatchCommit, Error>
    {
        if self.failed { return Err(Error::Poisoned); }
        if !self.retain_candidate { return Err(crate::Error::InvalidInput.into()); }
        if self.stage != BatchInspectionStatus::Ready { return Err(Error::RecoveryIncomplete); }
        let candidate = self.candidate.take().ok_or(Error::RecoveryIncomplete)?;
        let file = self.file.take().ok_or(Error::RecoveryIncomplete)?;
        if file.metadata()?.len() != self.origin.current()?.encoded_len() { return Err(extent_changed()); }
        if self.prepared != self.events.len()
            || self.origin.current()?.encoded_len().checked_sub(self.anchor.encoded_len)
                != Some(self.compared as u64)
            || self.compared > self.records.len()
        { return Err(crate::Error::Binding.into()); }
        let missing = self.events.len().checked_sub(self.covered).ok_or(crate::Error::Binding)?;
        self.origin.synchronize(&file, sync)?;
        // The suffix was encoded against the same preceding CRC, sequence and
        // rights. Keep its exact bytes; do not import an asserted progress flag.
        drop(self.records.drain(..self.compared));
        let owner = FileJournal { file, journal: self.origin.into_current()?, poisoned: false };
        FileBatchCommit::from_verified_suffix(owner, candidate, self.records, missing)
    }
}
