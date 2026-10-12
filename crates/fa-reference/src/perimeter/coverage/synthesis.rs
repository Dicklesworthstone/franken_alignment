//! Bounded least-cost eligible vertex cuts for the original logical graph.
//!
//! This compiler selects declared barrier sites; it never installs a barrier.
//! Completeness and eligibility retain the parent module's caller trust boundary.

use super::{CoverageAudit, CoverageCertificate, CoverageProblem};
use crate::Error;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Two vertices per original node and two terminals, for Graph's 128-node cap.
pub const MAX_CUT_RESIDUAL_CELLS: usize = 258 * 258;
pub const MAX_CUT_RESIDUAL_SCANS: u64 = 50_000_000;
pub const MAX_CUT_AUGMENTATIONS: u64 = 4_096;

/// Explicit limits for the residual solver, NOT wall time, FLOPs or process RSS.
/// Ordinary topology audits have their own fixed bounds in CoverageProblem.
/// Zero limits are valid: a zero augmentation allowance still admits free cuts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CutBudget {
    pub residual_cells: usize,
    pub residual_scans: u64,
    pub augmentations: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CutWork {
    /// Allocated u128 residual cells, excluding queues and certificate storage.
    pub residual_cells: usize,
    /// Inspected residual cells, including zero capacity and already-seen nodes.
    pub residual_scans: u64,
    pub augmentations: u64,
    /// Completed calls to the parent audit, each with two original-graph BFSs.
    pub topology_audits: u64,
    pub independent_cut_checks: u64,
}

/// Exhaustion exposes spent solver work but never a partial or unproved plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CutFailure {
    pub error: Error,
    pub work: CutWork,
}

/// A logical proposal, not an installed mediator or an authority token.
///
/// ```compile_fail,E0308
/// use fa_reference::action::Permit;
/// use fa_reference::perimeter::coverage::synthesis::CutPlan;
/// fn authorize(plan: CutPlan) -> Permit { plan }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CutPlan {
    certificate: CoverageCertificate,
    costs: BTreeMap<usize, u64>,
    total_cost: u128,
    work: CutWork,
}

impl CutPlan {
    pub fn certificate(&self) -> &CoverageCertificate {
        &self.certificate
    }

    pub fn barriers(&self) -> &BTreeSet<usize> {
        self.certificate.audit().barriers()
    }

    /// The entire cost inventory, including unselected candidates.
    pub fn costs(&self) -> &BTreeMap<usize, u64> {
        &self.costs
    }

    pub fn total_cost(&self) -> u128 {
        self.total_cost
    }

