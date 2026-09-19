use super::*;
use crate::action::{Purpose, ResolvedTarget, Scope};
use crate::action::consequence::mediation::{Channel, Completeness, CutCheck, Edge, Enforcer, Node,
    MAX_CHECK_EDGE_VISITS};
use crate::perimeter::{BypassDisposition, CredentialExposure, CredentialHolder, EffectFamilyRecord,
    Mediation, PerimeterScope, RouteRecord, ThreatClass, TrustDomain};
use std::collections::BTreeSet;

fn topology(nodes: &[(u64, NodeKind)], edges: &[(u64, u64)], gates: &[u64]) -> AuthorityGraph {
    let scope = Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect };
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    AuthorityGraph::new(GraphSpec {
        generation: 1, inventory_generation: 1, scope, target,
        family: EffectFamilyRecord {
            scope: PerimeterScope { tenant: 1, principal: 2, purpose: 1 }, family: "publication".into(),
            trust_domains: vec![TrustDomain::Actor, TrustDomain::ObservationAndAnalysis,
                TrustDomain::Enforcement, TrustDomain::GovernanceAndInvestigation],
            credentials: vec![CredentialExposure { credential: "file-owner".into(), holder: CredentialHolder::Broker }],
            routes: vec![RouteRecord { route: "publish".into(), threat: Some(ThreatClass::DirectCredentialOrEgress),
                mediation: Mediation::BrokeredEffects, bypass: BypassDisposition::Blocked }],
            residual_nonclaims: vec!["Declared graph, not operating-system containment".into()],
        },
        completeness: Completeness::DeclaredComplete { inventory_generation: 1 },
        nodes: nodes.iter().map(|&(id, kind)| Node { id, kind }).collect(),
        edges: edges.iter().enumerate().map(|(i, &(from, to))| Edge { id: i as u64 + 1, from, to,
            channel: Channel::Dispatch, route: "publish".into(), provenance: i as u64 + 100 }).collect(),
        enforcers: gates.iter().map(|&node| Enforcer { node, adapter: 10, contract_version: 1,
            generation: 1, provenance: node + 100 }).collect(),
    }).unwrap()
}
fn prices(values: &[(u64, u64)]) -> Vec<EnforcerCost> {
    values.iter().map(|&(node, units)| EnforcerCost { node, units }).collect()
}
fn candidate(plan: CutPlan) -> PlannedCut {
    let CutPlanOutcome::Candidate(candidate) = plan.outcome else { panic!("expected a usable cut candidate"); };
    assert!(matches!(candidate.proposal.graph.verify_cut(&candidate.proposal, MAX_CHECK_EDGE_VISITS),
        Ok(CutCheck::Verified(_))));
    candidate
}
fn serial() -> AuthorityGraph {
    topology(&[(1, NodeKind::Actor), (2, NodeKind::Enforcer), (3, NodeKind::Enforcer), (4, NodeKind::Sink)],
        &[(1, 2), (2, 3), (3, 4)], &[2, 3])
}

#[test]
fn chooses_cheapest_serial_enforcer_and_the_original_checker_verifies_it() {
    let graph = serial();
    let best = candidate(graph.plan_minimum_cut(&prices(&[(2, 9), (3, 2)]), CutPlanningLimits::default()).unwrap());
    assert_eq!(best.total_cost, 2);
    assert_eq!(best.proposal.gates, vec![3]);
    assert_eq!(best.proposal.reachable, vec![1, 2]);
    assert_eq!(best.proposal.graph, graph);
}

#[test]
fn cheaper_joint_cut_beats_one_expensive_shared_gate_and_prices_can_reverse_that() {
    let graph = topology(&[(1, NodeKind::Actor), (2, NodeKind::Enforcer), (3, NodeKind::Enforcer),
        (4, NodeKind::Enforcer), (5, NodeKind::Sink)], &[(1, 2), (1, 3), (2, 4), (3, 4), (4, 5)], &[2, 3, 4]);
    let joint = candidate(graph.plan_minimum_cut(&prices(&[(2, 2), (3, 3), (4, 8)]), CutPlanningLimits::default()).unwrap());
    assert_eq!((joint.proposal.gates, joint.total_cost), (vec![2, 3], 5));
    let shared = candidate(graph.plan_minimum_cut(&prices(&[(2, 2), (3, 3), (4, 4)]), CutPlanningLimits::default()).unwrap());
    assert_eq!((shared.proposal.gates, shared.total_cost), (vec![4], 4));
}

