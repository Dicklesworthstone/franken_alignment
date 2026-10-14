//! Resume sparse placement at the exact next edge, not at the start of a search.
//! The original synchronous planner remains an independent scheduling oracle.
use super::{AuthorityGraph, CutPlan, CutPlanOutcome, CutPlanningLimits, CutPlanningWork,
    CutProposal, EnforcerCost, Error, NodeKind, PathWitness, PlannedCut, Projection,
    Reach, Residual, MAX_PLANNING_EDGE_VISITS, MAX_RESIDUAL_ARCS, validate_costs};
use std::collections::{BTreeMap, VecDeque};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CutPlanningStatus { Planning, Ready, Failed(Error), Interrupted }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CutPlanningProgress {
    pub status: CutPlanningStatus,
    pub work: CutPlanningWork,
    /// Next atomic edge allowance, not the remaining cost. Augmenting a
    /// forward/reverse pair requires two visits in the SAME poll.
    pub minimum_edge_visits: usize,
}

#[derive(Clone, Copy)]
enum Phase {
    Original, AllGates, Residual, Final,
    Bottleneck { at: usize, amount: u64 },
    Augment { at: usize, amount: u64, next: u64 },
}

/// Frozen graph, costs and lifetime limits; private partial flow/partitions.
/// No host borrow, deployment handle, credential or accepted cut is retained.
/// Polling preserves the original sparse ordering, parallel channels and exact
/// work accounting. A result is still only input to the independent verifier.
///
/// Construction/projection, residual allocation, queue bookkeeping, path and
/// result materialization remain bounded synchronous work. Edge quanta are NOT
/// wall-clock, allocation, syscall or cancellation-latency guarantees.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::mediation::planning::CutPlanningCursor;
/// fn fork(run: CutPlanningCursor) { let _ = run.clone(); }
/// ```
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::mediation::{VerifiedCut, planning::CutPlanningCursor};
/// fn certify(run: CutPlanningCursor) -> VerifiedCut { run.finish().unwrap() }
/// ```
#[must_use = "finish the complete plan or cancel without installing a partial cut"]
pub struct CutPlanningCursor {
    graph: AuthorityGraph,
    prices: BTreeMap<u64, u64>,
    finite_bound: u64,
    infinity: u64,
    limits: CutPlanningLimits,
    arc_count: usize,
    ordinals: BTreeMap<u64, usize>,
    rows: Vec<Vec<(usize, usize)>>,
    roots: Vec<usize>,
    sinks: Vec<usize>,
    blocked: Vec<bool>,
    reach: Reach,
    queue: VecDeque<usize>,
    row: Option<usize>,
    edge: usize,
    residual: Option<Residual>,
    flow: u64,
    gates: Vec<u64>,
    phase: Phase,
    work: CutPlanningWork,
    status: CutPlanningStatus,
    interrupted: bool,
    minimum: usize,
    outcome: Option<CutPlanOutcome>,
}
impl fmt::Debug for CutPlanningCursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CutPlanningCursor").field("progress", &self.progress()).finish_non_exhaustive()
    }
}

