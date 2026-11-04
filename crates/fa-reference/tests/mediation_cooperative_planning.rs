//! Public sparse planner controls: original synchronous solver and independent
//! native checker, plus exhaustive subsets without either optimizer's network.
use fa_reference::Error;
use fa_reference::action::{Purpose, ResolvedTarget, Scope};
use fa_reference::action::consequence::mediation::{AuthorityGraph, Channel, Completeness,
    CutCheck, Edge, Enforcer, GraphSpec, Node, NodeKind, MAX_CHECK_EDGE_VISITS};
use fa_reference::action::consequence::mediation::planning::{CutPlan, CutPlanOutcome,
    CutPlanningCursor, CutPlanningLimits, CutPlanningStatus, EnforcerCost};
use fa_reference::perimeter::{BypassDisposition, CredentialExposure, CredentialHolder,
    EffectFamilyRecord, Mediation, PerimeterScope, RouteRecord, ThreatClass, TrustDomain};
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
fn serial() -> AuthorityGraph {
    topology(&[(1, NodeKind::Actor), (2, NodeKind::Enforcer), (3, NodeKind::Enforcer), (4, NodeKind::Sink)],
        &[(1, 2), (2, 3), (3, 4)], &[2, 3])
}
fn finish(mut run: CutPlanningCursor, quantum: usize) -> Result<CutPlan, Error> {
    for _ in 0..1_000_000 {
        let before = run.progress();
        let budget = quantum.max(before.minimum_edge_visits);
        let after = run.advance(budget)?;
        assert!(after.work.edge_visits - before.work.edge_visits <= budget);
        if after.status == CutPlanningStatus::Ready {
            assert_eq!(after.minimum_edge_visits, 0);
            assert_eq!(run.advance(0).unwrap(), after);
            return run.finish();
        }
        assert!(after.work.edge_visits > before.work.edge_visits);
    }
    panic!("admitted bounded planner did not terminate");
}
fn reachable(spec: &GraphSpec, blocked: &BTreeSet<u64>) -> BTreeSet<u64> {
    let mut seen: BTreeSet<_> = spec.nodes.iter().filter(|node| node.kind == NodeKind::Actor)
        .map(|node| node.id).collect();
    loop {
        let previous = seen.clone();
        for edge in &spec.edges {
            if previous.contains(&edge.from) && !blocked.contains(&edge.from) && !blocked.contains(&edge.to) {
                seen.insert(edge.to);
            }
        }
        if seen == previous { return seen; }
    }
}

#[test]
fn every_five_vertex_dag_preserves_optimum_partitions_and_exact_original_work() {
    let nodes = [(1, NodeKind::Actor), (2, NodeKind::Enforcer), (3, NodeKind::Enforcer),
        (4, NodeKind::Enforcer), (5, NodeKind::Sink)];
    let all: Vec<_> = (1..=5).flat_map(|from| (from + 1..=5).map(move |to| (from, to))).collect();
    for mask in 1..(1_usize << all.len()) {
        let edges: Vec<_> = all.iter().enumerate().filter(|(i, _)| mask & (1_usize << *i) != 0)
            .map(|(_, &edge)| edge).collect();
        let graph = topology(&nodes, &edges, &[2, 3, 4]);
        for weights in [[1, 2, 3], [3, 2, 1], [2, 2, 2]] {
            let costs = prices(&[(2, weights[0]), (3, weights[1]), (4, weights[2])]);
            let expected = graph.plan_minimum_cut(&costs, CutPlanningLimits::default()).unwrap();
            for quantum in [2, 7, usize::MAX] {
                let plan = finish(graph.begin_minimum_cut(&costs, CutPlanningLimits::default()).unwrap(), quantum).unwrap();
                assert_eq!(plan, expected, "mask={mask}, weights={weights:?}, quantum={quantum}");
            }
            if let CutPlanOutcome::Candidate(plan) = expected.outcome {
                let optimum = (0_u8..8).filter_map(|subset| {
                    let removed: BTreeSet<_> = (0_usize..3).filter(|i| subset & (1_u8 << *i) != 0)
                        .map(|i| i as u64 + 2).collect();
                    (!reachable(graph.spec(), &removed).contains(&5)).then(|| costs.iter()
                        .filter(|cost| removed.contains(&cost.node)).map(|cost| cost.units).sum::<u64>())
                }).min().unwrap();
                assert_eq!(plan.total_cost, optimum);
                assert!(matches!(graph.verify_cut(&plan.proposal, MAX_CHECK_EDGE_VISITS), Ok(CutCheck::Verified(_))));
            }
        }
    }
}

