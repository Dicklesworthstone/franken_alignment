//! Source-cut repair across one original receipt reconciliation, never a live
//! owner recovery or evidence of which request triggered the journal-wide sweep.
use super::super::super::{
    decoder::learned::{
        FileLearnedRecovery, FileLearnedRecoveryProgress, FileLearnedRecoveryStatus,
        FileReconciledLearnedReceipt,
    },
    driver::topology::checkpoint::TopologyCheckpoint,
    stream::FileStreamProposal,
};
use super::super::{FileRecoveryFloor, learned_recovery::check_recovery_reserve};
use super::{
    FileTopologyRequirement, JournalError, learned::FileMediatedLearnedRecovery,
    predictive::FilePredictiveMediatedLearnedRecovery,
};
use crate::action::consequence::delivery::persistent::RecoveryReserve;
use crate::{Error, action::ElapsedTick};
use std::fmt;

enum Inner {
    Mediated(FileMediatedLearnedRecovery),
    Predictive(FilePredictiveMediatedLearnedRecovery),
}

/// The original independently guarded learned replay, restricted to one retained
/// source checkpoint and one selected historical message or finish. Only an
/// unchanged cut or exactly one Fence/Time/Sweep may follow the retained prefix.
/// A matching triplet is confirmed without a clock, appended event or journal
/// replacement. Confirmation still synchronizes canonical storage and cleans
/// verified staging. An unchanged cut performs the original one reconciliation.
///
/// This repairs SOURCE CUSTODY, not the identity of an earlier command: Sweep
/// does not record which request caused it. The result projects the selected
/// original receipt at the verified cut without claiming it triggered the sweep.
/// No source is read again, work allowance refreshed, or unknown effect resent.
/// The checkpoint and independent anchor remain trusted custody, not protection
/// against their joint rollback or proof of external archive authentication.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::receipt::FileReceiptTopologyRecovery;
/// fn owner(run: FileReceiptTopologyRecovery) { run.finish(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::receipt::FileReceiptTopologyRecovery;
/// fn escape(run: FileReceiptTopologyRecovery) { run.into_inner(); }
/// ```
#[must_use = "complete the selected receipt custody or drop without changing storage"]
pub struct FileReceiptTopologyRecovery {
    inner: Inner,
    saved: TopologyCheckpoint,
    request: u64,
    finish_after: Option<u64>,
    reconciled: bool,
}

impl fmt::Debug for FileReceiptTopologyRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileReceiptTopologyRecovery")
            .field("request", &self.request)
            .field("progress", &self.progress())
            .finish_non_exhaustive()
    }
}

// Both original wrappers call this before any replay. Change ONLY the topology
// availability explained by the fixed source-free suffix. Every independently
// supplied floor is retained or strengthened; no weaker exact-cut path appears.
pub(super) fn prepare(
    inner: &FileLearnedRecovery,
    progress: FileLearnedRecoveryProgress,
    topology: &mut FileTopologyRequirement,
    minimum: &mut FileRecoveryFloor,
    saved: &TopologyCheckpoint,
) -> Result<bool, JournalError> {
    if progress.replayed_events != 0 || progress.status != FileLearnedRecoveryStatus::Replaying {
        return Err(Error::WrongState.into());
    }
    check_recovery_reserve(inner, RecoveryReserve::terminal())?;
    if &topology.current != saved.expected().graph() || topology.available != saved.available() {
        return Err(Error::Binding.into());
    }
    let (profile, events, identity) = inner.guarded_history();
    let available = saved.reconciled_successor_available(profile, identity, events)?;
    let reconciled = events.len() as u64 != saved.history_anchor().revision();
    let floor = saved.minimum();
    minimum.journal_revision = minimum.journal_revision.max(floor.journal_revision);
    minimum.control_sequence = minimum.control_sequence.max(floor.control_sequence);
    minimum.authority_epoch = minimum.authority_epoch.max(floor.authority_epoch);
    topology.available = available;
    Ok(reconciled)
}

