use fa_reference::perimeter::PerimeterScope;
use fa_reference::perimeter::coverage::synthesis::{
    CutBudget, CutPlan, CutSynthesis, CutWork, MAX_CUT_AUGMENTATIONS,
    MAX_CUT_RESIDUAL_CELLS, MAX_CUT_RESIDUAL_SCANS,
};
use fa_reference::perimeter::coverage::{
    CoverageDeclaration, CoverageIdentity, CoverageProblem, SinkCoverage,
};
use fa_reference::{Error, Graph};
use std::collections::{BTreeMap, BTreeSet};

fn budget() -> CutBudget {
    CutBudget {
        residual_cells: MAX_CUT_RESIDUAL_CELLS,
        residual_scans: MAX_CUT_RESIDUAL_SCANS,
        augmentations: MAX_CUT_AUGMENTATIONS,
    }
}

fn declaration(sources: &[usize], sinks: &[usize], candidates: &[usize]) -> CoverageDeclaration {
    CoverageDeclaration {
        identity: CoverageIdentity {
            scope: PerimeterScope { tenant: 1, principal: 2, purpose: 3 },
            projection: 4,
            semantic_epoch: 5,
            topology_generation: 6,
        },
        sources: sources.iter().copied().collect(),
        sinks: sinks.iter().copied().collect(),
        candidates: candidates.iter().copied().collect(),
        complete: true,
    }
}

fn planned(result: CutSynthesis) -> Box<CutPlan> {
    match result {
        CutSynthesis::Planned(plan) => plan,
        other => panic!("expected a verified plan, got {other:?}"),
    }
}

fn diamond() -> CoverageProblem {
    CoverageProblem::new(
        Graph::new(4, &[(0, 1), (1, 3), (0, 2), (2, 3)]).unwrap(),
        declaration(&[0], &[3], &[1, 2]),
    ).unwrap()
}

#[test]
fn shared_barrier_competes_with_separate_branches_by_total_cost() {
    let graph = Graph::new(5, &[(0, 1), (0, 2), (1, 3), (2, 3), (3, 4)]).unwrap();
    let problem = CoverageProblem::new(graph, declaration(&[0], &[4], &[1, 2, 3])).unwrap();
    let costs = BTreeMap::from([(1, 3), (2, 4), (3, 5)]);
    let shared = planned(problem.minimum_cost_cut(&costs, budget()).unwrap());
    assert_eq!(shared.barriers(), &BTreeSet::from([3]));
    assert_eq!(shared.total_cost(), 5);
    assert_eq!(shared.costs(), &costs);
    assert_eq!(problem.verify(shared.certificate()), Ok(()));
    assert_eq!(problem.verify_cut_plan(&shared, &costs, budget()), Ok(()));
    assert_eq!(shared.work().topology_audits, 2);
    assert_eq!(shared.work().independent_cut_checks, 1);
    let changed = BTreeMap::from([(1, 3), (2, 4), (3, 9)]);
    let split = planned(problem.minimum_cost_cut(&changed, budget()).unwrap());
    assert_eq!(split.barriers(), &BTreeSet::from([1, 2]));
    assert_eq!(split.total_cost(), 7);
    assert_eq!(problem.verify_cut_plan(&shared, &changed, budget()).unwrap_err().error,
        Error::Binding);
}

#[test]
fn unselected_prices_and_unchanged_generation_topology_are_still_bound() {
    let graph = Graph::new(4, &[(0, 1), (1, 2), (2, 3)]).unwrap();
    let declared = declaration(&[0], &[3], &[1, 2]);
    let problem = CoverageProblem::new(graph, declared.clone()).unwrap();
    let costs = BTreeMap::from([(1, 2), (2, 20)]);
    let plan = planned(problem.minimum_cost_cut(&costs, budget()).unwrap());
    assert_eq!(plan.barriers(), &BTreeSet::from([1]));
    let changed = BTreeMap::from([(1, 2), (2, 21)]);
    assert_eq!(problem.verify_cut_plan(&plan, &changed, budget()).unwrap_err().error,
        Error::Binding);
    let bypass = CoverageProblem::new(
        Graph::new(4, &[(0, 1), (1, 2), (2, 3), (0, 3)]).unwrap(), declared,
    ).unwrap();
    let failure = bypass.verify_cut_plan(&plan, &costs, budget()).unwrap_err();
    assert_eq!(failure.error, Error::Binding);
    assert_eq!(failure.work, CutWork::default());
    assert_eq!(problem.verify_cut_plan(&plan, &costs, budget()), Ok(()));
}