#[test]
fn zero_and_one_visit_polls_cannot_half_update_a_residual_pair() {
    let graph = serial(); let costs = prices(&[(2, 9), (3, 2)]);
    let mut run = graph.begin_minimum_cut(&costs, CutPlanningLimits::default()).unwrap();
    let initial = run.progress(); assert_eq!(initial.work.edge_visits, 0);
    assert_eq!(run.advance(0).unwrap(), initial);
    loop {
        let report = run.advance(1).unwrap();
        assert_eq!(report.status, CutPlanningStatus::Planning);
        if report.minimum_edge_visits == 2 { break; }
    }
    let before = run.progress();
    for _ in 0..5 { assert_eq!(run.advance(1).unwrap(), before); }
    let after = run.advance(2).unwrap();
    assert_eq!(after.work.edge_visits, before.work.edge_visits + 2);
    assert_eq!(finish(run, 1).unwrap(), graph.plan_minimum_cut(&costs, CutPlanningLimits::default()).unwrap());
}

#[test]
fn exact_lifetime_succeeds_and_every_smaller_ceiling_stays_exhausted_across_polls() {
    let graph = serial(); let costs = prices(&[(2, 9), (3, 2)]);
    let expected = graph.plan_minimum_cut(&costs, CutPlanningLimits::default()).unwrap();
    let exact = CutPlanningLimits { residual_arcs: expected.work.residual_arcs, edge_visits: expected.work.edge_visits };
    assert_eq!(finish(graph.begin_minimum_cut(&costs, exact).unwrap(), 2).unwrap(), expected);
    for limit in 1..exact.edge_visits {
        let limits = CutPlanningLimits { edge_visits: limit, ..exact };
        assert_eq!(graph.plan_minimum_cut(&costs, limits), Err(Error::Limit));
        let mut run = graph.begin_minimum_cut(&costs, limits).unwrap();
        loop { if run.advance(3).is_err() { break; } }
        let report = run.progress();
        assert_eq!(report.status, CutPlanningStatus::Failed(Error::Limit));
        assert!(report.work.edge_visits <= limit);
        assert_eq!(run.advance(usize::MAX), Err(Error::Limit));
        assert_eq!(run.progress(), report);
        assert_eq!(run.finish(), Err(Error::Limit));
    }
    assert!(matches!(graph.begin_minimum_cut(&costs,
        CutPlanningLimits { residual_arcs: exact.residual_arcs - 1, ..exact }), Err(Error::Limit)));
}

#[test]
fn all_roots_sinks_and_parallel_reverse_self_loop_channels_survive_every_polling_boundary() {
    let graph = topology(&[(1, NodeKind::Actor), (2, NodeKind::Actor), (3, NodeKind::Enforcer),
        (4, NodeKind::Enforcer), (5, NodeKind::Sink), (6, NodeKind::Sink)],
        &[(1, 3), (1, 3), (2, 4), (3, 3), (3, 4), (4, 3), (3, 5), (4, 6)], &[3, 4]);
    let costs = prices(&[(3, 5), (4, 7)]);
    let expected = graph.plan_minimum_cut(&costs, CutPlanningLimits::default()).unwrap();
    assert_eq!(expected.work.projection_edges, 8);
    assert_eq!(expected.work.augmentations, 2);
    for quantum in [1, 2, 3, 11, usize::MAX] {
        assert_eq!(finish(graph.begin_minimum_cut(&costs, CutPlanningLimits::default()).unwrap(), quantum).unwrap(), expected);
    }
}

#[test]
fn dropping_cancelling_and_premature_finish_do_not_change_the_frozen_graph_or_other_jobs() {
    let graph = serial(); let costs = prices(&[(2, 9), (3, 2)]);
    let expected = graph.plan_minimum_cut(&costs, CutPlanningLimits::default()).unwrap();
    for polls in 0..=expected.work.edge_visits {
        let mut run = graph.begin_minimum_cut(&costs, CutPlanningLimits::default()).unwrap();
        for _ in 0..polls { run.advance(2).unwrap(); }
        let report = run.progress();
        if polls % 3 == 0 { assert_eq!(run.cancel(), report.work); }
        else if polls % 3 == 1 { drop(run); }
        else if report.status != CutPlanningStatus::Ready { assert_eq!(run.finish(), Err(Error::Incomplete)); }
        else { assert_eq!(run.finish().unwrap(), expected); }
        assert_eq!(graph, serial());
    }
    assert_eq!(finish(graph.begin_minimum_cut(&costs, CutPlanningLimits::default()).unwrap(), 2).unwrap(), expected);
}

