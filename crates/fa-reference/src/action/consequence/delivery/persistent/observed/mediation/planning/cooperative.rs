//! Original observer custody remains separate from a yielding optimizer.
use super::{FileMediationObserver, FileMediationUpdate, FileOversight, JournalError,
    MediationPlanningBudget, MediationPlanReceipt, TopologyChange};
use crate::action::consequence::mediation::{CutProposal, MAX_CHECK_EDGE_VISITS};
use crate::action::consequence::mediation::planning::{CutPlanOutcome, CutPlanningCursor,
    CutPlanningProgress, CutPlanningWork, EnforcerCost};
use crate::Error;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileMediationPlanningProgress {
    pub started_revision: u64,
    pub authority_epoch: u64,
    pub control_sequence: u64,
    pub solver: CutPlanningProgress,
    /// A changed original basis permanently invalidates this private task.
    pub invalidated: Option<JournalError>,
}

/// An acknowledged topology change plus a fallible, still nonauthoritative task.
/// An inner failure never undoes that change, its revocation or its accounting.
#[derive(Debug)]
pub struct TopologyPlanningTask {
    pub change: Option<TopologyChange>,
    pub planning: Result<FileMediationPlanning, JournalError>,
}

/// No live host borrow survives a poll. Control can stop/fence/update the SAME
/// owner between quanta; that change invalidates this cut before further work.
/// Time-only journal growth is allowed under an explicitly current predecessor.
/// The marker keeps no store lock alive and cannot recover observer custody.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::mediation::planning::FileMediationPlanning;
/// fn observer(run: FileMediationPlanning) { let _ = run.observer(); }
/// ```
#[derive(Debug)]
#[must_use = "poll and certify with the original observer, or cancel without changing authority"]
pub struct FileMediationPlanning {
    issuer: Rc<()>,
    started_revision: u64,
    epoch: u64,
    sequence: u64,
    solver: CutPlanningCursor,
    verification_edge_visits: usize,
    invalidated: Option<JournalError>,
}

impl FileMediationObserver {
    /// Freeze actual live topology and prices without retaining an owner borrow.
    /// This performs no solver searches, independent verification or journal I/O.
    /// The fixed budget belongs to this task, not a refillable per-poll limit.
    pub fn begin_plan_and_certify(&self, host: &FileOversight, revision: u64, epoch: u64,
        costs: &[EnforcerCost], budget: MediationPlanningBudget)
        -> Result<FileMediationPlanning, JournalError>
    {
        self.check_owner(host)?;
        if revision != host.revision() || epoch != host.inspect().control.ledger.epoch {
            return Err(Error::Stale.into());
        }
        if budget.verification_edge_visits > MAX_CHECK_EDGE_VISITS { return Err(Error::Limit.into()); }
        let current = host.mediation_snapshot()?;
        if !current.available { return Err(Error::Incomplete.into()); }
        let solver = current.graph.begin_minimum_cut(costs, budget.planning)?;
        Ok(FileMediationPlanning { issuer: Rc::clone(&self.issuer), started_revision: revision,
            epoch, sequence: host.inspect().control.sequence, solver,
            verification_edge_visits: budget.verification_edge_visits, invalidated: None })
    }

    /// Acknowledge the original revoking update FIRST. Even a failed planner
    /// admission, later cancellation or no polling leaves the new topology live
    /// and the old cut withdrawn. Exact retries use the original update receipt.
    pub fn update_and_begin_plan(&self, host: &mut FileOversight, revision: u64,
        update: &FileMediationUpdate, costs: &[EnforcerCost], budget: MediationPlanningBudget)
        -> Result<TopologyPlanningTask, JournalError>
    {
        let change = self.update(host, revision, update)?;
        let planning = (|| {
            let current = host.mediation_snapshot()?;
            let expected = update.next.as_ref().ok_or(Error::Incomplete)?;
            if &current.graph != expected { return Err(Error::Stale.into()); }
            if !current.available { return Err(Error::Incomplete.into()); }
            self.begin_plan_and_certify(host, host.revision(), host.inspect().control.ledger.epoch, costs, budget)
        })();
        Ok(TopologyPlanningTask { change, planning })
    }
}

impl FileMediationPlanning {
    pub fn progress(&self) -> FileMediationPlanningProgress {
        FileMediationPlanningProgress { started_revision: self.started_revision,
            authority_epoch: self.epoch, control_sequence: self.sequence,
            solver: self.solver.progress(), invalidated: self.invalidated.clone() }
    }

    /// An incorrect caller predecessor/owner spends nothing and may be corrected.
    /// A changed original topology, epoch or control sequence is terminal. Ordinary
    /// clock observations can advance the journal without resetting solver work.
    pub fn advance(&mut self, host: &FileOversight, revision: u64, max_edge_visits: usize)
        -> Result<FileMediationPlanningProgress, JournalError>
    {
        self.check_call(host, revision)?;
        if let Err(error) = self.check_basis(host) {
            self.invalidated = Some(error.clone()); return Err(error);
        }
        self.solver.advance(max_edge_visits)?;
        Ok(self.progress())
    }

    pub fn cancel(self) -> CutPlanningWork { self.solver.cancel() }

    /// Recheck the live cut and original observer, then submit ONLY the solver's
    /// complete plain proposal to the original durable independent checker.
    /// Ready is not a certificate. The returned inner check can still refuse.
    /// No cached precheck can skip original verifier/reducer/storage boundaries.
    pub fn certify(self, observer: &FileMediationObserver, host: &mut FileOversight, revision: u64)
        -> Result<MediationPlanReceipt, JournalError>
    {
        self.check_call(host, revision)?;
        observer.check_owner(host)?;
        self.check_basis(host)?;
        let graph = self.solver.graph().clone();
        let plan = self.solver.finish()?;
        let proposal = match &plan.outcome {
            CutPlanOutcome::Candidate(candidate) => candidate.proposal.clone(),
            CutPlanOutcome::Bypass(_) | CutPlanOutcome::Unreachable { .. } => CutProposal {
                gates: graph.spec().enforcers.iter().map(|gate| gate.node).collect(),
                graph, reachable: Vec::new(),
            },
        };
        // Preserve the original nonpermitting classification check: an empty
        // claimed partition cannot forge a verified cut even if a solver errs.
        let checked = observer.certify(host, revision, self.epoch, &proposal, self.verification_edge_visits)?;
        Ok(MediationPlanReceipt { journal_revision: host.revision(), plan, checked })
    }

    fn check_call(&self, host: &FileOversight, revision: u64) -> Result<(), JournalError> {
        if let Some(error) = &self.invalidated { return Err(error.clone()); }
        if !Rc::ptr_eq(&self.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        if host.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != host.revision() { return Err(Error::Stale.into()); }
        Ok(())
    }
    fn check_basis(&self, host: &FileOversight) -> Result<(), JournalError> {
        let control = host.inspect().control;
        if control.ledger.epoch != self.epoch || control.sequence != self.sequence {
            return Err(Error::Stale.into());
        }
        let current = host.mediation_snapshot()?;
        if !current.available { return Err(Error::Incomplete.into()); }
        if &current.graph != self.solver.graph() { return Err(Error::Binding.into()); }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