#[test]
fn uncuttable_route_returns_a_surviving_witness_not_an_expensive_fake_cut() {
    let problem = CoverageProblem::new(
        Graph::new(4, &[(0, 1), (1, 3), (0, 3)]).unwrap(),
        declaration(&[0], &[3], &[1, 2]),
    ).unwrap();
    let costs = BTreeMap::from([(1, 1), (2, u64::MAX)]);
    match problem.minimum_cost_cut(&costs, budget()).unwrap() {
        CutSynthesis::Impossible { audit, work } => {
            assert_eq!(audit.sinks()[&3], SinkCoverage::Bypass { path: vec![0, 3] });
            assert_eq!(audit.barriers(), &BTreeSet::from([1, 2]));
            assert_eq!(work.residual_cells, 0);
            assert_eq!(work.residual_scans, 0);
            assert_eq!(work.topology_audits, 1);
        }
        other => panic!("expected the uncuttable bypass, got {other:?}"),
    }
}

#[test]
fn unreachable_has_no_certificate_even_with_a_zero_solver_allowance() {
    let problem = CoverageProblem::new(
        Graph::new(4, &[]).unwrap(), declaration(&[0], &[3], &[1, 2]),
    ).unwrap();
    let costs = BTreeMap::from([(1, 1), (2, 2)]);
    let zero = CutBudget { residual_cells: 0, residual_scans: 0, augmentations: 0 };
    match problem.minimum_cost_cut(&costs, zero).unwrap() {
        CutSynthesis::AlreadyUnreachable { audit, work } => {
            assert_eq!(audit.active_sinks(), 0);
            assert!(!audit.covers_active_sinks());
            assert_eq!(work.residual_cells, 0);
        }
        other => panic!("vacuity was incorrectly credited: {other:?}"),
    }
    assert_eq!(problem.minimum_cost_cut(&BTreeMap::new(), zero).unwrap_err().error,
        Error::Binding);
}

#[test]
fn free_cuts_work_without_augmentation_but_still_require_a_complete_search() {
    let problem = diamond();
    let costs = BTreeMap::from([(1, 0), (2, 0)]);
    let no_augment = CutBudget { augmentations: 0, ..budget() };
    let plan = planned(problem.minimum_cost_cut(&costs, no_augment).unwrap());
    assert_eq!(plan.total_cost(), 0);
    assert_eq!(plan.barriers(), &BTreeSet::from([1, 2]));
    assert_eq!(plan.work().augmentations, 0);
    assert!(plan.work().residual_scans > 0);
    let no_search = CutBudget { residual_scans: 0, ..no_augment };
    let failure = problem.minimum_cost_cut(&costs, no_search).unwrap_err();
    assert_eq!(failure.error, Error::Limit);
    assert_eq!(failure.work.residual_scans, 0);
}

#[test]
fn two_maximum_u64_costs_are_not_saturated_or_confused_with_infinity() {
    let problem = diamond();
    let costs = BTreeMap::from([(1, u64::MAX), (2, u64::MAX)]);
    let plan = planned(problem.minimum_cost_cut(&costs, budget()).unwrap());
    assert_eq!(plan.barriers(), &BTreeSet::from([1, 2]));
    assert_eq!(plan.total_cost(), 2 * u128::from(u64::MAX));
    assert_eq!(plan.work().augmentations, 2);
    assert_eq!(problem.verify_cut_plan(&plan, &costs, budget()), Ok(()));
}

