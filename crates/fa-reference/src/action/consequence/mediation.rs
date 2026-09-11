//! Typed authority projection and independent cut checking (FA-072/FA-073).
//!
//! The finite graph, perimeter inventory and enforcer provenance are trusted
//! declarations. This proves no OS containment, credential isolation or source
//! authenticity. A cut is evidence about this exact graph, never a Permit.

use crate::action::{Purpose, ResolvedTarget, Scope};
use crate::perimeter::{BypassDisposition, EffectFamilyRecord, Mediation, PerimeterInventory};
use crate::Error;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::rc::Rc;

pub const MAX_NODES: usize = 256;
pub const MAX_EDGES: usize = 4_096;
pub const MAX_CUT_NODES: usize = 64;
pub const MAX_CHECK_EDGE_VISITS: usize = 2 * MAX_NODES * MAX_EDGES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind { Actor, Process, Credential, Enforcer, Sink }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node { pub id: u64, pub kind: NodeKind }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel { Delegation, Execution, Ipc, CredentialAccess, Dispatch }

/// Parallel channels have distinct edge IDs and keep their own provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edge {
    pub id: u64,
    pub from: u64,
    pub to: u64,
    pub channel: Channel,
    pub route: String,
    pub provenance: u64,
}

/// Caller-registered boundary, not a self-attestation by the actor or enforcer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Enforcer {
    pub node: u64,
    pub adapter: u64,
    pub contract_version: u64,
    pub generation: u64,
    pub provenance: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completeness {
    Unknown,
    DeclaredComplete { inventory_generation: u64 },
}

/// A one-resource, one-principal projection. ALL Actor nodes are entry points;
/// ALL Sink nodes represent exits to target. Callers cannot select a convenient
/// subset of roots/sinks when asking for a certificate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphSpec {
    pub generation: u64,
    pub inventory_generation: u64,
    pub scope: Scope,
    pub target: ResolvedTarget,
    pub family: EffectFamilyRecord,
    pub completeness: Completeness,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub enforcers: Vec<Enforcer>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorityGraph(Rc<GraphSpec>);

/// Untrusted candidate partition. The checker independently recomputes it;
/// neither constructing nor editing this value supplies a verified cut.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CutProposal {
    pub graph: AuthorityGraph,
    pub gates: Vec<u64>,
    pub reachable: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathWitness { pub nodes: Vec<u64>, pub edges: Vec<u64> }

/// A conditional graph result, with exact input retention. No live authority,
/// authenticity, or empirical functioning of the declared enforcers follows.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::mediation::VerifiedCut;
/// use fa_reference::action::Permit;
/// fn elevate(cut: VerifiedCut) -> Permit { cut }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedCut {
    graph: AuthorityGraph,
    gates: Vec<u64>,
    reachable: Vec<u64>,
    edge_visits: usize,
}

