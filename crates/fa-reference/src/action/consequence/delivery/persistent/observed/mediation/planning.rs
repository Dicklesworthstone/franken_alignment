//! Select placement, then persist the ORIGINAL independently checked cut.
//! This is a supervisor integration, not an optimizer-owned permission path.
mod cooperative;
pub use cooperative::{FileMediationPlanning, FileMediationPlanningProgress, TopologyPlanningTask};

use super::{FileMediationObserver, FileMediationUpdate, FileOversight, JournalError};
use crate::action::consequence::delivery::TopologyChange;
use crate::action::consequence::mediation::{CutCheck, MAX_CHECK_EDGE_VISITS};
use crate::action::consequence::mediation::planning::{CutPlan, CutPlanningLimits, CutPlanningStatus, EnforcerCost};
use crate::Error;
#[cfg(test)]
use crate::action::consequence::mediation::planning::CutPlanOutcome;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MediationPlanningBudget {
    pub planning: CutPlanningLimits,
    pub verification_edge_visits: usize,
}
impl Default for MediationPlanningBudget {
    fn default() -> Self {
        Self { planning: CutPlanningLimits::default(), verification_edge_visits: MAX_CHECK_EDGE_VISITS }
    }
}

/// Acknowledged ORIGINAL check result, including its actual refusal. Cost and
/// solver work are advisory observations, not authoritative journal fields.
/// Replay independently rechecks the retained gates/partition; it does not
/// certify cost optimality or reconstruct a claim that the solver was executed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediationPlanReceipt {
    pub journal_revision: u64,
    pub plan: CutPlan,
    pub checked: Result<CutCheck, Error>,
}

/// The topology transition was already acknowledged, even when subsequent
/// planning/certification fails. Never treat an inner error as rollback of that
/// revocation. An outer error means the update was not acknowledged by this call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopologyPlanningReceipt {
    pub change: Option<TopologyChange>,
    pub certification: Result<MediationPlanReceipt, JournalError>,
}

impl FileMediationObserver {
    /// Choose a costed cut of the CURRENT graph and submit its plain proposal to
    /// the original durable verifier. Both predecessor checks happen before
    /// planning. No caller can pass a claimed VerifiedCut or substitute a graph.
    ///
    /// A returned candidate is not success: inspect `checked`. Missing solver
    /// capacity gives no check result; insufficient verifier capacity is recorded
    /// by the original checker. Neither can manufacture a certificate. A rejected
    /// advisory plan does not withdraw an earlier valid cut of the SAME immutable
    /// graph. Actual topology changes use update_and_plan instead.
    pub fn plan_and_certify(&self, host: &mut FileOversight, revision: u64, epoch: u64,
        costs: &[EnforcerCost], budget: MediationPlanningBudget)
        -> Result<MediationPlanReceipt, JournalError>
    {
        let mut run = self.begin_plan_and_certify(host, revision, epoch, costs, budget)?;
        while run.progress().solver.status == CutPlanningStatus::Planning {
            run.advance(host, revision, usize::MAX)?;
        }
        run.certify(self, host, revision)
    }

    /// A new topology is evidence, not a tentative optimization. Commit the
    /// original topology update FIRST, clearing old cuts and fencing old keys,
    /// before running any fallible solver or verifier. A bypass, unavailable
    /// inventory, exhausted budget or storage fault cannot revive the old graph.
    /// These are two ordered local cuts, not a fictitious atomic remote update.
    ///
    /// An exact update retry does not repeat its fence. Replanning is allowed
    /// only while that same graph remains current and available; retrying an old
    /// operation after a newer update must not certify the wrong generation.
    /// A withdrawal stays withdrawn and cannot be repaired by this optimizer.
    pub fn update_and_plan(&self, host: &mut FileOversight, revision: u64,
        update: &FileMediationUpdate, costs: &[EnforcerCost], budget: MediationPlanningBudget)
        -> Result<TopologyPlanningReceipt, JournalError>
    {
        let change = self.update(host, revision, update)?;
        let certification = (|| {
            let current = host.mediation_snapshot()?;
            let expected = update.next.as_ref().ok_or(Error::Incomplete)?;
            if &current.graph != expected { return Err(Error::Stale.into()); }
            if !current.available { return Err(Error::Incomplete.into()); }
            let revision = host.revision();
            let epoch = host.inspect().control.ledger.epoch;
            self.plan_and_certify(host, revision, epoch, costs, budget)
        })();
        Ok(TopologyPlanningReceipt { change, certification })
    }
}

#[cfg(test)]
mod tests;