#[test]
fn exact_limits_succeed_and_one_less_retains_work_without_a_plan() {
    let problem = diamond();
    let costs = BTreeMap::from([(1, 2), (2, 3)]);
    let plan = planned(problem.minimum_cost_cut(&costs, budget()).unwrap());
    let used = plan.work();
    let exact = CutBudget {
        residual_cells: used.residual_cells,
        residual_scans: used.residual_scans,
        augmentations: used.augmentations,
    };
    assert_eq!(problem.minimum_cost_cut(&costs, exact).unwrap(), CutSynthesis::Planned(plan));
    for dimension in 0..3 {
        let mut short = exact;
        match dimension {
            0 => short.residual_cells -= 1,
            1 => short.residual_scans -= 1,
            _ => short.augmentations -= 1,
        }
        let failure = problem.minimum_cost_cut(&costs, short).unwrap_err();
        assert_eq!(failure.error, Error::Limit);
        match dimension {
            0 => assert_eq!(failure.work.residual_cells, 0),
            1 => assert_eq!(failure.work.residual_scans, short.residual_scans),
            _ => assert_eq!(failure.work.augmentations, short.augmentations),
        }
        assert_eq!(failure.work.independent_cut_checks, 0);
    }
    let again = planned(problem.minimum_cost_cut(&costs, exact).unwrap());
    assert_eq!(again.total_cost(), 5);
    assert_eq!(again.work(), used);
}

#[test]
fn incomplete_cost_inventories_and_excessive_solver_limits_refuse_without_work() {
    let problem = diamond();
    let before = problem.clone();
    let costs = BTreeMap::from([(1, 1), (2, 1)]);
    for malformed in [
        BTreeMap::from([(1, 1)]),
        BTreeMap::from([(1, 1), (3, 1)]),
        BTreeMap::from([(0, 1), (1, 1), (2, 1)]),
    ] {
        let failure = problem.minimum_cost_cut(&malformed, budget()).unwrap_err();
        assert_eq!(failure.error, Error::Binding);
        assert_eq!(failure.work, CutWork::default());
    }
    for dimension in 0..3 {
        let mut excessive = budget();
        match dimension {
            0 => excessive.residual_cells += 1,
            1 => excessive.residual_scans += 1,
            _ => excessive.augmentations += 1,
        }
        let failure = problem.minimum_cost_cut(&costs, excessive).unwrap_err();
        assert_eq!(failure.error, Error::Limit);
        assert_eq!(failure.work, CutWork::default());
    }
    assert_eq!(problem, before);
    assert!(matches!(problem.minimum_cost_cut(&costs, budget()), Ok(CutSynthesis::Planned(_))));
}

#[test]
fn multiple_sources_sinks_and_cycles_share_one_exact_optimization() {
    let edges = [(0, 2), (1, 3), (2, 3), (3, 2), (2, 4), (3, 4), (4, 6), (4, 5), (5, 7)];
    let graph = Graph::new(8, &edges).unwrap();
    let problem = CoverageProblem::new(graph.clone(), declaration(&[0, 1], &[6, 7], &[2, 3, 4, 5])).unwrap();
    let costs = BTreeMap::from([(2, 2), (3, 3), (4, 9), (5, 20)]);
    let plan = planned(problem.minimum_cost_cut(&costs, budget()).unwrap());
    assert_eq!(plan.barriers(), &BTreeSet::from([2, 3]));
    assert_eq!(plan.total_cost(), 5);
    assert_eq!(plan.certificate().audit().active_sinks(), 2);
    assert!(graph.cut_disconnects(&[0, 1], &[6, 7], plan.barriers()).unwrap());
}

#[test]
fn empty_eligibility_and_overlapping_endpoints_cannot_be_removed_by_the_solver() {
    let direct = CoverageProblem::new(
        Graph::new(2, &[(0, 1)]).unwrap(), declaration(&[0], &[1], &[]),
    ).unwrap();
    assert!(matches!(direct.minimum_cost_cut(&BTreeMap::new(), budget()),
        Ok(CutSynthesis::Impossible { .. })));
    let overlap = CoverageProblem::new(
        Graph::new(3, &[(0, 1), (1, 2)]).unwrap(), declaration(&[0], &[0, 2], &[1]),
    ).unwrap();
    match overlap.minimum_cost_cut(&BTreeMap::from([(1, 0)]), budget()).unwrap() {
        CutSynthesis::Impossible { audit, .. } => {
            assert_eq!(audit.sinks()[&0], SinkCoverage::Bypass { path: vec![0] });
        }
        other => panic!("the source/sink overlap was incorrectly cut: {other:?}"),
    }
}

