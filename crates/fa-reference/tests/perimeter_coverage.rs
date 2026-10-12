use fa_reference::perimeter::PerimeterScope;
use fa_reference::perimeter::coverage::{
    CoverageDeclaration, CoverageIdentity, CoverageProblem, SinkCoverage,
};
use fa_reference::{Error, Graph};
use std::collections::BTreeSet;

fn declaration() -> CoverageDeclaration {
    CoverageDeclaration {
        identity: CoverageIdentity {
            scope: PerimeterScope { tenant: 1, principal: 2, purpose: 3 },
            projection: 4,
            semantic_epoch: 5,
            topology_generation: 6,
        },
        sources: BTreeSet::from([0]),
        sinks: BTreeSet::from([3]),
        candidates: BTreeSet::from([1, 2]),
        complete: true,
    }
}

#[test]
fn alternative_route_is_a_concrete_bypass_until_both_gates_are_selected() {
    let graph = Graph::new(4, &[(0, 1), (1, 3), (0, 2), (2, 3), (2, 0)]).unwrap();
    let problem = CoverageProblem::new(graph.clone(), declaration()).unwrap();
    let audit = problem.audit(&BTreeSet::from([1])).unwrap();
    assert_eq!(audit.sinks()[&3], SinkCoverage::Bypass { path: vec![0, 2, 3] });
    assert!(!audit.covers_active_sinks());
    assert_eq!(problem.certify(&BTreeSet::from([1])), Err(Error::Incomplete));
    let cut = BTreeSet::from([1, 2]);
    let certificate = problem.certify(&cut).unwrap();
    assert_eq!(certificate.audit().active_sinks(), 1);
    assert_eq!(certificate.audit().sinks()[&3],
        SinkCoverage::Mediated { baseline_path: vec![0, 1, 3] });
    assert!(graph.cut_disconnects(&[0], &[3], &cut).unwrap());
    assert_eq!(problem.verify(&certificate), Ok(()));
}

#[test]
fn unreachable_sinks_are_not_credited_as_mediated() {
    let mut declared = declaration();
    declared.sinks.insert(4);
    let graph = Graph::new(5, &[(0, 1), (1, 3)]).unwrap();
    let problem = CoverageProblem::new(graph, declared).unwrap();
    let audit = problem.audit(&BTreeSet::from([1])).unwrap();
    assert_eq!(audit.active_sinks(), 1);
    assert_eq!(audit.sinks()[&4], SinkCoverage::AlreadyUnreachable);
    assert!(audit.covers_active_sinks());
    let isolated = CoverageProblem::new(Graph::new(4, &[]).unwrap(), declaration()).unwrap();
    assert_eq!(isolated.audit(&BTreeSet::new()).unwrap().active_sinks(), 0);
    assert_eq!(isolated.certify(&BTreeSet::new()), Err(Error::Incomplete));
}

#[test]
fn endpoints_and_undeclared_barriers_cannot_manufacture_coverage() {
    let graph = Graph::new(4, &[(0, 1), (1, 3)]).unwrap();
    let problem = CoverageProblem::new(graph.clone(), declaration()).unwrap();
    for invalid in [0, 3, 4] {
        assert_eq!(problem.audit(&BTreeSet::from([invalid])), Err(Error::Binding));
    }
    for endpoint in [0, 3] {
        let mut declared = declaration();
        declared.candidates.insert(endpoint);
        assert_eq!(CoverageProblem::new(graph.clone(), declared), Err(Error::Binding));
    }
    assert!(problem.certify(&BTreeSet::from([1])).is_ok());
}

#[test]
fn a_source_that_is_also_a_sink_has_a_zero_edge_bypass() {
    let mut declared = declaration();
    declared.sinks.insert(0);
    let problem = CoverageProblem::new(Graph::new(4, &[(0, 1), (1, 3)]).unwrap(), declared).unwrap();
    let audit = problem.audit(&BTreeSet::from([1, 2])).unwrap();
    assert_eq!(audit.sinks()[&0], SinkCoverage::Bypass { path: vec![0] });
    assert_eq!(problem.certify(audit.barriers()), Err(Error::Incomplete));
}

