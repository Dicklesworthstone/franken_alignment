//! Public graph controls; no fixture is evidence of a real operating perimeter.
#[path = "support/mediation.rs"]
mod support;

use support::{edge, graph, scope, spec, target};
use fa_reference::action::consequence::mediation::*;
use fa_reference::perimeter::{BypassDisposition, Mediation, RouteRecord, ThreatClass};
use fa_reference::Error;
use std::collections::BTreeSet;

fn verify(graph: &AuthorityGraph, gates: &[u64]) -> Result<CutCheck, Error> {
    graph.verify_cut(&graph.propose_cut(gates)?, MAX_CHECK_EDGE_VISITS)
}

#[test]
fn verified_cut_retains_exact_scope_channels_parallel_edges_and_partition() {
    let mut spec = spec(1);
    spec.edges.push(Edge { channel: Channel::Ipc, ..edge(8, 2, 3) });
    spec.edges.reverse();
    let graph = AuthorityGraph::new(spec).unwrap();
    let CutCheck::Verified(cut) = verify(&graph, &[3]).unwrap() else { panic!("valid cut"); };
    assert_eq!(cut.graph(), &graph);
    assert_eq!(cut.gates(), &[3]);
    assert_eq!(cut.reachable(), &[1, 2]);
    assert_eq!(cut.graph().spec().edges.len(), 5);
    assert_eq!(cut.graph().spec().edges[4].channel, Channel::Ipc);
    assert_eq!(cut.graph().spec().edges[4].provenance, 18);
    assert!(cut.edge_visits() > 0);
    assert!(graph.binds(scope(), target()));
    let mut newer_content = target(); newer_content.expected_version = 2;
    assert!(graph.binds(scope(), newer_content));
    newer_content.generation += 1;
    assert!(!graph.binds(scope(), newer_content));
}

#[test]
fn bypass_and_unreachable_are_different_from_a_verified_cut() {
    let original = graph(1);
    assert!(matches!(verify(&original, &[3]), Ok(CutCheck::Verified(_))));
    let mut spec = original.spec().clone();
    spec.edges.push(edge(9, 2, 5));
    let bypass = AuthorityGraph::new(spec).unwrap();
    assert_eq!(verify(&bypass, &[3]), Ok(CutCheck::Bypass(PathWitness {
        nodes: vec![1, 2, 5], edges: vec![1, 9],
    })));
    let mut spec = original.spec().clone(); spec.edges.retain(|edge| edge.id != 4);
    let disconnected = AuthorityGraph::new(spec).unwrap();
    assert_eq!(verify(&disconnected, &[3]), Ok(CutCheck::Unreachable { sinks: vec![5] }));
}

#[test]
fn every_actor_entry_and_every_sink_is_checked() {
    let mut spec = spec(1);
    spec.nodes.push(Node { id: 6, kind: NodeKind::Actor });
    spec.edges.push(edge(6, 6, 5));
    let graph = AuthorityGraph::new(spec).unwrap();
    assert_eq!(verify(&graph, &[3]), Ok(CutCheck::Bypass(PathWitness { nodes: vec![6, 5], edges: vec![6] })));
    let mut spec = support::spec(1);
    spec.nodes.push(Node { id: 7, kind: NodeKind::Sink });
    spec.edges.push(edge(7, 2, 7));
    let graph = AuthorityGraph::new(spec).unwrap();
    assert_eq!(verify(&graph, &[3]), Ok(CutCheck::Bypass(PathWitness { nodes: vec![1, 2, 7], edges: vec![1, 7] })));
}

#[test]
fn incomplete_stale_or_partially_uncovered_inventory_never_certifies() {
    let mut source = spec(1); source.completeness = Completeness::Unknown;
    assert_eq!(verify(&AuthorityGraph::new(source).unwrap(), &[3]), Err(Error::Incomplete));
    let mut source = spec(1); source.completeness = Completeness::DeclaredComplete { inventory_generation: 2 };
    assert_eq!(verify(&AuthorityGraph::new(source).unwrap(), &[3]), Err(Error::Stale));
    let mut source = spec(1);
    source.family.routes.push(RouteRecord { route: "alternate".into(), threat: Some(ThreatClass::PromptInjection),
        mediation: Mediation::ObserveOnly, bypass: BypassDisposition::ResidualUncovered });
    assert_eq!(verify(&AuthorityGraph::new(source).unwrap(), &[3]), Err(Error::Binding));
    let mut source = spec(1); let mut additional = source.family.routes[0].clone();
    additional.route = "unprojected".into(); source.family.routes.push(additional);
    assert_eq!(verify(&AuthorityGraph::new(source).unwrap(), &[3]), Err(Error::Incomplete));
}