impl AuthorityGraph {
    /// Same admission as plan_minimum_cut; no searches or augmentations yet.
    /// Every poll spends the original FIXED lifetime allowance. Repeated polls
    /// cannot buy a larger search, reset prices, or change the selected graph.
    pub fn begin_minimum_cut(&self, costs: &[EnforcerCost], limits: CutPlanningLimits)
        -> Result<CutPlanningCursor, Error>
    {
        if limits.residual_arcs == 0 || limits.residual_arcs > MAX_RESIDUAL_ARCS
            || limits.edge_visits == 0 || limits.edge_visits > MAX_PLANNING_EDGE_VISITS
        { return Err(Error::Limit); }
        self.admitted_inventory()?;
        let spec = self.spec();
        let (prices, total) = validate_costs(spec, costs)?;
        let infinity = total.checked_add(1).ok_or(Error::Overflow)?;
        let terminals = spec.nodes.iter().filter(|node|
            matches!(node.kind, NodeKind::Actor | NodeKind::Sink)).count();
        let arc_count = spec.nodes.len().checked_add(spec.edges.len())
            .and_then(|n| n.checked_add(terminals)).and_then(|n| n.checked_mul(2))
            .ok_or(Error::Limit)?;
        if arc_count > limits.residual_arcs { return Err(Error::Limit); }
        // Move the ORIGINAL projection's sparse rows and ID mapping. Its only
        // borrow is not retained; the immutable graph uses its original Rc.
        let projection = Projection::new(spec);
        let mut reach = Reach { seen: vec![false; spec.nodes.len()],
            predecessor: vec![None; spec.nodes.len()] };
        let mut queue = VecDeque::new();
        for &root in &projection.roots { reach.seen[root] = true; queue.push_back(root); }
        Ok(CutPlanningCursor {
            graph: self.clone(), prices, finite_bound: total, infinity, limits, arc_count,
            ordinals: projection.ordinals, rows: projection.rows,
            roots: projection.roots, sinks: projection.sinks,
            blocked: vec![false; spec.nodes.len()], reach, queue, row: None, edge: 0,
            residual: None, flow: 0, gates: Vec::new(), phase: Phase::Original,
            work: CutPlanningWork { projection_nodes: spec.nodes.len(),
                projection_edges: spec.edges.len(), ..CutPlanningWork::default() },
            status: CutPlanningStatus::Planning, interrupted: false, minimum: 1, outcome: None,
        })
    }
}

impl CutPlanningCursor {
    pub fn graph(&self) -> &AuthorityGraph { &self.graph }
    pub fn progress(&self) -> CutPlanningProgress {
        CutPlanningProgress {
            status: if self.interrupted { CutPlanningStatus::Interrupted } else { self.status },
            work: self.work,
            minimum_edge_visits: if self.status == CutPlanningStatus::Planning && !self.interrupted {
                self.minimum
            } else { 0 },
        }
    }

    /// Zero does no work. Completed polls may run bounded phase bookkeeping
    /// after the last charged edge, so the exact lifetime allowance can finish.
    /// Errors and unwinds are terminal; neither exports an incumbent cut.
    pub fn advance(&mut self, max_edge_visits: usize) -> Result<CutPlanningProgress, Error> {
        if self.interrupted { return Err(Error::WrongState); }
        match self.status {
            CutPlanningStatus::Failed(error) => return Err(error),
            CutPlanningStatus::Ready => return Ok(self.progress()),
            CutPlanningStatus::Interrupted => return Err(Error::WrongState),
            CutPlanningStatus::Planning => {}
        }
        if max_edge_visits == 0 { return Ok(self.progress()); }
        self.interrupted = true;
        let result = self.run(max_edge_visits);
        self.interrupted = false;
        if let Err(error) = result { self.status = CutPlanningStatus::Failed(error); return Err(error); }
        Ok(self.progress())
    }

    pub fn cancel(self) -> CutPlanningWork { self.work }

    /// Complete proposal data only. The original verifier, not this optimizer,
    /// owns coverage certification. Consuming an incomplete job returns no plan.
    pub fn finish(self) -> Result<CutPlan, Error> {
        if self.interrupted { return Err(Error::WrongState); }
        match self.status {
            CutPlanningStatus::Ready => Ok(CutPlan {
                outcome: self.outcome.ok_or(Error::Incomplete)?, work: self.work,
            }),
            CutPlanningStatus::Failed(error) => Err(error),
            _ => Err(Error::Incomplete),
        }
    }

    fn run(&mut self, mut left: usize) -> Result<(), Error> {
        while self.status == CutPlanningStatus::Planning {
            // Resolve only bounded, zero-edge bookkeeping. Searches visit the
            // complete reachable residual side, exactly as the original solver.
            let cost = self.next_cost()?;
            if self.status == CutPlanningStatus::Ready { break; }
            self.minimum = cost;
            if cost > left { break; }
            self.work.visit(cost, self.limits.edge_visits)?;
            left -= cost;
            self.step()?;
        }
        Ok(())
    }

