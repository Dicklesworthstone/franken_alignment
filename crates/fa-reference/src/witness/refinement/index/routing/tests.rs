use super::*;
use crate::product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker};
use crate::witness::{AdapterDomainInput, DomainClosure, QueryRole, Reuse, SnapshotEntry, WitnessRequest, WitnessSnapshot, MAX_VALUE_BYTES};

fn domain(source: u64) -> DomainProjection {
    DomainProjection::new(3, 4, ProjectionKey { source, branch: 5, projection: 6, source_epoch: 7 })
}
fn basis(domain: DomainProjection, revision: u64, entries: Vec<SnapshotEntry>) -> (WitnessSnapshot, ProductFrontiers) {
    let marker = TrustedClosingMarker { key: domain.projection(), final_sequence: 1, marker_generation: 1 };
    let mut frontiers = ProductFrontiers::new(1, 1).unwrap();
    frontiers.accept(marker.key, FrontierStage::Authenticated, 1).unwrap();
    frontiers.record_close(marker).unwrap();
    (WitnessSnapshot::new(revision, revision, 1,
        AdapterDomainInput::new(domain, DomainClosure::Closed(marker)), entries).unwrap(), frontiers)
}
fn judgment(domain: DomainProjection, requests: Vec<WitnessRequest>) -> WitnessJudgment {
    let entries = requests.iter().filter_map(|request| match request {
        WitnessRequest::ExactValue { key, .. } => Some(SnapshotEntry::new(*key, 1, b"value".to_vec()).unwrap()),
        _ => None,
    }).collect();
    let (snapshot, frontiers) = basis(domain, 1, entries);
    WitnessJudgment::capture(&snapshot, &frontiers, requests).unwrap()
}
fn unlimited() -> RoutingBudget { RoutingBudget { steps: u64::MAX, bytes: u64::MAX } }
fn index() -> InvalidationIndex {
    InvalidationIndex::new(RoutingLimits { judgments: 128, dependencies: MAX_ROUTED_DEPENDENCIES }).unwrap()
}

// Deliberately scan the ORIGINAL private witnesses, not sorted postings, prefix
// maxima, or production lookup helpers. This oracle is not a validity verdict.
fn scan(judgment: &WitnessJudgment, opaque: bool, change: WitnessChange) -> bool {
    if opaque || matches!(change, WitnessChange::All) { return true; }
    let new = match change {
        WitnessChange::Key { domain, .. } | WitnessChange::Range { domain, .. }
        | WitnessChange::Domain { domain } => domain,
        WitnessChange::All => unreachable!(),
    };
    let old = judgment.domain;
    let a = old.projection(); let b = new.projection();
    if old.domain_id() != new.domain_id() || a.source != b.source
        || a.branch != b.branch || a.projection != b.projection { return false; }
    if old != new || matches!(change, WitnessChange::Domain { .. }) { return true; }
    judgment.witnesses.iter().any(|witness| match (witness, change) {
        (Witness::ExactValue { key, .. } | Witness::AbsentKey { key, .. }, WitnessChange::Key { key: changed, .. }) => *key == changed,
        (Witness::ExactValue { key, .. } | Witness::AbsentKey { key, .. }, WitnessChange::Range { start, end, .. }) => start <= *key && *key < end,
        (Witness::EmptyRange { start, end, .. } | Witness::RangeMembers { start, end, .. }, WitnessChange::Key { key, .. }) => *start <= key && key < *end,
        (Witness::EmptyRange { start, end, .. } | Witness::RangeMembers { start, end, .. }, WitnessChange::Range { start: low, end: high, .. }) => *start < high && low < *end,
        _ => unreachable!(),
    })
}