#[test]
fn forged_partitions_and_trivial_or_unregistered_cuts_are_refused() {
    let graph = graph(1);
    let proposal = graph.propose_cut(&[3]).unwrap();
    let mut forged = proposal.clone(); forged.reachable.clear();
    assert_eq!(graph.verify_cut(&forged, MAX_CHECK_EDGE_VISITS), Err(Error::Binding));
    let mut forged = proposal.clone(); forged.reachable.push(1);
    assert_eq!(graph.verify_cut(&forged, MAX_CHECK_EDGE_VISITS), Err(Error::Duplicate));
    let mut forged = proposal; forged.reachable.push(99);
    assert_eq!(graph.verify_cut(&forged, MAX_CHECK_EDGE_VISITS), Err(Error::Missing));
    for invalid in [1, 2, 4, 5, 99] { assert_eq!(graph.propose_cut(&[invalid]), Err(Error::Binding)); }
    assert_eq!(graph.propose_cut(&[3, 3]), Err(Error::Duplicate));
    assert_eq!(graph.propose_cut(&[]), Err(Error::InvalidInput));
    let mut source = spec(1); source.enforcers.clear();
    assert_eq!(AuthorityGraph::new(source).unwrap().propose_cut(&[3]), Err(Error::Binding));
}

#[test]
fn equal_generation_does_not_substitute_different_content_provenance_or_scope() {
    let original = graph(1); let proposal = original.propose_cut(&[3]).unwrap();
    let mut changed = original.spec().clone(); changed.edges[0].provenance += 1;
    assert_eq!(AuthorityGraph::new(changed).unwrap().verify_cut(&proposal, MAX_CHECK_EDGE_VISITS), Err(Error::Binding));
    let mut changed = original.spec().clone(); changed.scope.tenant = 9; changed.family.scope.tenant = 9;
    assert_eq!(AuthorityGraph::new(changed).unwrap().verify_cut(&proposal, MAX_CHECK_EDGE_VISITS), Err(Error::Binding));
    assert_eq!(graph(2).verify_cut(&proposal, MAX_CHECK_EDGE_VISITS), Err(Error::Binding));
    let mut wrong = spec(1); wrong.enforcers[0].generation = 2;
    assert_eq!(AuthorityGraph::new(wrong).unwrap_err(), Error::Binding);
}

#[test]
fn projection_and_exact_checker_work_bounds_are_enforced() {
    let mut source = spec(1);
    for id in 6..=MAX_NODES as u64 { source.nodes.push(Node { id, kind: NodeKind::Process }); }
    for id in 5..=MAX_EDGES as u64 { source.edges.push(edge(id, 2, 3)); }
    let graph = AuthorityGraph::new(source.clone()).unwrap();
    let proposal = graph.propose_cut(&[3]).unwrap();
    let CutCheck::Verified(cut) = graph.verify_cut(&proposal, MAX_CHECK_EDGE_VISITS).unwrap() else { panic!("valid cut"); };
    assert!(matches!(graph.verify_cut(&proposal, cut.edge_visits()), Ok(CutCheck::Verified(_))));
    assert_eq!(graph.verify_cut(&proposal, cut.edge_visits() - 1), Err(Error::Limit));
    source.nodes.push(Node { id: MAX_NODES as u64 + 1, kind: NodeKind::Process });
    assert_eq!(AuthorityGraph::new(source.clone()).unwrap_err(), Error::Limit);
    source.nodes.pop(); source.edges.push(edge(MAX_EDGES as u64 + 1, 2, 3));
    assert_eq!(AuthorityGraph::new(source).unwrap_err(), Error::Limit);
    assert_eq!(graph.verify_cut(&proposal, 0), Err(Error::Limit));
}

fn reaches_sink(graph: &GraphSpec, node: u64, seen: &mut BTreeSet<u64>) -> bool {
    if node == 3 || !seen.insert(node) { return false; }
    if graph.nodes.iter().any(|candidate| candidate.id == node && candidate.kind == NodeKind::Sink) { return true; }
    graph.edges.iter().filter(|edge| edge.from == node).any(|edge| reaches_sink(graph, edge.to, seen))
}

#[test]
fn independent_public_dfs_matches_cuts_with_cycles_reverse_edges_and_bypasses() {
    let extras = [(2, 5), (1, 4), (4, 2), (2, 2), (5, 1), (1, 3), (3, 5), (4, 1)];
    for mask in 0_u16..256 {
        let mut source = spec(1);
        for (index, (from, to)) in extras.iter().enumerate() {
            if mask & (1 << index) != 0 { source.edges.push(edge(index as u64 + 10, *from, *to)); }
        }
        let bypass = reaches_sink(&source, 1, &mut BTreeSet::new());
        let graph = AuthorityGraph::new(source).unwrap();
        match verify(&graph, &[3]).unwrap() {
            CutCheck::Verified(_) => assert!(!bypass, "mask {mask}"),
            CutCheck::Bypass(path) => {
                assert!(bypass, "mask {mask}");
                assert_eq!(path.nodes.first(), Some(&1)); assert_eq!(path.nodes.last(), Some(&5));
                assert!(!path.nodes.contains(&3)); assert_eq!(path.edges.len() + 1, path.nodes.len());
                for (pair, edge_id) in path.nodes.windows(2).zip(&path.edges) {
                    assert!(graph.spec().edges.iter().any(|edge| edge.id == *edge_id && edge.from == pair[0] && edge.to == pair[1]));
                }
            }
            CutCheck::Unreachable { .. } => panic!("original chain remains connected"),
        }
    }
}