    fn next_cost(&mut self) -> Result<usize, Error> {
        loop {
            match self.phase {
                Phase::Bottleneck { at, amount } => {
                    if at != self.source() { return Ok(1); }
                    let next = self.flow.checked_add(amount).ok_or(Error::Overflow)?;
                    if amount == 0 || next > self.finite_bound { return Err(Error::Binding); }
                    self.phase = Phase::Augment { at: self.sink(), amount, next };
                }
                Phase::Augment { at, next, .. } => {
                    if at != self.source() { return Ok(2); }
                    self.flow = next;
                    self.work.augmentations += 1;
                    self.phase = Phase::Residual;
                    self.start_search(true);
                }
                _ => {
                    if let Some(row) = self.row {
                        let len = if matches!(self.phase, Phase::Residual) {
                            self.residual.as_ref().ok_or(Error::Incomplete)?.rows[row].len()
                        } else { self.rows[row].len() };
                        if self.edge < len { return Ok(1); }
                    }
                    self.row = self.queue.pop_front();
                    self.edge = 0;
                    if self.row.is_some() { continue; }
                    self.end_search()?;
                    if self.status == CutPlanningStatus::Ready { return Ok(0); }
                }
            }
        }
    }

    fn step(&mut self) -> Result<(), Error> {
        match self.phase {
            Phase::Bottleneck { at, amount } => {
                let (from, index) = self.reach.predecessor[at].ok_or(Error::Binding)?;
                let capacity = self.residual.as_ref().ok_or(Error::Incomplete)?.rows[from][index].remaining;
                self.phase = Phase::Bottleneck { at: from, amount: amount.min(capacity) };
            }
            Phase::Augment { at, amount, next } => {
                let (from, index) = self.reach.predecessor[at].ok_or(Error::Binding)?;
                let residual = self.residual.as_mut().ok_or(Error::Incomplete)?;
                let arc = residual.rows[from][index];
                residual.rows[from][index].remaining = arc.remaining.checked_sub(amount).ok_or(Error::Binding)?;
                let reverse = &mut residual.rows[arc.to][arc.reverse];
                reverse.remaining = reverse.remaining.checked_add(amount).ok_or(Error::Overflow)?;
                self.phase = Phase::Augment { at: from, amount, next };
            }
            _ => {
                let from = self.row.ok_or(Error::Incomplete)?;
                let (to, index, admitted) = if matches!(self.phase, Phase::Residual) {
                    let arc = self.residual.as_ref().ok_or(Error::Incomplete)?.rows[from][self.edge];
                    (arc.to, self.edge, arc.remaining > 0)
                } else {
                    let (to, index) = self.rows[from][self.edge];
                    (to, index, !self.blocked[to])
                };
                self.edge += 1;
                if admitted && !self.reach.seen[to] {
                    self.reach.seen[to] = true;
                    self.reach.predecessor[to] = Some((from, index));
                    self.queue.push_back(to);
                }
            }
        }
        Ok(())
    }