impl FileReceiptTopologyRecovery {
    pub(super) fn mediated(
        inner: FileMediatedLearnedRecovery,
        saved: &TopologyCheckpoint,
        request: u64,
        finish_after: Option<u64>,
        reconciled: bool,
    ) -> Self {
        Self {
            inner: Inner::Mediated(inner),
            saved: saved.clone(),
            request,
            finish_after,
            reconciled,
        }
    }
    pub(super) fn predictive(
        inner: FilePredictiveMediatedLearnedRecovery,
        saved: &TopologyCheckpoint,
        request: u64,
        finish_after: Option<u64>,
        reconciled: bool,
    ) -> Self {
        Self {
            inner: Inner::Predictive(inner),
            saved: saved.clone(),
            request,
            finish_after,
            reconciled,
        }
    }
    pub fn progress(&self) -> FileLearnedRecoveryProgress {
        match &self.inner {
            Inner::Mediated(run) => run.progress(),
            Inner::Predictive(run) => run.progress(),
        }
    }
    /// Replay the original bounded event quantum under all original guards.
    /// Ready confirms historical validity only; no live authority is produced.
    pub fn advance(
        &mut self,
        expected_events: usize,
        max_events: usize,
    ) -> Result<FileLearnedRecoveryProgress, JournalError> {
        match &mut self.inner {
            Inner::Mediated(run) => run.advance(expected_events, max_events),
            Inner::Predictive(run) => run.advance(expected_events, max_events),
        }
    }
    /// Require the selected message or exact finish predecessor only after the
    /// entire original predictor, policy, current epoch, topology and floors pass.
    pub fn check_request(&self) -> Result<(), JournalError> {
        self.selected_request().map(|_| ())
    }
    // Only the sealed actor-port composition uses the descriptor. It freezes
    // the complete source-only envelope BEFORE confirmation or new settlement.
    pub(crate) fn selected_request(
        &self,
    ) -> Result<(u64, Option<u64>, FileStreamProposal), JournalError> {
        let original = match (&self.inner, self.finish_after) {
            (Inner::Mediated(run), None) => {
                run.learned_text_stream_message_request(self.request)?
            }
            (Inner::Predictive(run), None) => {
                run.learned_text_stream_message_request(self.request)?
            }
            (Inner::Mediated(run), Some(after)) => {
                run.learned_text_stream_finish_request(after, self.request)?
            }
            (Inner::Predictive(run), Some(after)) => {
                run.learned_text_stream_finish_request(after, self.request)?
            }
        };
        Ok((self.request, self.finish_after, original))
    }
    /// Confirm the exact already visible settlement, or complete the original
    /// single Fence/Time/Sweep if the saved cut is still current. No repeated
    /// numerical replay, new effect, owner or approval role escapes either path.
    /// Canonical changes and sync/cleanup failures return no candidate custody.
    pub fn finish_reconciled_receipt_with_history<F>(
        self,
        clock: F,
    ) -> Result<FileReconciledLearnedReceipt, JournalError>
    where
        F: FnOnce() -> ElapsedTick,
    {
        self.check_request()?;
        let Self {
            inner,
            saved,
            request,
            finish_after,
            reconciled,
        } = self;
        match (inner, reconciled) {
            (Inner::Mediated(run), true) => {
                run.confirm_reconciled_receipt_with_history(request, finish_after, &saved)
            }
            (Inner::Predictive(run), true) => {
                run.confirm_reconciled_receipt_with_history(request, finish_after, &saved)
            }
            (Inner::Mediated(run), false) => {
                run.finish_reconciled_receipt_with_checkpoint(request, finish_after, &saved, clock)
            }
            (Inner::Predictive(run), false) => {
                run.finish_reconciled_receipt_with_checkpoint(request, finish_after, &saved, clock)
            }
        }
    }
}