#[test]
fn all_roots_and_sinks_parallel_reverse_and_self_loop_channels_are_preserved() {
    let graph = topology(&[(1, NodeKind::Actor), (2, NodeKind::Actor), (3, NodeKind::Enforcer),
        (4, NodeKind::Enforcer), (5, NodeKind::Sink), (6, NodeKind::Sink)],
        &[(1, 3), (1, 3), (2, 4), (3, 3), (3, 4), (4, 3), (3, 5), (4, 6)], &[3, 4]);
    let plan = graph.plan_minimum_cut(&prices(&[(3, 5), (4, 7)]), CutPlanningLimits::default()).unwrap();
    assert_eq!(plan.work.projection_edges, 8);
    assert_eq!(plan.work.residual_arcs, 2 * (6 + 8 + 4));
    let best = candidate(plan);
    assert_eq!(best.proposal.gates, vec![3, 4]);
    assert_eq!(best.total_cost, 12);
    assert_eq!(best.proposal.reachable, vec![1, 2]);
    assert_eq!(best.proposal.graph.spec().edges.len(), 8);
}

#[test]
fn augmenting_paths_repair_an_earlier_choice_through_reverse_residual_arcs() {
    let mut residual = Residual::new(6);
    for (from, to) in [(0, 1), (0, 2), (1, 3), (1, 4), (2, 3), (3, 5), (4, 5)] {
        residual.add(from, to, 1);
    }
    let mut work = CutPlanningWork::default();
    assert_eq!(residual.maximum_flow(0, 5, 2, &mut work, MAX_PLANNING_EDGE_VISITS).unwrap().0, 2);
    assert_eq!(work.augmentations, 2);
    // The first BFS used 1->3. A forward-only greedy algorithm cannot reach 2.
    assert_eq!(residual.rows[1].iter().find(|arc| arc.to == 3).unwrap().remaining, 1);
}

#[test]
fn bypass_names_real_original_edges_and_unregistered_enforcers_are_not_cuttable() {
    let graph = topology(&[(1, NodeKind::Actor), (2, NodeKind::Enforcer), (3, NodeKind::Enforcer),
        (4, NodeKind::Sink)], &[(1, 2), (2, 4), (1, 3), (3, 4)], &[2]);
    let plan = graph.plan_minimum_cut(&prices(&[(2, 1)]), CutPlanningLimits::default()).unwrap();
    assert_eq!(plan.outcome, CutPlanOutcome::Bypass(PathWitness { nodes: vec![1, 3, 4], edges: vec![3, 4] }));
    assert_eq!(plan.work.residual_arcs, 0);
    assert!(matches!(graph.verify_cut(&graph.propose_cut(&[2]).unwrap(), MAX_CHECK_EDGE_VISITS), Ok(CutCheck::Bypass(_))));
    assert_eq!(graph.plan_minimum_cut(&prices(&[(2, 1), (3, 1)]), CutPlanningLimits::default()), Err(Error::Binding));
}

#[test]
fn unreachable_sink_is_not_vacuous_mediation_and_no_gate_graph_exposes_a_bypass() {
    let graph = topology(&[(1, NodeKind::Actor), (2, NodeKind::Enforcer), (3, NodeKind::Sink),
        (4, NodeKind::Sink)], &[(1, 2), (2, 3)], &[2]);
    let plan = graph.plan_minimum_cut(&prices(&[(2, 1)]), CutPlanningLimits::default()).unwrap();
    assert_eq!(plan.outcome, CutPlanOutcome::Unreachable { sinks: vec![4] });
    assert_eq!(plan.work.residual_vertices, 0);
    let unmediated = topology(&[(1, NodeKind::Actor), (2, NodeKind::Sink)], &[(1, 2)], &[]);
    assert!(matches!(unmediated.plan_minimum_cut(&[], CutPlanningLimits::default()).unwrap().outcome, CutPlanOutcome::Bypass(_)));
}