    fn source(&self) -> usize { 2 * self.rows.len() }
    fn sink(&self) -> usize { self.source() + 1 }
    fn start_search(&mut self, residual: bool) {
        let size = if residual { self.sink() + 1 } else { self.rows.len() };
        self.reach = Reach { seen: vec![false; size], predecessor: vec![None; size] };
        self.queue.clear(); self.row = None; self.edge = 0;
        if residual {
            let source = self.source(); self.reach.seen[source] = true; self.queue.push_back(source);
        } else {
            for &root in &self.roots { self.reach.seen[root] = true; self.queue.push_back(root); }
        }
    }
    fn ready(&mut self, outcome: CutPlanOutcome) {
        self.outcome = Some(outcome); self.status = CutPlanningStatus::Ready; self.minimum = 0;
    }
    fn end_search(&mut self) -> Result<(), Error> {
        match self.phase {
            Phase::Original => {
                let sinks: Vec<_> = self.sinks.iter().filter(|&&sink| !self.reach.seen[sink])
                    .map(|&sink| self.graph.spec().nodes[sink].id).collect();
                if !sinks.is_empty() { self.ready(CutPlanOutcome::Unreachable { sinks }); }
                else {
                    self.blocked = self.graph.spec().nodes.iter().map(|node| self.prices.contains_key(&node.id)).collect();
                    self.phase = Phase::AllGates; self.start_search(false);
                }
            }
            Phase::AllGates => {
                if let Some(&sink) = self.sinks.iter().find(|&&sink| self.reach.seen[sink]) {
                    let mut nodes = vec![self.graph.spec().nodes[sink].id];
                    let mut edges = Vec::new(); let mut at = sink;
                    while let Some((from, edge)) = self.reach.predecessor[at] {
                        edges.push(self.graph.spec().edges[edge].id);
                        nodes.push(self.graph.spec().nodes[from].id); at = from;
                    }
                    nodes.reverse(); edges.reverse();
                    self.ready(CutPlanOutcome::Bypass(PathWitness { nodes, edges }));
                } else {
                    let source = self.source(); let sink = self.sink();
                    let mut residual = Residual::new(sink + 1);
                    for (index, node) in self.graph.spec().nodes.iter().enumerate() {
                        residual.add(2 * index, 2 * index + 1, self.prices.get(&node.id).copied().unwrap_or(self.infinity));
                        match node.kind {
                            NodeKind::Actor => residual.add(source, 2 * index, self.infinity),
                            NodeKind::Sink => residual.add(2 * index + 1, sink, self.infinity),
                            _ => {}
                        }
                    }
                    for edge in &self.graph.spec().edges {
                        residual.add(2 * self.ordinals[&edge.from] + 1, 2 * self.ordinals[&edge.to], self.infinity);
                    }
                    self.work.residual_vertices = sink + 1;
                    self.work.residual_arcs = residual.rows.iter().map(Vec::len).sum();
                    if self.work.residual_arcs != self.arc_count { return Err(Error::Binding); }
                    self.residual = Some(residual); self.phase = Phase::Residual; self.start_search(true);
                }
            }
            Phase::Residual => {
                if self.reach.seen[self.sink()] {
                    self.phase = Phase::Bottleneck { at: self.sink(), amount: u64::MAX };
                } else {
                    self.gates = self.graph.spec().enforcers.iter().filter(|gate| {
                        let ordinal = self.ordinals[&gate.node];
                        self.reach.seen[2 * ordinal] && !self.reach.seen[2 * ordinal + 1]
                    }).map(|gate| gate.node).collect();
                    let cost = self.gates.iter().try_fold(0_u64, |sum, id| sum.checked_add(self.prices[id]))
                        .ok_or(Error::Overflow)?;
                    if self.gates.is_empty() || cost != self.flow { return Err(Error::Binding); }
                    self.blocked = self.graph.spec().nodes.iter().map(|node| self.gates.binary_search(&node.id).is_ok()).collect();
                    self.phase = Phase::Final; self.start_search(false);
                }
            }
            Phase::Final => {
                if self.sinks.iter().any(|&sink| self.reach.seen[sink]) { return Err(Error::Binding); }
                let reachable = self.graph.spec().nodes.iter().enumerate().filter(|(i, _)| self.reach.seen[*i])
                    .map(|(_, node)| node.id).collect();
                let proposal = CutProposal { graph: self.graph.clone(),
                    gates: std::mem::take(&mut self.gates), reachable };
                self.ready(CutPlanOutcome::Candidate(PlannedCut { proposal, total_cost: self.flow }));
            }
            _ => return Err(Error::WrongState),
        }
        Ok(())
    }
}
