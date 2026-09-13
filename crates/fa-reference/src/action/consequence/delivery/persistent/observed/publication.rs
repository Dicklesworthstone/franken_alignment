//! Fresh evidence at first publication, not another authorization or receipt.
//! This profile is enabled before any proposal and retained in the original
//! journal. Historical outcomes and original reconciliation always take priority.
use super::{Event, FileOversight, JournalError, Transition};
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::EndpointOutcome;
use crate::action::consequence::oversight::CommitteeInput;
use crate::{Error, Snapshot};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicationBasis {
    Revalidated,
    Rejected(Error),
    PreviouslyResolved,
    DeadlineElapsed,
}

/// Supervisor data after acknowledged canonical replacement. Even a sealed
/// nonexecution does not refund the broker here: use original reconciliation.
/// This copyable value cannot authorize another action or act as an endpoint key.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// use fa_reference::action::consequence::delivery::persistent::observed::publication::CheckedPublication;
/// fn authorize(result: CheckedPublication) -> FilePermit { result }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckedPublication {
    pub outcome: EndpointOutcome,
    pub basis: PublicationBasis,
}

impl FileOversight {
    /// Install once before any actor/operator proposal, including refused actor
    /// admissions. No disable, post-review upgrade or weaker publication fallback
    /// exists for this owner. Legacy owners remain explicitly unguarded.
    pub fn enable_publication_guard(&mut self, revision: u64) -> Result<(), JournalError> {
        self.transact(revision, Event::PublicationGuard)?;
        Ok(())
    }

    pub fn publication_guard_required(&self) -> bool { self.machine.publication_guard }

    /// Supply a freshly captured whole input and policy snapshot, then the trusted
    /// elapsed tick obtained AFTER capture. A missing/stale basis seals the
    /// original endpoint request; it cannot be retried into execution with better
    /// evidence. The endpoint's retained outcome wins over a later missing source.
    ///
    /// This can execute only an ORIGINAL envelope retained by this live owner.
    /// Reopening discards sendable envelopes and cannot reconstruct one through
    /// this API. Clock, source authenticity and coverage remain host assumptions.
    pub fn publish_checked(&mut self, revision: u64, attempt: u64,
        current: Option<&CommitteeInput>, snapshot: Snapshot, now: ElapsedTick)
        -> Result<CheckedPublication, JournalError>
    {
        if let Some(input) = current { self.check_action(attempt, input.action())?; }
        let supplied = current.map(|input| input.views().clone());
        match self.transact(revision, Event::PublishChecked(attempt, supplied, snapshot, now))? {
            Transition::PublicationChecked(result) => Ok(result),
            _ => unreachable!("checked publication transition"),
        }
    }
}