#[test]
fn missing_duplicate_zero_or_overflowing_prices_never_default_to_a_cheap_gate() {
    let graph = serial();
    for (costs, error) in [
        (prices(&[(2, 1)]), Error::Incomplete),
        (prices(&[(2, 1), (2, 2)]), Error::Duplicate),
        (prices(&[(2, 0), (3, 2)]), Error::InvalidInput),
        (prices(&[(2, 1), (4, 2)]), Error::Binding),
        (prices(&[(2, u64::MAX), (3, 1)]), Error::Overflow),
        (prices(&[(2, u64::MAX - 1), (3, 1)]), Error::Overflow),
    ] { assert_eq!(graph.plan_minimum_cut(&costs, CutPlanningLimits::default()), Err(error)); }
    let best = candidate(graph.plan_minimum_cut(&prices(&[(2, u64::MAX - 2), (3, 1)]), CutPlanningLimits::default()).unwrap());
    assert_eq!(best.total_cost, 1);
}

#[test]
fn unknown_or_stale_inventory_and_uncovered_routes_refuse_before_planning() {
    let source = serial();
    for (completeness, error) in [(Completeness::Unknown, Error::Incomplete),
        (Completeness::DeclaredComplete { inventory_generation: 2 }, Error::Stale)] {
        let mut spec = source.spec().clone(); spec.completeness = completeness;
        let graph = AuthorityGraph::new(spec).unwrap();
        assert_eq!(graph.plan_minimum_cut(&prices(&[(2, 1), (3, 1)]), CutPlanningLimits::default()), Err(error));
    }
    let mut spec = source.spec().clone();
    let mut missing_route = spec.family.routes[0].clone();
    missing_route.route = "unmodeled-second-route".into();
    spec.family.routes.push(missing_route);
    let graph = AuthorityGraph::new(spec).unwrap();
    assert_eq!(graph.plan_minimum_cut(&prices(&[(2, 1), (3, 1)]), CutPlanningLimits::default()), Err(Error::Incomplete));
    assert!(source.plan_minimum_cut(&prices(&[(2, 1), (3, 1)]), CutPlanningLimits::default()).is_ok());
}

#[test]
fn exact_sparse_memory_and_work_limits_have_one_over_refusal_controls() {
    let graph = serial(); let costs = prices(&[(2, 9), (3, 2)]);
    let complete = graph.plan_minimum_cut(&costs, CutPlanningLimits::default()).unwrap();
    let limits = CutPlanningLimits { residual_arcs: complete.work.residual_arcs, edge_visits: complete.work.edge_visits };
    assert_eq!(graph.plan_minimum_cut(&costs, limits).unwrap(), complete);
    assert_eq!(graph.plan_minimum_cut(&costs, CutPlanningLimits { edge_visits: limits.edge_visits - 1, ..limits }), Err(Error::Limit));
    assert_eq!(graph.plan_minimum_cut(&costs, CutPlanningLimits { residual_arcs: limits.residual_arcs - 1, ..limits }), Err(Error::Limit));
    assert_eq!(graph.plan_minimum_cut(&costs, CutPlanningLimits { residual_arcs: MAX_RESIDUAL_ARCS + 1, ..limits }), Err(Error::Limit));
    assert_eq!(graph.plan_minimum_cut(&costs, CutPlanningLimits { edge_visits: 0, ..limits }), Err(Error::Limit));
    assert_eq!(graph, serial());
}

