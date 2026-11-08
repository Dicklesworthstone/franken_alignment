//! Finish the original Fence/Time/Sweep without dropping mediated-role custody.
use super::super::ReconciledGuardedOwner;
use super::{FileMediatedRecovery, FileMediatedRoles, FileOversight, JournalError, check_machine};
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::observed::{
    driver::topology::checkpoint::TopologyCheckpoint, guarded::FileOversightRoles,
};
use crate::action::consequence::delivery::persistent::requests::PendingReconciliations;

/// Original acknowledged owner, separate roles, and EACH native sweep outcome.
/// Returning an owner does not mean all liabilities are resolved. Fresh clock
/// evidence is not fresh topology, prediction, evaluation or permission to send.
#[must_use = "retain original reconciliation outcomes and separate mediated roles"]
pub struct ReconciledMediatedOwner {
    pub owner: FileOversight,
    pub roles: FileMediatedRoles,
    pub outcomes: PendingReconciliations,
}

impl FileMediatedRecovery {
    /// Carry exact independently retained topology-source custody through the
    /// ORIGINAL acknowledged Fence/Time/Sweep. The complete mediated cursor
    /// validates the saved source cut before the clock, cleanup or mutation.
    /// Older source work, a different owner/profile/prefix, mismatched graph or
    /// availability, and decreasing native floors refuse without a new fence.
    ///
    /// All fallible source-document copies are prepared before mutation. The
    /// returned checkpoint takes the exact encoded acknowledged history from
    /// the original replacement while the SAME exclusive owner is still held.
    /// No source read, source check, operation, helper, resend or second replay
    /// occurs. Spent work and last operation stay unchanged; availability is
    /// withdrawn by the native Fence. Every reconciliation outcome still needs
    /// inspection before new work. Independent archive export remains separate.
    ///
    /// Failed or ambiguous persistence yields neither an owner nor a checkpoint.
    /// Ordinary three-event capacity and the existing reserve are unchanged.
    pub fn finish_reconciled_with_checkpoint<F>(
        self,
        saved: &TopologyCheckpoint,
        clock: F,
    ) -> Result<(ReconciledMediatedOwner, TopologyCheckpoint), JournalError>
    where
        F: FnOnce() -> ElapsedTick,
    {
        let expected = self.expected;
        let mut prepared = None;
        let recovered = self.inner.finish_reconciled_history_checked(clock, |run| {
            check_machine(&run.profile, &run.machine, &run.events, &expected)?;
            prepared = Some(saved.prepare_reconciled_start(
                &run.profile,
                run.store.identity(),
                &run.events,
                &run.machine.snapshot(run.events.len()),
            )?);
            Ok(())
        })?;
        let checkpoint = prepared
            .expect("complete original checkpoint preflight")
            .acknowledge_reconciled(recovered.anchor, &recovered.owner);
        let oversight = FileOversightRoles::provision(&recovered.owner, recovered.human);
        let roles = FileMediatedRoles::provision(
            &recovered.owner,
            oversight,
            expected.prediction.is_some(),
            expected.evaluation.is_some(),
        );
        Ok((
            ReconciledMediatedOwner {
                owner: recovered.owner,
                roles,
                outcomes: recovered.outcomes,
            },
            checkpoint,
        ))
    }

    /// Consume complete mediated replay into the ORIGINAL atomic
    /// Fence -> Time -> Sweep replacement. No second replay, intermediate live
    /// owner, source read, helper call, new approval or effect resend occurs.
    ///
    /// The complete mediated requirements are rechecked before the single clock
    /// call. Original capacity checks and both canonical-image comparisons remain
    /// in force. A failing/unwinding clock cannot spend a fence or clean staging.
    /// Unacknowledged persistence returns neither candidate outcomes nor roles.
    ///
    /// This is live recovery: all three events need ordinary capacity. It cannot
    /// consume the terminal-only stop reserve or refill any budget. The returned
    /// source/observer eligibility stays withdrawn by the original fence; inspect
    /// every outcome before deciding whether an earlier liability is settled.
    pub fn finish_reconciled<F>(self, clock: F) -> Result<ReconciledMediatedOwner, JournalError>
    where F: FnOnce() -> ElapsedTick {
        let expected = self.expected;
        let ReconciledGuardedOwner { owner, roles, outcomes } = self.inner.finish_reconciled_checked(clock,
            |run| check_machine(&run.profile, &run.machine, &run.events, &expected))?;
        let roles = FileMediatedRoles::provision(&owner, roles,
            expected.prediction.is_some(), expected.evaluation.is_some());
        Ok(ReconciledMediatedOwner { owner, roles, outcomes })
    }
}

#[cfg(test)]
#[path = "reconciled_tests.rs"]
mod tests;