impl VerifiedCut {
    pub fn graph(&self) -> &AuthorityGraph { &self.graph }
    pub fn gates(&self) -> &[u64] { &self.gates }
    pub fn reachable(&self) -> &[u64] { &self.reachable }
    /// Actual edge examinations by the independent checker, not latency or RAM.
    pub fn edge_visits(&self) -> usize { self.edge_visits }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CutCheck {
    Verified(VerifiedCut),
    Bypass(PathWitness),
    /// Disconnection is not credited as a working mediation boundary.
    Unreachable { sinks: Vec<u64> },
}

impl AuthorityGraph {
    pub fn new(mut spec: GraphSpec) -> Result<Self, Error> {
        if spec.nodes.len() > MAX_NODES || spec.edges.len() > MAX_EDGES
            || spec.enforcers.len() > MAX_CUT_NODES { return Err(Error::Limit); }
        spec.scope.validate()?;
        if spec.scope.purpose != Purpose::Effect || spec.generation == 0
            || spec.inventory_generation == 0 { return Err(Error::InvalidInput); }
        if [spec.target.adapter, spec.target.object, spec.target.contract_version,
            spec.target.expected_version, spec.target.generation].contains(&0)
        { return Err(Error::InvalidInput); }
        if spec.family.scope.tenant != spec.scope.tenant
            || spec.family.scope.principal != spec.scope.principal { return Err(Error::Binding); }
        // Reuse the existing complete family validator; do not silently accept
        // credentials or residual routes its own boundary rejects.
        let inventory = PerimeterInventory::new(vec![spec.family])?;
        spec.family = inventory.records()[0].clone();
        let mut nodes = BTreeMap::new();
        for node in &spec.nodes {
            if node.id == 0 { return Err(Error::InvalidInput); }
            if nodes.insert(node.id, node.kind).is_some() { return Err(Error::Duplicate); }
        }
        if !nodes.values().any(|kind| *kind == NodeKind::Actor)
            || !nodes.values().any(|kind| *kind == NodeKind::Sink) { return Err(Error::Incomplete); }
        let routes: BTreeSet<_> = spec.family.routes.iter().map(|r| r.route.as_str()).collect();
        let mut edge_ids = BTreeSet::new();
        for edge in &spec.edges {
            if edge.id == 0 || edge.provenance == 0 { return Err(Error::InvalidInput); }
            if !edge_ids.insert(edge.id) { return Err(Error::Duplicate); }
            if !nodes.contains_key(&edge.from) || !nodes.contains_key(&edge.to) { return Err(Error::Missing); }
            if !routes.contains(edge.route.as_str()) { return Err(Error::Binding); }
        }
        let mut gates = BTreeSet::new();
        for gate in &spec.enforcers {
            if gate.provenance == 0 { return Err(Error::InvalidInput); }
            if nodes.get(&gate.node) != Some(&NodeKind::Enforcer) { return Err(Error::Binding); }
            if !gates.insert(gate.node) { return Err(Error::Duplicate); }
            if gate.adapter != spec.target.adapter || gate.contract_version != spec.target.contract_version
                || gate.generation != spec.target.generation { return Err(Error::Binding); }
        }
        // Stable ordering does not collapse parallel edges or their labels.
        spec.nodes.sort_by_key(|node| node.id);
        spec.edges.sort_by_key(|edge| edge.id);
        spec.enforcers.sort_by_key(|gate| gate.node);
        Ok(Self(Rc::new(spec)))
    }

    pub fn spec(&self) -> &GraphSpec { &self.0 }

    /// The topology binds resource identity, not its changing content version.
    /// The endpoint independently enforces each frozen expected_version.
    pub fn binds(&self, scope: Scope, target: ResolvedTarget) -> bool {
        let expected = self.0.target;
        self.0.scope == scope && expected.adapter == target.adapter
            && expected.object == target.object && expected.contract_version == target.contract_version
            && expected.generation == target.generation
    }

    fn gates(&self, gates: &[u64]) -> Result<BTreeSet<u64>, Error> {
        if gates.is_empty() { return Err(Error::InvalidInput); }
        if gates.len() > MAX_CUT_NODES { return Err(Error::Limit); }
        let mut unique = BTreeSet::new();
        for gate in gates {
            if !unique.insert(*gate) { return Err(Error::Duplicate); }
            if !self.0.enforcers.iter().any(|enforcer| enforcer.node == *gate) { return Err(Error::Binding); }
        }
        Ok(unique)
    }

    fn admitted_inventory(&self) -> Result<(), Error> {
        match self.0.completeness {
            Completeness::Unknown => return Err(Error::Incomplete),
            Completeness::DeclaredComplete { inventory_generation } => {
                if inventory_generation != self.0.inventory_generation { return Err(Error::Stale); }
            }
        }
        for route in &self.0.family.routes {
            if route.mediation != Mediation::BrokeredEffects || route.bypass != BypassDisposition::Blocked {
                return Err(Error::Binding);
            }
            if !self.0.edges.iter().any(|edge| edge.route == route.route) { return Err(Error::Incomplete); }
        }
        Ok(())
    }