#[test]
fn canonical_input_order_makes_ties_deterministic_and_candidate_edits_stay_untrusted() {
    let source = serial();
    let mut spec = source.spec().clone(); spec.nodes.reverse(); spec.edges.reverse(); spec.enforcers.reverse();
    let reordered = AuthorityGraph::new(spec).unwrap();
    let costs = prices(&[(2, 2), (3, 2)]);
    let plan = source.plan_minimum_cut(&costs, CutPlanningLimits::default()).unwrap();
    assert_eq!(plan, reordered.plan_minimum_cut(&prices(&[(3, 2), (2, 2)]), CutPlanningLimits::default()).unwrap());
    let mut candidate = candidate(plan); candidate.proposal.reachable.push(4);
    assert_eq!(source.verify_cut(&candidate.proposal, MAX_CHECK_EDGE_VISITS), Err(Error::Binding));
    let mut changed = source.spec().clone(); changed.generation += 1;
    let changed = AuthorityGraph::new(changed).unwrap();
    assert_eq!(changed.verify_cut(&candidate.proposal, MAX_CHECK_EDGE_VISITS), Err(Error::Binding));
}

// Independent fixed-point set oracle. No split graph, residual flow, planner
// reachability or cut-checker code is used to decide the optimal subset here.
fn oracle_reachable(spec: &GraphSpec, removed: &BTreeSet<u64>) -> BTreeSet<u64> {
    let mut reached: BTreeSet<_> = spec.nodes.iter().filter(|node| node.kind == NodeKind::Actor).map(|node| node.id).collect();
    loop {
        let before = reached.clone();
        for edge in &spec.edges {
            if !removed.contains(&edge.from) && !removed.contains(&edge.to) && before.contains(&edge.from) { reached.insert(edge.to); }
        }
        if reached == before { return reached; }
    }
}

#[test]
fn every_five_vertex_dag_matches_exhaustive_weighted_subsets_and_independent_cut_check() {
    let nodes = [(1, NodeKind::Actor), (2, NodeKind::Enforcer), (3, NodeKind::Enforcer),
        (4, NodeKind::Enforcer), (5, NodeKind::Sink)];
    let all_edges: Vec<_> = (1..=5).flat_map(|from| (from + 1..=5).map(move |to| (from, to))).collect();
    for mask in 1..(1_usize << all_edges.len()) {
        let edges: Vec<_> = all_edges.iter().enumerate().filter(|(i, _)| mask & (1_usize << *i) != 0).map(|(_, &edge)| edge).collect();
        let graph = topology(&nodes, &edges, &[2, 3, 4]);
        for weights in [[1, 2, 3], [3, 2, 1], [2, 2, 2]] {
            let costs = prices(&[(2, weights[0]), (3, weights[1]), (4, weights[2])]);
            let plan = graph.plan_minimum_cut(&costs, CutPlanningLimits::default()).unwrap();
            if !oracle_reachable(graph.spec(), &BTreeSet::new()).contains(&5) {
                assert_eq!(plan.outcome, CutPlanOutcome::Unreachable { sinks: vec![5] });
                continue;
            }
            let optimum = (0_u8..8).filter_map(|subset| {
                let removed: BTreeSet<_> = (0_usize..3).filter(|i| subset & (1_u8 << *i) != 0).map(|i| i as u64 + 2).collect();
                (!oracle_reachable(graph.spec(), &removed).contains(&5)).then(||
                    costs.iter().filter(|cost| removed.contains(&cost.node)).map(|cost| cost.units).sum::<u64>())
            }).min();
            match optimum {
                Some(cost) => assert_eq!(candidate(plan).total_cost, cost, "edge mask {mask}, weights {weights:?}"),
                None => {
                    let CutPlanOutcome::Bypass(path) = plan.outcome else { panic!("must expose uncovered path"); };
                    assert_eq!(path.nodes.first(), Some(&1)); assert_eq!(path.nodes.last(), Some(&5));
                    assert!(!path.nodes.iter().any(|id| [2, 3, 4].contains(id)));
                    for (edge, pair) in path.edges.iter().zip(path.nodes.windows(2)) {
                        assert!(graph.spec().edges.iter().any(|original|
                            original.id == *edge && original.from == pair[0] && original.to == pair[1]));
                    }
                }
            }
        }
    }
}
