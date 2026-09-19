//! Sparse, bounded placement of registered enforcers (plan 15.6-15.9).
//!
//! Costs describe the operator's declared placement objective, not risk or
//! permission. Vertex splitting and residual max-flow select a minimum-cost
//! candidate. The result is deliberately NOT a VerifiedCut: the original
//! independent checker must still verify it before any live gate accepts it.
//! Completeness, provenance and functioning enforcers remain assumptions.

use super::{AuthorityGraph, CutProposal, GraphSpec, NodeKind, PathWitness, MAX_CUT_NODES,
    MAX_EDGES, MAX_NODES};
use crate::Error;
use std::collections::{BTreeMap, VecDeque};

pub const MAX_PLANNING_EDGE_VISITS: usize = 4_194_304;
/// Both forward and reverse arcs: one split per vertex, every original edge,
/// and at most one synthetic root/sink edge per original vertex.
pub const MAX_RESIDUAL_ARCS: usize = 2 * (MAX_EDGES + 2 * MAX_NODES);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnforcerCost {
    pub node: u64,
    /// Positive integer deployment cost; no implicit price for an omitted gate.
    pub units: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CutPlanningLimits {
    pub residual_arcs: usize,
    pub edge_visits: usize,
}
impl Default for CutPlanningLimits {
    fn default() -> Self {
        Self { residual_arcs: MAX_RESIDUAL_ARCS, edge_visits: MAX_PLANNING_EDGE_VISITS }
    }
}

/// Counts are logical sizes/work, not allocated bytes, peak memory or latency.
/// edge_visits counts graph/residual search examinations and every residual arc
/// examined/updated while augmenting. Construction sizes are separate fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CutPlanningWork {
    pub projection_nodes: usize,
    pub projection_edges: usize,
    pub residual_vertices: usize,
    pub residual_arcs: usize,
    pub edge_visits: usize,
    pub augmentations: usize,
}
impl CutPlanningWork {
    fn visit(&mut self, count: usize, limit: usize) -> Result<(), Error> {
        let next = self.edge_visits.checked_add(count).ok_or(Error::Limit)?;
        if next > limit { return Err(Error::Limit); }
        self.edge_visits = next;
        Ok(())
    }
}