#[test]
fn every_key_and_range_matches_an_independent_full_dependency_scan() {
    let mut index = index();
    let mut original = Vec::new();
    for source in 0..3 {
        for key in 0..6 {
            for request in [WitnessRequest::ExactValue { key, role: QueryRole::Subject }, WitnessRequest::AbsentKey { key }] {
                original.push(judgment(domain(source), vec![request]));
            }
            original.push(judgment(domain(source), vec![WitnessRequest::EmptyRange { start: key, end: key + 2 }]));
            original.push(judgment(domain(source), vec![WitnessRequest::RangeMembers { start: key, end: key + 3 }]));
        }
    }
    for (id, judgment) in original.iter().enumerate() { index.register(id as u64, Some(judgment), false).unwrap(); }
    for source in 0..4 {
        let mut changes = vec![WitnessChange::All, WitnessChange::Domain { domain: domain(source) }];
        for low in 0..9 {
            changes.push(WitnessChange::Key { domain: domain(source), key: low });
            for high in low + 1..10 { changes.push(WitnessChange::Range { domain: domain(source), start: low, end: high }); }
        }
        for change in changes {
            let expected: Vec<_> = original.iter().enumerate().filter(|(_, j)| scan(j, false, change)).map(|(id, _)| id as u64).collect();
            assert_eq!(index.affected(change, unlimited()).candidates.unwrap(), expected, "{change:?}");
        }
    }
}

#[test]
fn exact_revalidation_never_finds_a_changed_dependency_outside_the_routed_set() {
    let d = domain(1);
    let (old, frontiers) = basis(d, 1, vec![SnapshotEntry::new(0, 1, b"old".to_vec()).unwrap(), SnapshotEntry::new(4, 1, vec![]).unwrap()]);
    let requests = [WitnessRequest::ExactValue { key: 0, role: QueryRole::PolicyInput },
        WitnessRequest::AbsentKey { key: 1 }, WitnessRequest::EmptyRange { start: 6, end: 9 },
        WitnessRequest::RangeMembers { start: 2, end: 6 }];
    let judgments: Vec<_> = requests.into_iter().map(|r| WitnessJudgment::capture(&old, &frontiers, vec![r]).unwrap()).collect();
    let mut index = index();
    for (id, j) in judgments.iter().enumerate() { index.register(id as u64, Some(j), false).unwrap(); }
    for key in 0..11 {
        for remove in [false, true] {
            let mut values = vec![(0, 1, b"old".to_vec()), (4, 1, vec![])];
            values.retain(|(k, _, _)| *k != key);
            if !remove { values.push((key, 2, b"changed".to_vec())); }
            let (new, frontier) = basis(d, 2, values.into_iter().map(|(k, v, bytes)| SnapshotEntry::new(k, v, bytes).unwrap()).collect());
            let candidates = index.affected(WitnessChange::Key { domain: d, key }, unlimited()).candidates.unwrap();
            for (id, j) in judgments.iter().enumerate() {
                if matches!(j.reuse_at(&new, &frontier).unwrap(), Reuse::Invalidated { .. }) {
                    assert!(candidates.contains(&(id as u64)), "key {key}, remove {remove}, dependency {id}");
                }
            }
        }
    }
}

#[test]
fn opaque_inputs_are_never_narrowed_and_semantic_or_closure_changes_reach_empty_footprints() {
    let d = domain(1);
    let j = judgment(d, vec![WitnessRequest::AbsentKey { key: 3 }]);
    let empty = judgment(d, vec![]);
    let mut index = index();
    index.register(8, Some(&j), false).unwrap();
    index.register(2, Some(&j), true).unwrap();
    index.register(99, None, true).unwrap();
    index.register(4, Some(&empty), false).unwrap();
    assert_eq!(index.affected(WitnessChange::Key { domain: domain(2), key: 100 }, unlimited()).candidates.unwrap(), vec![2, 99]);
    assert_eq!(index.affected(WitnessChange::Domain { domain: d }, unlimited()).candidates.unwrap(), vec![8, 2, 99, 4]);
    for new in [DomainProjection::new(d.domain_id(), 5, d.projection()),
        DomainProjection::new(d.domain_id(), d.domain_epoch(), ProjectionKey { source_epoch: 8, ..d.projection() })] {
        assert_eq!(index.affected(WitnessChange::Key { domain: new, key: 100 }, unlimited()).candidates.unwrap(), vec![8, 2, 99, 4]);
    }
}