    /// Sparse BFS proposes a partition; it is not the independent verifier.
    /// A bypass still produces a candidate so the checker can explain the path.
    pub fn propose_cut(&self, gates: &[u64]) -> Result<CutProposal, Error> {
        let removed = self.gates(gates)?;
        let mut outgoing: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
        for edge in &self.0.edges { outgoing.entry(edge.from).or_default().push(edge.to); }
        let mut reachable = BTreeSet::new();
        let mut queue = VecDeque::new();
        for node in &self.0.nodes {
            if node.kind == NodeKind::Actor { reachable.insert(node.id); queue.push_back(node.id); }
        }
        while let Some(from) = queue.pop_front() {
            for to in outgoing.get(&from).into_iter().flatten() {
                if !removed.contains(to) && reachable.insert(*to) { queue.push_back(*to); }
            }
        }
        Ok(CutProposal { graph: self.clone(), gates: removed.into_iter().collect(),
            reachable: reachable.into_iter().collect() })
    }

    /// Independent bounded edge-scan fixed point, not the BFS above. Checks the
    /// original graph first so an unreachable target is not a successful cut.
    /// The budget covers BOTH scans and exhaustion can never issue a certificate.
    pub fn verify_cut(&self, proposal: &CutProposal, edge_budget: usize) -> Result<CutCheck, Error> {
        if self != &proposal.graph { return Err(Error::Binding); }
        if edge_budget == 0 || edge_budget > MAX_CHECK_EDGE_VISITS { return Err(Error::Limit); }
        if proposal.reachable.len() > MAX_NODES { return Err(Error::Limit); }
        let removed = self.gates(&proposal.gates)?;
        let claimed: BTreeSet<_> = proposal.reachable.iter().copied().collect();
        if claimed.len() != proposal.reachable.len() { return Err(Error::Duplicate); }
        if claimed.iter().any(|id| !self.0.nodes.iter().any(|node| node.id == *id)) { return Err(Error::Missing); }
        self.admitted_inventory()?;
        let mut visits = 0;
        let original = self.scan(&BTreeSet::new(), &mut visits, edge_budget)?;
        let unreachable: Vec<_> = self.0.nodes.iter().filter(|node| {
            node.kind == NodeKind::Sink && !original.contains_key(&node.id)
        }).map(|node| node.id).collect();
        if !unreachable.is_empty() { return Ok(CutCheck::Unreachable { sinks: unreachable }); }
        let reached = self.scan(&removed, &mut visits, edge_budget)?;
        if let Some(sink) = self.0.nodes.iter().find(|node| node.kind == NodeKind::Sink && reached.contains_key(&node.id)) {
            let mut nodes = vec![sink.id];
            let mut edges = Vec::new();
            let mut current = sink.id;
            while let Some((from, edge)) = reached[&current] {
                nodes.push(from); edges.push(edge); current = from;
            }
            nodes.reverse(); edges.reverse();
            return Ok(CutCheck::Bypass(PathWitness { nodes, edges }));
        }
        if claimed != reached.keys().copied().collect() { return Err(Error::Binding); }
        Ok(CutCheck::Verified(VerifiedCut { graph: self.clone(), gates: removed.into_iter().collect(),
            reachable: claimed.into_iter().collect(), edge_visits: visits }))
    }

    fn scan(&self, removed: &BTreeSet<u64>, visits: &mut usize, budget: usize)
        -> Result<BTreeMap<u64, Option<(u64, u64)>>, Error>
    {
        let mut reached = BTreeMap::new();
        for node in &self.0.nodes {
            if node.kind == NodeKind::Actor { reached.insert(node.id, None); }
        }
        loop {
            let mut changed = false;
            for edge in &self.0.edges {
                if *visits == budget { return Err(Error::Limit); }
                *visits += 1;
                if !removed.contains(&edge.from) && !removed.contains(&edge.to)
                    && reached.contains_key(&edge.from) && !reached.contains_key(&edge.to)
                { reached.insert(edge.to, Some((edge.from, edge.id))); changed = true; }
            }
            if !changed { return Ok(reached); }
        }
    }
}