#[test]
fn equal_cost_ties_are_stable_under_edge_permutation_duplicates_and_self_loops() {
    let edges = [(0, 1), (1, 2), (2, 3), (1, 1), (2, 2), (2, 1), (0, 1)];
    let declared = declaration(&[0], &[3], &[1, 2]);
    let forward = CoverageProblem::new(Graph::new(4, &edges).unwrap(), declared.clone()).unwrap();
    let reverse_edges: Vec<_> = edges.into_iter().rev().collect();
    let reverse = CoverageProblem::new(Graph::new(4, &reverse_edges).unwrap(), declared).unwrap();
    let costs = BTreeMap::from([(1, 3), (2, 3)]);
    let first = forward.minimum_cost_cut(&costs, budget()).unwrap();
    assert_eq!(first, reverse.minimum_cost_cut(&costs, budget()).unwrap());
    assert_eq!(planned(first).total_cost(), 3);
}

#[test]
fn maximum_graph_size_obeys_the_declared_residual_matrix_bound() {
    let edges: Vec<_> = (0..127).map(|node| (node, node + 1)).collect();
    let candidates: Vec<_> = (1..127).collect();
    let problem = CoverageProblem::new(
        Graph::new(128, &edges).unwrap(), declaration(&[0], &[127], &candidates),
    ).unwrap();
    let costs = candidates.into_iter().map(|node| (node, 7)).collect();
    let plan = planned(problem.minimum_cost_cut(&costs, budget()).unwrap());
    assert_eq!(plan.total_cost(), 7);
    assert_eq!(plan.work().residual_cells, MAX_CUT_RESIDUAL_CELLS);
    assert_eq!(plan.barriers().len(), 1);
    assert_eq!(problem.verify(plan.certificate()), Ok(()));
}

#[test]
fn all_four_node_directed_graphs_match_exhaustive_subset_optima() {
    let possible: Vec<_> = (0..4).flat_map(|from|
        (0..4).filter(move |&to| from != to).map(move |to| (from, to))).collect();
    let cost_pairs = [
        (0, 0), (0, 1), (1, 0), (1, 1), (1, 3), (3, 1),
        (u64::MAX, 1), (1, u64::MAX), (u64::MAX, u64::MAX),
    ];
    for mask in 0..(1_usize << possible.len()) {
        let edges: Vec<_> = possible.iter().enumerate()
            .filter(|(index, _)| mask & (1 << index) != 0)
            .map(|(_, &edge)| edge).collect();
        let graph = Graph::new(4, &edges).unwrap();
        let problem = CoverageProblem::new(graph.clone(), declaration(&[0], &[3], &[1, 2])).unwrap();
        let active = !graph.cut_disconnects(&[0], &[3], &BTreeSet::new()).unwrap();
        for (left, right) in cost_pairs {
            let costs = BTreeMap::from([(1, left), (2, right)]);
            let mut minimum: Option<u128> = None;
            for bits in 0..4 {
                let cut: BTreeSet<_> = (1..=2)
                    .filter(|node| bits & (1 << (node - 1)) != 0).collect();
                if graph.cut_disconnects(&[0], &[3], &cut).unwrap() {
                    let cost = cut.iter().map(|node| u128::from(costs[node])).sum();
                    minimum = Some(minimum.map_or(cost, |previous| previous.min(cost)));
                }
            }
            match problem.minimum_cost_cut(&costs, budget()).unwrap() {
                CutSynthesis::AlreadyUnreachable { audit, .. } => {
                    assert!(!active);
                    assert_eq!(audit.active_sinks(), 0);
                }
                CutSynthesis::Impossible { audit, .. } => {
                    assert!(active);
                    assert_eq!(minimum, None);
                    assert!(audit.has_bypass());
                }
                CutSynthesis::Planned(plan) => {
                    assert!(active);
                    assert_eq!(Some(plan.total_cost()), minimum, "graph {mask}, costs {costs:?}");
                    assert!(graph.cut_disconnects(&[0], &[3], plan.barriers()).unwrap());
                    assert_eq!(problem.verify(plan.certificate()), Ok(()));
                }
            }
        }
    }
}