    pub fn work(&self) -> CutWork {
        self.work
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CutSynthesis {
    /// No active sink exists; no positive coverage certificate is issued.
    AlreadyUnreachable { audit: CoverageAudit, work: CutWork },
    /// A concrete bypass survives even deletion of EVERY eligible candidate.
    Impossible { audit: CoverageAudit, work: CutWork },
    Planned(Box<CutPlan>),
}

impl CoverageProblem {
    /// Minimum total declared vertex cost over the exact eligible candidate set.
    /// Zero-cost barriers are supported. Costs use u64; sums and residuals use
    /// u128 so even multiple u64::MAX costs do not overflow or become infinity.
    ///
    /// A deterministic vertex-split max-flow solver proposes a cut. The original
    /// Graph::cut_disconnects oracle independently verifies its actual coverage.
    /// Equal-cost alternatives use sorted traversal, NOT minimum cardinality.
    /// An exhausted budget never exports an incumbent as an optimal solution.
    pub fn minimum_cost_cut(
        &self,
        costs: &BTreeMap<usize, u64>,
        budget: CutBudget,
    ) -> Result<CutSynthesis, CutFailure> {
        let mut work = CutWork::default();
        self.synthesize(costs, budget, &mut work)
            .map_err(|error| CutFailure { error, work })
    }

    /// Rebind the full topology AND every candidate's cost, then recompute the
    /// optimum. A graph-only certificate does not prove that old prices remain
    /// current. Verification is observational and installs no barriers.
    pub fn verify_cut_plan(
        &self,
        plan: &CutPlan,
        costs: &BTreeMap<usize, u64>,
        budget: CutBudget,
    ) -> Result<(), CutFailure> {
        if self != &plan.certificate.problem || costs != &plan.costs {
            return Err(CutFailure {
                error: Error::Binding,
                work: CutWork::default(),
            });
        }
        match self.minimum_cost_cut(costs, budget)? {
            CutSynthesis::Planned(checked)
                if checked.total_cost == plan.total_cost
                    && checked.certificate == plan.certificate =>
            {
                Ok(())
            }
            checked => {
                let work = match checked {
                    CutSynthesis::Planned(plan) => plan.work,
                    CutSynthesis::AlreadyUnreachable { work, .. }
                    | CutSynthesis::Impossible { work, .. } => work,
                };
                Err(CutFailure { error: Error::Binding, work })
            }
        }
    }

    fn synthesize(
        &self,
        costs: &BTreeMap<usize, u64>,
        budget: CutBudget,
        work: &mut CutWork,
    ) -> Result<CutSynthesis, Error> {
        if budget.residual_cells > MAX_CUT_RESIDUAL_CELLS
            || budget.residual_scans > MAX_CUT_RESIDUAL_SCANS
            || budget.augmentations > MAX_CUT_AUGMENTATIONS
        {
            return Err(Error::Limit);
        }
        if costs.len() != self.declaration.candidates.len()
            || costs.keys().ne(self.declaration.candidates.iter())
        {
            return Err(Error::Binding);
        }
        let all = self.audit(&self.declaration.candidates)?;
        work.topology_audits += 1;
        if all.active_sinks() == 0 {
            return Ok(CutSynthesis::AlreadyUnreachable { audit: all, work: *work });
        }
        if all.has_bypass() {
            return Ok(CutSynthesis::Impossible { audit: all, work: *work });
        }

        let nodes = self.graph.edges.len();
        let vertices = 2 * nodes + 2;
        let cells = vertices * vertices;
        if cells > budget.residual_cells {
            return Err(Error::Limit);
        }
        let mut residual = Vec::new();
        residual.try_reserve_exact(cells).map_err(|_| Error::Limit)?;
        residual.resize(cells, 0_u128);
        work.residual_cells = cells;
        let total = costs.values().try_fold(0_u128, |sum, &cost| {
            sum.checked_add(u128::from(cost)).ok_or(Error::Overflow)
        })?;
        // Strictly more expensive than deleting every eligible candidate.
        // The preceding audit established that this complete deletion is a cut.
        let infinite = total.checked_add(1).ok_or(Error::Overflow)?;
        let source = 2 * nodes;
        let sink = source + 1;
        for (node, edges) in self.graph.edges.iter().enumerate() {
            residual[(2 * node) * vertices + 2 * node + 1] =
                costs.get(&node).map_or(infinite, |&cost| u128::from(cost));
            for &next in edges {
                residual[(2 * node + 1) * vertices + 2 * next] = infinite;
            }
        }
        for &node in &self.declaration.sources {
            residual[source * vertices + 2 * node] = infinite;
        }
        for &node in &self.declaration.sinks {
            residual[(2 * node + 1) * vertices + sink] = infinite;
        }

        let mut flow = 0_u128;
        let reachable = loop {
            let parents = residual_search(&residual, vertices, source, sink, budget, work)?;
            if parents[sink].is_none() {
                break parents;
            }
            if work.augmentations == budget.augmentations {
                return Err(Error::Limit);
            }
            let mut amount = infinite;
            let mut node = sink;
            while node != source {
                let previous = parents[node].ok_or(Error::Binding)?;
                amount = amount.min(residual[previous * vertices + node]);
                node = previous;
            }
            if amount == 0 {
                return Err(Error::Binding);
            }
            node = sink;
            while node != source {
                let previous = parents[node].ok_or(Error::Binding)?;
                residual[previous * vertices + node] -= amount;
                let reverse = &mut residual[node * vertices + previous];
                *reverse = reverse.checked_add(amount).ok_or(Error::Overflow)?;
                node = previous;
            }
            work.augmentations += 1;
            flow = flow.checked_add(amount).ok_or(Error::Overflow)?;
            if flow > total {
                return Err(Error::Binding);
            }
        };
        let barriers: BTreeSet<_> = self.declaration.candidates.iter().copied()
            .filter(|&node| reachable[2 * node].is_some() && reachable[2 * node + 1].is_none())
            .collect();
        let selected_cost = barriers.iter().try_fold(0_u128, |sum, node| {
            sum.checked_add(u128::from(costs[node])).ok_or(Error::Overflow)
        })?;
        if selected_cost != flow {
            return Err(Error::Binding);
        }
        let certificate = self.certify(&barriers)?;
        work.topology_audits += 1;
        work.independent_cut_checks += 1;
        Ok(CutSynthesis::Planned(Box::new(CutPlan {
            certificate,
            costs: costs.clone(),
            total_cost: selected_cost,
            work: *work,
        })))
    }
}

// Edmonds-Karp BFS. On a failed search the returned set is the COMPLETE residual
// source side. Every matrix-cell examination is charged before it takes place.
fn residual_search(
    residual: &[u128],
    vertices: usize,
    source: usize,
    sink: usize,
    budget: CutBudget,
    work: &mut CutWork,
) -> Result<Vec<Option<usize>>, Error> {
    let mut parents = vec![None; vertices];
    parents[source] = Some(source);
    let mut queue = VecDeque::from([source]);
    while let Some(node) = queue.pop_front() {
        let row = &residual[node * vertices..(node + 1) * vertices];
        for (next, capacity) in row.iter().enumerate() {
            if work.residual_scans == budget.residual_scans {
                return Err(Error::Limit);
            }
            work.residual_scans += 1;
            if *capacity > 0 && parents[next].is_none() {
                parents[next] = Some(node);
                if next == sink {
                    return Ok(parents);
                }
                queue.push_back(next);
            }
        }
    }
    Ok(parents)
}