#[test]
fn graph_and_prices_cannot_be_replaced_under_equal_generation_labels() {
    let graph = serial(); let mut costs = prices(&[(2, 9), (3, 2)]);
    let expected = graph.plan_minimum_cut(&costs, CutPlanningLimits::default()).unwrap();
    let run = graph.begin_minimum_cut(&costs, CutPlanningLimits::default()).unwrap();
    costs[0].units = 1; costs[1].units = 20;
    let mut spec = graph.spec().clone(); spec.edges[0].provenance += 1;
    let changed = AuthorityGraph::new(spec).unwrap();
    let plan = finish(run, 2).unwrap(); assert_eq!(plan, expected);
    let CutPlanOutcome::Candidate(plan) = plan.outcome else { panic!("expected candidate"); };
    assert_eq!(changed.verify_cut(&plan.proposal, MAX_CHECK_EDGE_VISITS), Err(Error::Binding));
    let different = finish(graph.begin_minimum_cut(&costs, CutPlanningLimits::default()).unwrap(), 2).unwrap();
    let CutPlanOutcome::Candidate(different) = different.outcome else { panic!("expected new-priced candidate"); };
    assert_eq!(different.proposal.gates, vec![2]); assert_eq!(different.total_cost, 1);
    assert_eq!(plan.proposal.gates, vec![3]);
}

#[test]
fn missing_inventory_invalid_prices_and_capacity_have_original_admission_precedence() {
    let graph = serial();
    for costs in [prices(&[(2, 1)]), prices(&[(2, 1), (2, 2)]), prices(&[(2, 0), (3, 2)]),
        prices(&[(2, 1), (4, 2)]), prices(&[(2, u64::MAX), (3, 1)]),
        prices(&[(2, u64::MAX - 1), (3, 1)])] {
        assert_eq!(graph.begin_minimum_cut(&costs, CutPlanningLimits::default()).unwrap_err(),
            graph.plan_minimum_cut(&costs, CutPlanningLimits::default()).unwrap_err());
    }
    for completeness in [Completeness::Unknown, Completeness::DeclaredComplete { inventory_generation: 2 }] {
        let mut spec = graph.spec().clone(); spec.completeness = completeness;
        let incomplete = AuthorityGraph::new(spec).unwrap();
        assert_eq!(incomplete.begin_minimum_cut(&[], CutPlanningLimits::default()).unwrap_err(),
            incomplete.plan_minimum_cut(&[], CutPlanningLimits::default()).unwrap_err());
    }
    let costs = prices(&[(2, u64::MAX - 2), (3, 1)]);
    assert_eq!(finish(graph.begin_minimum_cut(&costs, CutPlanningLimits::default()).unwrap(), 2).unwrap(),
        graph.plan_minimum_cut(&costs, CutPlanningLimits::default()).unwrap());
}

#[test]
fn bypass_and_disconnected_sinks_remain_distinct_nonpermitting_results() {
    for (nodes, edges, gates, costs) in [
        (vec![(1, NodeKind::Actor), (2, NodeKind::Sink)], vec![(1, 2)], vec![], vec![]),
        (vec![(1, NodeKind::Actor), (2, NodeKind::Enforcer), (3, NodeKind::Sink), (4, NodeKind::Sink)],
            vec![(1, 2), (2, 3)], vec![2], prices(&[(2, 1)])),
        (vec![(1, NodeKind::Actor), (2, NodeKind::Enforcer), (3, NodeKind::Enforcer), (4, NodeKind::Sink)],
            vec![(1, 2), (2, 4), (1, 3), (3, 4)], vec![2], prices(&[(2, 1)])),
    ] {
        let graph = topology(&nodes, &edges, &gates);
        let plan = finish(graph.begin_minimum_cut(&costs, CutPlanningLimits::default()).unwrap(), 1).unwrap();
        assert_eq!(plan, graph.plan_minimum_cut(&costs, CutPlanningLimits::default()).unwrap());
        assert!(!matches!(plan.outcome, CutPlanOutcome::Candidate(_)));
        assert_eq!(plan.work.residual_arcs, 0);
    }
}
