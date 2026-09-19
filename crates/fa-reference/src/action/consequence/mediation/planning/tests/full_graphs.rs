use super::*;

#[test]
fn every_nonempty_four_vertex_digraph_including_cycles_matches_independent_subsets() {
    let nodes = [(1, NodeKind::Actor), (2, NodeKind::Enforcer), (3, NodeKind::Enforcer), (4, NodeKind::Sink)];
    let all_edges: Vec<_> = (1_u64..=4).flat_map(|from| (1_u64..=4)
        .filter(move |&to| from != to).map(move |to| (from, to))).collect();
    // All 4,095 nonempty directed simple graphs, not merely one DAG order.
    // Self-loops and parallel channels have separate positive controls.
    for mask in 1..(1_usize << all_edges.len()) {
        let edges: Vec<_> = all_edges.iter().enumerate().filter(|(i, _)| mask & (1_usize << *i) != 0)
            .map(|(_, &edge)| edge).collect();
        let graph = topology(&nodes, &edges, &[2, 3]);
        for weights in [[1, 9], [9, 1], [5, 5]] {
            let costs = prices(&[(2, weights[0]), (3, weights[1])]);
            let plan = graph.plan_minimum_cut(&costs, CutPlanningLimits::default()).unwrap();
            if !oracle_reachable(graph.spec(), &BTreeSet::new()).contains(&4) {
                assert_eq!(plan.outcome, CutPlanOutcome::Unreachable { sinks: vec![4] });
                continue;
            }
            let optimum = (0_u8..4).filter_map(|subset| {
                let removed: BTreeSet<_> = (0_usize..2).filter(|i| subset & (1_u8 << *i) != 0)
                    .map(|i| i as u64 + 2).collect();
                (!oracle_reachable(graph.spec(), &removed).contains(&4)).then(|| costs.iter()
                    .filter(|cost| removed.contains(&cost.node)).map(|cost| cost.units).sum::<u64>())
            }).min();
            match optimum {
                Some(cost) => assert_eq!(candidate(plan).total_cost, cost, "digraph {mask}, costs {weights:?}"),
                None => {
                    let CutPlanOutcome::Bypass(path) = plan.outcome else { panic!("actual uncovered path"); };
                    assert_eq!(path.nodes.first(), Some(&1)); assert_eq!(path.nodes.last(), Some(&4));
                    assert_eq!(path.edges.len() + 1, path.nodes.len());
                    assert!(!path.nodes.iter().any(|id| [2, 3].contains(id)));
                    for (edge, pair) in path.edges.iter().zip(path.nodes.windows(2)) {
                        assert!(graph.spec().edges.iter().any(|original|
                            original.id == *edge && original.from == pair[0] && original.to == pair[1]));
                    }
                }
            }
        }
    }
}

#[test]
fn maximum_vertex_edge_and_enforcer_profile_produces_a_usable_cut_with_exact_bounds() {
    let nodes: Vec<_> = (1..=MAX_NODES as u64).map(|id| (id, match id {
        1 => NodeKind::Actor,
        id if id == MAX_NODES as u64 => NodeKind::Sink,
        id if id <= MAX_CUT_NODES as u64 + 1 => NodeKind::Enforcer,
        _ => NodeKind::Process,
    })).collect();
    let gates: Vec<_> = (2..=MAX_CUT_NODES as u64 + 1).collect();
    let mut edges: Vec<_> = gates.iter().flat_map(|&gate| [(1, gate), (gate, MAX_NODES as u64)]).collect();
    // Preserve all parallel channels instead of deduplicating to fake the bound.
    edges.resize(MAX_EDGES, (1, 2));
    let graph = topology(&nodes, &edges, &gates);
    let costs: Vec<_> = gates.iter().map(|&node| EnforcerCost { node, units: 1 }).collect();
    let plan = graph.plan_minimum_cut(&costs, CutPlanningLimits::default()).unwrap();
    assert_eq!(plan.work.projection_nodes, MAX_NODES);
    assert_eq!(plan.work.projection_edges, MAX_EDGES);
    assert_eq!(plan.work.residual_vertices, 2 * MAX_NODES + 2);
    assert_eq!(plan.work.residual_arcs, 2 * (MAX_NODES + MAX_EDGES + 2));
    let exact = CutPlanningLimits { residual_arcs: plan.work.residual_arcs, edge_visits: plan.work.edge_visits };
    assert_eq!(graph.plan_minimum_cut(&costs, exact).unwrap(), plan);
    assert_eq!(graph.plan_minimum_cut(&costs, CutPlanningLimits { edge_visits: exact.edge_visits - 1, ..exact }), Err(Error::Limit));
    assert_eq!(graph.plan_minimum_cut(&costs, CutPlanningLimits { residual_arcs: exact.residual_arcs - 1, ..exact }), Err(Error::Limit));
    let selected = candidate(plan);
    assert_eq!(selected.proposal.gates, gates);
    assert_eq!(selected.total_cost, MAX_CUT_NODES as u64);
    assert_eq!(selected.proposal.reachable, vec![1]);
    let mut too_many_nodes = graph.spec().clone();
    too_many_nodes.nodes.push(Node { id: MAX_NODES as u64 + 1, kind: NodeKind::Process });
    assert_eq!(AuthorityGraph::new(too_many_nodes), Err(Error::Limit));
    let mut too_many_edges = graph.spec().clone();
    too_many_edges.edges.push(Edge { id: MAX_EDGES as u64 + 1, ..too_many_edges.edges[0].clone() });
    assert_eq!(AuthorityGraph::new(too_many_edges), Err(Error::Limit));
}