#[test]
fn half_open_boundaries_maximum_key_and_overlapping_dependencies_are_exact() {
    let d = domain(1);
    let j = judgment(d, vec![WitnessRequest::AbsentKey { key: u64::MAX },
        WitnessRequest::EmptyRange { start: u64::MAX - 4, end: u64::MAX },
        WitnessRequest::RangeMembers { start: u64::MAX - 2, end: u64::MAX }]);
    let mut index = index(); index.register(1, Some(&j), false).unwrap();
    for key in [u64::MAX - 5, u64::MAX - 4, u64::MAX - 1, u64::MAX] {
        let ids = index.affected(WitnessChange::Key { domain: d, key }, unlimited()).candidates.unwrap();
        assert_eq!(ids, if key == u64::MAX - 5 { vec![] } else { vec![1] });
    }
    assert_eq!(index.affected(WitnessChange::Range { domain: d, start: 0, end: u64::MAX - 4 }, unlimited()).candidates.unwrap(), vec![]);
    assert_eq!(index.affected(WitnessChange::Range { domain: d, start: 3, end: 3 }, unlimited()).candidates, Err(Error::InvalidInput));
}

#[test]
fn budget_exhaustion_never_exposes_a_partial_answer_or_mutates_the_index() {
    let mut index = index();
    for id in 0..4 { index.register(id, None, true).unwrap(); }
    let full = index.affected(WitnessChange::All, unlimited());
    assert_eq!(index.affected(WitnessChange::All, full.spent), full);
    for budget in [RoutingBudget::default(), RoutingBudget { steps: full.spent.steps - 1, bytes: full.spent.bytes },
        RoutingBudget { steps: full.spent.steps, bytes: full.spent.bytes - 1 }] {
        let partial = index.affected(WitnessChange::All, budget);
        assert_eq!(partial.candidates, Err(Error::Incomplete));
        assert!(partial.spent.steps <= budget.steps && partial.spent.bytes <= budget.bytes);
    }
    assert_eq!(index.affected(WitnessChange::All, unlimited()), full);
}

#[test]
fn construction_limits_and_duplicate_ids_leave_prior_routes_intact() {
    let j = judgment(domain(1), vec![WitnessRequest::AbsentKey { key: 1 }]);
    let mut index = InvalidationIndex::new(RoutingLimits { judgments: 2, dependencies: 1 }).unwrap();
    index.register(7, Some(&j), false).unwrap();
    let before = index.affected(WitnessChange::All, unlimited());
    assert_eq!(index.register(7, None, true), Err(Error::Duplicate));
    assert_eq!(index.register(8, Some(&j), false), Err(Error::Limit));
    assert_eq!(index.register(8, None, false), Err(Error::Incomplete));
    assert_eq!(index.affected(WitnessChange::All, unlimited()), before);
    index.register(8, None, true).unwrap();
    assert_eq!(index.register(9, None, true), Err(Error::Limit));
    assert_eq!(index.registered(), 2); assert_eq!(index.dependencies(), 1);
}

#[test]
fn lookup_cost_does_not_read_values_and_disjoint_interval_prefixes_are_pruned() {
    let mut reports = Vec::new();
    for bytes in [vec![], vec![9; MAX_VALUE_BYTES]] {
        let (snapshot, frontiers) = basis(domain(1), 1, vec![SnapshotEntry::new(2, 1, bytes).unwrap()]);
        let j = WitnessJudgment::capture(&snapshot, &frontiers,
            vec![WitnessRequest::ExactValue { key: 2, role: QueryRole::Subject }]).unwrap();
        let mut index = index(); index.register(1, Some(&j), false).unwrap();
        reports.push(index.affected(WitnessChange::Key { domain: domain(1), key: 2 }, unlimited()));
    }
    assert_eq!(reports[0], reports[1]);
    let mut index = index();
    for id in 0..8 {
        let j = judgment(domain(1), (0..32).map(|k| {
            let start = (id * 32 + k) * 4;
            WitnessRequest::EmptyRange { start, end: start + 2 }
        }).collect());
        index.register(id, Some(&j), false).unwrap();
    }
    let report = index.affected(WitnessChange::Key { domain: domain(1), key: 1023 }, unlimited());
    assert_eq!(report.candidates.as_ref().unwrap(), &Vec::<u64>::new());
    assert!(report.spent.steps < 40, "{report:?}");
    assert_eq!(index.affected(WitnessChange::Key { domain: domain(1), key: 1020 }, unlimited()).candidates.unwrap(), vec![7]);
}