/// Editable proposal data, not an optimality certificate or an effect key.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::mediation::{VerifiedCut, planning::PlannedCut};
/// fn elevate(plan: PlannedCut) -> VerifiedCut { plan }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedCut {
    pub proposal: CutProposal,
    pub total_cost: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CutPlanOutcome {
    Candidate(PlannedCut),
    /// An actual modeled route avoids ALL registered enforcers. Spending more
    /// on those enforcers cannot cover it; the inventory/topology must change.
    Bypass(PathWitness),
    /// Preserve the original checker's non-vacuity rule for every declared sink.
    Unreachable { sinks: Vec<u64> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CutPlan {
    pub outcome: CutPlanOutcome,
    pub work: CutPlanningWork,
}

impl AuthorityGraph {
    /// Plan over ALL actor roots, ALL protected sinks and ONLY registered
    /// enforcers. Every enforcer must have exactly one explicit positive cost.
    /// Non-enforcers, unregistered enforcer-shaped nodes and individual channels
    /// cannot be removed by the optimizer. Parallel/reverse/self-loop channels
    /// are retained separately. Determinism uses the graph's canonical ID order,
    /// not a claim of lexicographically smallest choice among equal-cost cuts.
    ///
    /// Sparse storage is O(V + E); no V-by-V capacity matrix is constructed.
    /// Preflight bounds the complete residual layout before allocation. Search
    /// and augmentation share one work ceiling; exhaustion returns no candidate.
    /// These are bounded reference semantics, not qualified production scaling.
    pub fn plan_minimum_cut(&self, costs: &[EnforcerCost], limits: CutPlanningLimits)
        -> Result<CutPlan, Error>
    {
        if limits.residual_arcs == 0 || limits.residual_arcs > MAX_RESIDUAL_ARCS
            || limits.edge_visits == 0 || limits.edge_visits > MAX_PLANNING_EDGE_VISITS
        { return Err(Error::Limit); }
        self.admitted_inventory()?;
        let spec = self.spec();
        let (prices, total) = validate_costs(spec, costs)?;
        // An uncuttable arc must cost strictly more than ALL legal cuts. Never
        // saturate an overflowing sum or confuse an expensive gate with infinity.
        let infinity = total.checked_add(1).ok_or(Error::Overflow)?;
        let terminals = spec.nodes.iter().filter(|node|
            matches!(node.kind, NodeKind::Actor | NodeKind::Sink)).count();
        let arc_count = spec.nodes.len().checked_add(spec.edges.len())
            .and_then(|n| n.checked_add(terminals)).and_then(|n| n.checked_mul(2))
            .ok_or(Error::Limit)?;
        if arc_count > limits.residual_arcs { return Err(Error::Limit); }
        let mut work = CutPlanningWork { projection_nodes: spec.nodes.len(),
            projection_edges: spec.edges.len(), ..CutPlanningWork::default() };
        let projection = Projection::new(spec);
        let empty = vec![false; spec.nodes.len()];
        let original = projection.reach(&empty, &mut work, limits.edge_visits)?;
        let unreachable: Vec<_> = projection.sinks.iter().filter(|&&sink| !original.seen[sink])
            .map(|&sink| spec.nodes[sink].id).collect();
        if !unreachable.is_empty() {
            return Ok(CutPlan { outcome: CutPlanOutcome::Unreachable { sinks: unreachable }, work });
        }
        let blocked: Vec<_> = spec.nodes.iter().map(|node| prices.contains_key(&node.id)).collect();
        let without_gates = projection.reach(&blocked, &mut work, limits.edge_visits)?;
        if let Some(&sink) = projection.sinks.iter().find(|&&sink| without_gates.seen[sink]) {
            return Ok(CutPlan { outcome: CutPlanOutcome::Bypass(projection.path(&without_gates, sink)), work });
        }
        // All-gates removal disconnected every originally reachable sink, so a
        // finite cut exists. The two synthetic vertices cannot themselves be cut.
        let source = 2 * spec.nodes.len();
        let sink = source + 1;
        let mut residual = Residual::new(sink + 1);
        for (index, node) in spec.nodes.iter().enumerate() {
            residual.add(2 * index, 2 * index + 1, prices.get(&node.id).copied().unwrap_or(infinity));
            match node.kind {
                NodeKind::Actor => residual.add(source, 2 * index, infinity),
                NodeKind::Sink => residual.add(2 * index + 1, sink, infinity),
                _ => {}
            }
        }
        for edge in &spec.edges {
            residual.add(2 * projection.ordinals[&edge.from] + 1,
                2 * projection.ordinals[&edge.to], infinity);
        }
        work.residual_vertices = sink + 1;
        work.residual_arcs = residual.rows.iter().map(Vec::len).sum();
        if work.residual_arcs != arc_count { return Err(Error::Binding); }
        let (flow, reachable) = residual.maximum_flow(source, sink, total, &mut work, limits.edge_visits)?;
        let gates: Vec<_> = spec.enforcers.iter().filter(|gate| {
            let ordinal = projection.ordinals[&gate.node];
            reachable[2 * ordinal] && !reachable[2 * ordinal + 1]
        }).map(|gate| gate.node).collect();
        let cost = gates.iter().try_fold(0_u64, |sum, id| sum.checked_add(prices[id]))
            .ok_or(Error::Overflow)?;
        if gates.is_empty() || cost != flow { return Err(Error::Binding); }
        // Recompute the ordinary graph partition, not a split-network partition.
        // This remains untrusted input to the separate original cut checker.
        let blocked: Vec<_> = spec.nodes.iter().map(|node| gates.binary_search(&node.id).is_ok()).collect();
        let final_reach = projection.reach(&blocked, &mut work, limits.edge_visits)?;
        if projection.sinks.iter().any(|&sink| final_reach.seen[sink]) { return Err(Error::Binding); }
        let reachable = spec.nodes.iter().enumerate().filter(|(i, _)| final_reach.seen[*i])
            .map(|(_, node)| node.id).collect();
        Ok(CutPlan { outcome: CutPlanOutcome::Candidate(PlannedCut {
            proposal: CutProposal { graph: self.clone(), gates, reachable }, total_cost: cost,
        }), work })
    }
}

fn validate_costs(spec: &GraphSpec, costs: &[EnforcerCost]) -> Result<(BTreeMap<u64, u64>, u64), Error> {
    if costs.len() > MAX_CUT_NODES { return Err(Error::Limit); }
    let mut prices = BTreeMap::new();
    let mut total = 0_u64;
    for cost in costs {
        if cost.units == 0 { return Err(Error::InvalidInput); }
        if !spec.enforcers.iter().any(|gate| gate.node == cost.node) { return Err(Error::Binding); }
        if prices.insert(cost.node, cost.units).is_some() { return Err(Error::Duplicate); }
        total = total.checked_add(cost.units).ok_or(Error::Overflow)?;
    }
    if prices.len() != spec.enforcers.len() { return Err(Error::Incomplete); }
    Ok((prices, total))
}

struct Projection<'a> {
    spec: &'a GraphSpec,
    ordinals: BTreeMap<u64, usize>,
    rows: Vec<Vec<(usize, usize)>>,
    roots: Vec<usize>,
    sinks: Vec<usize>,
}
struct Reach { seen: Vec<bool>, predecessor: Vec<Option<(usize, usize)>> }
impl<'a> Projection<'a> {
    fn new(spec: &'a GraphSpec) -> Self {
        let ordinals: BTreeMap<_, _> = spec.nodes.iter().enumerate().map(|(i, node)| (node.id, i)).collect();
        let mut rows = vec![Vec::new(); spec.nodes.len()];
        for (index, edge) in spec.edges.iter().enumerate() {
            rows[ordinals[&edge.from]].push((ordinals[&edge.to], index));
        }
        let roots = spec.nodes.iter().enumerate().filter(|(_, n)| n.kind == NodeKind::Actor).map(|(i, _)| i).collect();
        let sinks = spec.nodes.iter().enumerate().filter(|(_, n)| n.kind == NodeKind::Sink).map(|(i, _)| i).collect();
        Self { spec, ordinals, rows, roots, sinks }
    }
    fn reach(&self, blocked: &[bool], work: &mut CutPlanningWork, limit: usize) -> Result<Reach, Error> {
        let mut result = Reach { seen: vec![false; self.rows.len()], predecessor: vec![None; self.rows.len()] };
        let mut queue = VecDeque::new();
        for &root in &self.roots { result.seen[root] = true; queue.push_back(root); }
        while let Some(from) = queue.pop_front() {
            for &(to, edge) in &self.rows[from] {
                work.visit(1, limit)?;
                if !blocked[to] && !result.seen[to] {
                    result.seen[to] = true;
                    result.predecessor[to] = Some((from, edge));
                    queue.push_back(to);
                }
            }
        }
        Ok(result)
    }
    fn path(&self, reached: &Reach, mut sink: usize) -> PathWitness {
        let mut nodes = vec![self.spec.nodes[sink].id];
        let mut edges = Vec::new();
        while let Some((from, edge)) = reached.predecessor[sink] {
            edges.push(self.spec.edges[edge].id);
            nodes.push(self.spec.nodes[from].id);
            sink = from;
        }
        nodes.reverse(); edges.reverse();
        PathWitness { nodes, edges }
    }
}

#[derive(Clone, Copy)]
struct Arc { to: usize, reverse: usize, remaining: u64 }
struct Residual { rows: Vec<Vec<Arc>> }
impl Residual {
    fn new(vertices: usize) -> Self { Self { rows: vec![Vec::new(); vertices] } }
    fn add(&mut self, from: usize, to: usize, capacity: u64) {
        // Vertex splitting means even a self-loop in the original graph has
        // distinct residual endpoints. Reverse slots never alias their forward.
        debug_assert_ne!(from, to);
        let forward = self.rows[from].len();
        let reverse = self.rows[to].len();
        self.rows[from].push(Arc { to, reverse, remaining: capacity });
        self.rows[to].push(Arc { to: from, reverse: forward, remaining: 0 });
    }
    fn search(&self, source: usize, work: &mut CutPlanningWork, limit: usize) -> Result<Reach, Error> {
        let mut reach = Reach { seen: vec![false; self.rows.len()], predecessor: vec![None; self.rows.len()] };
        let mut queue = VecDeque::from([source]);
        reach.seen[source] = true;
        while let Some(from) = queue.pop_front() {
            for (index, arc) in self.rows[from].iter().enumerate() {
                work.visit(1, limit)?;
                if arc.remaining > 0 && !reach.seen[arc.to] {
                    reach.seen[arc.to] = true;
                    reach.predecessor[arc.to] = Some((from, index));
                    queue.push_back(arc.to);
                }
            }
        }
        Ok(reach)
    }
    fn maximum_flow(&mut self, source: usize, sink: usize, finite_bound: u64,
        work: &mut CutPlanningWork, limit: usize) -> Result<(u64, Vec<bool>), Error>
    {
        let mut total = 0_u64;
        loop {
            let reached = self.search(source, work, limit)?;
            if !reached.seen[sink] { return Ok((total, reached.seen)); }
            let mut amount = u64::MAX;
            let mut at = sink;
            while at != source {
                let (from, index) = reached.predecessor[at].ok_or(Error::Binding)?;
                work.visit(1, limit)?;
                amount = amount.min(self.rows[from][index].remaining);
                at = from;
            }
            let next = total.checked_add(amount).ok_or(Error::Overflow)?;
            if amount == 0 || next > finite_bound { return Err(Error::Binding); }
            at = sink;
            while at != source {
                let (from, index) = reached.predecessor[at].ok_or(Error::Binding)?;
                work.visit(2, limit)?;
                let arc = self.rows[from][index];
                self.rows[from][index].remaining = arc.remaining.checked_sub(amount).ok_or(Error::Binding)?;
                let reverse = &mut self.rows[arc.to][arc.reverse];
                reverse.remaining = reverse.remaining.checked_add(amount).ok_or(Error::Overflow)?;
                at = from;
            }
            total = next;
            work.augmentations += 1;
        }
    }
}

#[cfg(test)]
mod tests;
