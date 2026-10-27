//! Acknowledged learned receipt custody without a live owner or approval role.
use super::super::super::super::{
    driver::topology::checkpoint::TopologyCheckpoint, guarded::anchored::FileHistoryAnchor,
};
use super::{FileLearnedRecovery, FileOversight, JournalError};
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::FileDeliverySnapshot;
use crate::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use std::fmt;

/// One original fixed Fence/Time/Sweep replacement and its selected actor
/// observation. The original exclusive recovery lock captures the exact bytes;
/// the returned value holds no Store, observer, reviewer or live owner.
///
/// The history and snapshot are sensitive operator evidence. Only outcome()
/// supplies the existing redacted actor projection. External archive retention
/// is independent of acknowledgment and must not trigger another effect.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::FileReconciledLearnedReceipt;
/// fn owner(receipt: FileReconciledLearnedReceipt) { receipt.into_inner(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::FileReconciledLearnedReceipt;
/// fn publish(receipt: FileReconciledLearnedReceipt) { receipt.into_supervised_driver(); }
/// ```
#[derive(Clone, PartialEq, Eq)]
#[must_use = "retain acknowledged custody before discarding the recovery result"]
pub struct FileReconciledLearnedReceipt {
    outcome: Knowledge<ActorOutcome>,
    snapshot: FileDeliverySnapshot,
    anchor: FileHistoryAnchor,
    reconciled_from: u64,
}

impl fmt::Debug for FileReconciledLearnedReceipt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileReconciledLearnedReceipt")
            .field("reconciled_from", &self.reconciled_from)
            .field("revision", &self.anchor.revision())
            .finish_non_exhaustive()
    }
}

impl FileReconciledLearnedReceipt {
    pub fn outcome(&self) -> &Knowledge<ActorOutcome> {
        &self.outcome
    }
    pub fn snapshot(&self) -> &FileDeliverySnapshot {
        &self.snapshot
    }
    pub fn history_anchor(&self) -> &FileHistoryAnchor {
        &self.anchor
    }
    /// Exact complete source history BEFORE this one reconciliation transaction.
    pub fn reconciled_from(&self) -> u64 {
        self.reconciled_from
    }
    pub fn into_parts(self) -> (Knowledge<ActorOutcome>, FileHistoryAnchor) {
        (self.outcome, self.anchor)
    }
}

impl FileLearnedRecovery {
    /// Perform the original selected reconciliation and retain its acknowledged
    /// canonical bytes. Selection, full replay, clock ordering, ordinary capacity,
    /// canonical rechecks, cleanup and the single replacement remain unchanged.
    /// The original engine moves the encoded replacement into the anchor after
    /// acknowledgment; no second numerical replay or post-commit encoding occurs.
    /// A failed/ambiguous replacement returns no candidate result.
    pub fn finish_reconciled_receipt_with_history<F>(
        self,
        request: u64,
        finish_after: Option<u64>,
        clock: F,
    ) -> Result<FileReconciledLearnedReceipt, JournalError>
    where
        F: FnOnce() -> ElapsedTick,
    {
        match finish_after {
            None => {
                self.learned_text_stream_message_request(request)?;
            }
            Some(after) => {
                self.learned_text_stream_finish_request(after, request)?;
            }
        }
        let reconciled_from = self.events.len() as u64;
        let Self {
            profile,
            store,
            events,
            machine,
            canonical,
            ..
        } = self;
        let recovered = FileOversight::finish_reconciled_history(
            profile, store, events, machine, canonical, clock,
        )?;
        // Request records are never removed or reassigned by the native sweep.
        // This is the SAME selected projection used by the original finalizer.
        let outcome = recovered.owner.request_status(request)?.actor_observation();
        Ok(FileReconciledLearnedReceipt {
            outcome,
            snapshot: recovered.owner.inspect(),
            anchor: recovered.anchor,
            reconciled_from,
        })
    }

    /// Require exact independently retained source custody BEFORE the trusted
    /// clock, cleanup or replacement. It cannot borrow an older checkpoint across
    /// ordinary or source work. The result retains the acknowledged history even
    /// if a later independent source/archive export fails.
    pub fn finish_reconciled_receipt_with_checkpoint<F>(
        self,
        request: u64,
        finish_after: Option<u64>,
        saved: &TopologyCheckpoint,
        clock: F,
    ) -> Result<FileReconciledLearnedReceipt, JournalError>
    where
        F: FnOnce() -> ElapsedTick,
    {
        let machine = self.verified_guarded_machine()?;
        saved.check_reconciled_start(
            &self.profile,
            self.store.identity(),
            &self.events,
            &machine.snapshot(self.events.len()),
        )?;
        self.finish_reconciled_receipt_with_history(request, finish_after, clock)
    }
}