#[test]
fn incomplete_or_malformed_declarations_refuse_next_to_a_valid_control() {
    let graph = Graph::new(4, &[(0, 1), (1, 3)]).unwrap();
    assert!(CoverageProblem::new(graph.clone(), declaration()).is_ok());
    let mut declared = declaration();
    declared.complete = false;
    assert_eq!(CoverageProblem::new(graph.clone(), declared), Err(Error::Incomplete));
    let mut declared = declaration();
    declared.identity.topology_generation = 0;
    assert_eq!(CoverageProblem::new(graph.clone(), declared), Err(Error::InvalidInput));
    let mut declared = declaration();
    declared.sources.clear();
    assert_eq!(CoverageProblem::new(graph.clone(), declared), Err(Error::InvalidInput));
    let mut declared = declaration();
    declared.sinks.insert(4);
    assert_eq!(CoverageProblem::new(graph, declared), Err(Error::InvalidInput));
}

#[test]
fn certificates_bind_full_topology_scope_endpoints_and_eligibility() {
    let graph = Graph::new(4, &[(0, 1), (1, 3)]).unwrap();
    let original = CoverageProblem::new(graph.clone(), declaration()).unwrap();
    let certificate = original.certify(&BTreeSet::from([1])).unwrap();
    assert_eq!(original.verify(&certificate), Ok(()));
    for change in 0..7 {
        let mut declared = declaration();
        match change {
            0 => declared.identity.scope.tenant += 1,
            1 => declared.identity.projection += 1,
            2 => declared.identity.semantic_epoch += 1,
            3 => declared.identity.topology_generation += 1,
            4 => { declared.candidates.remove(&2); }
            5 => { declared.sources.insert(2); declared.candidates.remove(&2); }
            _ => { declared.sinks.insert(2); declared.candidates.remove(&2); }
        }
        let changed = CoverageProblem::new(graph.clone(), declared).unwrap();
        assert_eq!(changed.verify(&certificate), Err(Error::Binding));
    }
    let bypass = Graph::new(4, &[(0, 1), (1, 3), (0, 3)]).unwrap();
    assert_eq!(CoverageProblem::new(bypass, declaration()).unwrap().verify(&certificate),
        Err(Error::Binding));
}

#[test]
fn source_and_edge_order_do_not_change_shortest_witness_tie_breaking() {
    let mut declared = declaration();
    declared.sources.insert(4);
    let edges = [(4, 2), (2, 3), (0, 1), (1, 3), (0, 1)];
    let forward = CoverageProblem::new(Graph::new(5, &edges).unwrap(), declared.clone()).unwrap();
    let reversed: Vec<_> = edges.into_iter().rev().collect();
    let reverse = CoverageProblem::new(Graph::new(5, &reversed).unwrap(), declared).unwrap();
    let audit = forward.audit(&BTreeSet::new()).unwrap();
    assert_eq!(audit, reverse.audit(&BTreeSet::new()).unwrap());
    assert_eq!(audit.sinks()[&3], SinkCoverage::Bypass { path: vec![0, 1, 3] });
    assert_eq!(forward.audit(&BTreeSet::from([1])).unwrap().sinks()[&3],
        SinkCoverage::Bypass { path: vec![4, 2, 3] });
}

#[test]
fn all_four_node_directed_graphs_agree_with_original_cut_oracle() {
    let edges: Vec<_> = (0..4).flat_map(|from|
        (0..4).filter(move |&to| from != to).map(move |to| (from, to))).collect();
    for mask in 0..(1_usize << edges.len()) {
        let chosen: Vec<_> = edges.iter().enumerate()
            .filter(|(index, _)| mask & (1 << index) != 0)
            .map(|(_, &edge)| edge).collect();
        let graph = Graph::new(4, &chosen).unwrap();
        let problem = CoverageProblem::new(graph.clone(), declaration()).unwrap();
        for bits in 0..4 {
            let cut: BTreeSet<_> = (1..=2).filter(|node| bits & (1 << (node - 1)) != 0).collect();
            let audit = problem.audit(&cut).unwrap();
            let connected = !graph.cut_disconnects(&[0], &[3], &BTreeSet::new()).unwrap();
            assert_eq!(audit.active_sinks(), usize::from(connected));
            assert_eq!(audit.has_bypass(), !graph.cut_disconnects(&[0], &[3], &cut).unwrap());
            if let SinkCoverage::Bypass { path } = &audit.sinks()[&3] {
                assert_eq!(path.first(), Some(&0));
                assert_eq!(path.last(), Some(&3));
                assert!(path.iter().all(|node| !cut.contains(node)));
                assert!(path.windows(2).all(|pair| chosen.contains(&(pair[0], pair[1]))));
            }
        }
    }
}
