//! Finish the original Fence/Time/Sweep without dropping mediated-role custody.
use super::{FileMediatedRecovery, FileMediatedRoles, FileOversight, JournalError, check_machine};
use super::super::ReconciledGuardedOwner;
use crate::action::ElapsedTick;
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
