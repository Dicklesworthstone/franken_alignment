//! Bind the original batch inspector to an independently retained exact prefix.
use super::{BatchAnchor, BatchLimits, Error, Event, FileBatchInspection, FileJournal};
use crate::replay::HistoryAnchor;

impl FileJournal {
    /// Inspect an interrupted group using its EXACT independently retained
    /// pre-group history, not only its sequence, length and accidental-damage CRC.
    /// The original events must also be retained independently; the archive
    /// cannot authenticate unwritten intentions or establish external outcomes.
    ///
    /// Consume the same already recovered owner. Identity, limits, reserve and
    /// the exact pre-group boundary are checked before metadata or seeking.
    /// Prefix bytes are compared inside the original budgeted replay BEFORE
    /// mismatching bytes can reach its decoder or reducer. All original group
    /// preparation, stored-suffix, EOF, lock and ownership checks remain in force.
    /// No second read pass or alternative decoder is introduced by anchoring.
    ///
    /// finish() returns only the existing owner and historical BatchStatus.
    /// Read-only selection cannot later be upgraded to resumption. Capture and
    /// retain history_anchor() BEFORE attempting the group, not from its possibly
    /// advanced recovery state. No history or profile is inferred from the file.
    /// Memory additionally includes the immutable prefix, shared across clones;
    /// independent anchor import and earlier strict reopen work are separate.
    pub fn begin_inspect_batch_anchored(
        self,
        history: HistoryAnchor,
        events: Vec<Event>,
        limits: BatchLimits,
    ) -> Result<FileBatchInspection, Error> {
        let anchor = BatchAnchor {
            identity: self.journal.identity(),
            head: history.head(),
            encoded_len: history.encoded_len(),
        };
        self.begin_inspect_batch_checked(anchor, events, limits, Some(history))
    }

    /// Select exact-history resumption BEFORE any inspection. A Ready inspector
    /// transfers the same locked descriptor and encoded missing suffix through
    /// resume() to the original FileBatchCommit. No intermediate writer, extra
    /// replay, re-encoding, fresh group, external dispatch or inferred refund.
    /// A poisoned/torn file still requires the original strict recovery first.
    /// Nonempty missing suffixes still synchronize before new accounting escapes;
    /// an already acknowledged complete group needs no new write or sync.
    pub fn begin_resume_batch_anchored(
        self,
        history: HistoryAnchor,
        events: Vec<Event>,
        limits: BatchLimits,
    ) -> Result<FileBatchInspection, Error> {
        let mut inspection = self.begin_inspect_batch_anchored(history, events, limits)?;
        inspection.retain_candidate = true;
        Ok(inspection)
    }
}
