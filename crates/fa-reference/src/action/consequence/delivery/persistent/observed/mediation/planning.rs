//! Select placement, then persist the ORIGINAL independently checked cut.
//! This is a supervisor integration, not an optimizer-owned permission path.
use super::{FileMediationObserver, FileMediationUpdate, FileOversight, JournalError};
use crate::action::consequence::delivery::TopologyChange;
use crate::action::consequence::mediation::{CutCheck, CutProposal, MAX_CHECK_EDGE_VISITS};
use crate::action::consequence::mediation::planning::{CutPlan, CutPlanOutcome, CutPlanningLimits, EnforcerCost};
use crate::Error;

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
        self.check_owner(host)?;
        if revision != host.revision() || epoch != host.inspect().control.ledger.epoch {
            return Err(Error::Stale.into());
        }
        if budget.verification_edge_visits > MAX_CHECK_EDGE_VISITS { return Err(Error::Limit.into()); }
        let current = host.mediation_snapshot()?;
        if !current.available { return Err(Error::Incomplete.into()); }
        let plan = current.graph.plan_minimum_cut(costs, budget.planning)?;
        let proposal = match &plan.outcome {
            CutPlanOutcome::Candidate(candidate) => candidate.proposal.clone(),
            CutPlanOutcome::Bypass(_) | CutPlanOutcome::Unreachable { .. } => {
                // Independently inspect all registered gates. A deliberately
                // empty claimed partition cannot certify a graph with actors,
                // even if a defective planner misclassified the topology.
                // With no gates, the original checker records InvalidInput.
                CutProposal { graph: current.graph.clone(),
                    gates: current.graph.spec().enforcers.iter().map(|gate| gate.node).collect(),
                    reachable: Vec::new() }
            }
        };
        let checked = self.certify(host, revision, epoch, &proposal, budget.verification_edge_visits)?;
        Ok(MediationPlanReceipt { journal_revision: host.revision(), plan, checked })
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
