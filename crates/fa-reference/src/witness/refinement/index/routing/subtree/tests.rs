use super::*;
use super::super::{RoutingBudget, RoutingLimits, RoutingStrategy, WitnessChange, MAX_ROUTED_DEPENDENCIES};
use crate::product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker};
use crate::witness::{AdapterDomainInput, DomainClosure, DomainProjection, QueryRole, Reuse,
    SnapshotEntry, Witness, WitnessJudgment, WitnessRequest, WitnessSnapshot, MAX_WITNESSES};

fn domain(source: u64) -> DomainProjection {
    DomainProjection::new(3, 4, ProjectionKey { source, branch: 5, projection: 6, source_epoch: 7 })
}
fn basis(d: DomainProjection, revision: u64, entries: Vec<SnapshotEntry>) -> (WitnessSnapshot, ProductFrontiers) {
    let close = TrustedClosingMarker { key: d.projection(), final_sequence: 1, marker_generation: 1 };
    let mut frontiers = ProductFrontiers::new(1, 1).unwrap();
    frontiers.accept(close.key, FrontierStage::Authenticated, 1).unwrap();
    frontiers.record_close(close).unwrap();
    (WitnessSnapshot::new(revision, revision, 1,
        AdapterDomainInput::new(d, DomainClosure::Closed(close)), entries).unwrap(), frontiers)
}
fn original(d: DomainProjection, requests: Vec<WitnessRequest>) -> WitnessJudgment {
    let rows = requests.iter().filter_map(|r| match r {
        WitnessRequest::ExactValue { key, .. } => Some(SnapshotEntry::new(*key, 1, b"old".to_vec()).unwrap()),
        _ => None,
    }).collect();
    let (snapshot, frontiers) = basis(d, 1, rows);
    WitnessJudgment::capture(&snapshot, &frontiers, requests).unwrap()
}
fn limits() -> RoutingLimits { RoutingLimits { judgments: MAX_ROUTED_JUDGMENTS, dependencies: MAX_ROUTED_DEPENDENCIES } }
fn unlimited() -> RoutingBudget { RoutingBudget { steps: u64::MAX, bytes: u64::MAX } }
fn tree() -> InvalidationIndex { InvalidationIndex::new_with_strategy(limits(), RoutingStrategy::SubtreeV2).unwrap() }

// Independent eager dependency scan. No sorted arrays, summaries, DomainKey,
// production lookup helper or notification-derived exact-validation mask.
fn scan(j: &WitnessJudgment, opaque: bool, change: WitnessChange) -> bool {
    if opaque || change == WitnessChange::All { return true; }
    let new = match change {
        WitnessChange::Key { domain, .. } | WitnessChange::Range { domain, .. }
        | WitnessChange::Domain { domain } => domain,
        WitnessChange::All => unreachable!(),
    };
    let old = j.domain; let a = old.projection(); let b = new.projection();
    if old.domain_id() != new.domain_id() || a.source != b.source || a.branch != b.branch || a.projection != b.projection {
        return false;
    }
    if old != new || matches!(change, WitnessChange::Domain { .. }) { return true; }
    j.witnesses.iter().any(|w| match (w, change) {
        (Witness::ExactValue { key, .. } | Witness::AbsentKey { key, .. }, WitnessChange::Key { key: changed, .. }) => *key == changed,
        (Witness::ExactValue { key, .. } | Witness::AbsentKey { key, .. }, WitnessChange::Range { start, end, .. }) => start <= *key && *key < end,
        (Witness::EmptyRange { start, end, .. } | Witness::RangeMembers { start, end, .. }, WitnessChange::Key { key, .. }) => *start <= key && key < *end,
        (Witness::EmptyRange { start, end, .. } | Witness::RangeMembers { start, end, .. }, WitnessChange::Range { start: a, end: b, .. }) => *start < b && a < *end,
        _ => false,
    })
}

#[test]
fn subtree_routes_match_full_scan_with_domains_epochs_and_nonmonotone_owner_ids() {
    let mut index = tree(); let mut originals = Vec::new();
    for source in 0..3 {
        for low in 0..8 {
            let d = domain(source);
            let j = original(d, vec![
                WitnessRequest::ExactValue { key: 100 + low, role: QueryRole::Subject },
                WitnessRequest::AbsentKey { key: 200 + low },
                WitnessRequest::EmptyRange { start: low, end: low + 3 },
                WitnessRequest::RangeMembers { start: low + 1, end: low + 7 },
            ]);
            let id = 1000 - originals.len() as u64;
            let opaque = low == 7;
            index.register(id, Some(&j), opaque).unwrap();
            originals.push((id, j, opaque));
        }
    }
    for source in 0..4 {
        for d in [domain(source), DomainProjection::new(3, 5, domain(source).projection()),
            DomainProjection::new(3, 4, ProjectionKey { source_epoch: 8, ..domain(source).projection() })] {
            let mut changes = vec![WitnessChange::All, WitnessChange::Domain { domain: d }];
            for low in 0..18 {
                changes.push(WitnessChange::Key { domain: d, key: low });
                for end in low + 1..20 { changes.push(WitnessChange::Range { domain: d, start: low, end }); }
            }
            for key in 99..210 { changes.push(WitnessChange::Key { domain: d, key }); }
            for change in changes {
                let expected: Vec<_> = originals.iter().filter(|(_, j, opaque)| scan(j, *opaque, change)).map(|(id, _, _)| *id).collect();
                assert_eq!(index.affected(change, unlimited()).candidates.unwrap(), expected, "{change:?}");
            }
        }
    }
}

fn broad_index(strategy: RoutingStrategy) -> InvalidationIndex {
    let mut index = InvalidationIndex::new_with_strategy(limits(), strategy).unwrap();
    for id in 0..16 {
        let requests = (0..MAX_WITNESSES).map(|k| {
            if id == 0 && k == 0 { WitnessRequest::EmptyRange { start: 0, end: u64::MAX } }
            else { let start = (id * MAX_WITNESSES + k) as u64 * 4;
                WitnessRequest::EmptyRange { start, end: start + 1 } }
        }).collect();
        index.register(700 + id as u64, Some(&original(domain(1), requests)), false).unwrap();
    }
    index
}

#[test]
fn one_long_range_cannot_force_a_scan_of_a_thousand_disjoint_narrow_ranges() {
    let change = WitnessChange::Key { domain: domain(1), key: 1_000_000 };
    let legacy = broad_index(RoutingStrategy::PrefixV1);
    let index = broad_index(RoutingStrategy::SubtreeV2);
    let budget = RoutingBudget { steps: 100, bytes: 8_000 };
    assert_eq!(legacy.affected(change, budget).candidates, Err(Error::Incomplete));
    let report = index.affected(change, budget);
    assert_eq!(report.candidates.as_ref().unwrap(), &vec![700]);
    assert_eq!(legacy.affected(change, unlimited()).candidates, report.candidates);
    assert!(report.spent.steps <= budget.steps && report.spent.bytes <= budget.bytes);
    assert!(legacy.affected(change, unlimited()).spent.steps > 1000);
    assert!(index.subtrees.len() <= MAX_ROUTED_DEPENDENCIES);
}

#[test]
fn already_selected_owner_summaries_bound_dense_overlap_without_losing_other_owners() {
    let mut legacy = InvalidationIndex::new(limits()).unwrap(); let mut index = tree();
    for id in 0..MAX_ROUTED_JUDGMENTS {
        let j = original(domain(1), (0..MAX_WITNESSES).map(|k| {
            let start = (id * MAX_WITNESSES + k) as u64;
            WitnessRequest::RangeMembers { start, end: 1_000_000 }
        }).collect());
        legacy.register(9000 - id as u64, Some(&j), false).unwrap();
        index.register(9000 - id as u64, Some(&j), false).unwrap();
    }
    let change = WitnessChange::Key { domain: domain(1), key: 500_000 };
    let old = legacy.affected(change, unlimited()); let new = index.affected(change, unlimited());
    assert_eq!(new.candidates, old.candidates);
    assert_eq!(new.candidates.as_ref().unwrap().len(), MAX_ROUTED_JUDGMENTS);
    assert!(new.spent.steps * 4 < old.spent.steps, "new={new:?}; old={old:?}");
    assert!(new.spent.bytes * 4 < old.spent.bytes);
}

#[test]
fn tree_budget_edges_never_leak_partial_candidates_and_default_legacy_costs_are_unchanged() {
    let index = broad_index(RoutingStrategy::SubtreeV2);
    let change = WitnessChange::Key { domain: domain(1), key: 1_000_000 };
    let full = index.affected(change, unlimited());
    assert_eq!(index.affected(change, full.spent), full);
    for budget in [RoutingBudget::default(), RoutingBudget { steps: full.spent.steps - 1, bytes: full.spent.bytes },
        RoutingBudget { steps: full.spent.steps, bytes: full.spent.bytes - 1 }] {
        let report = index.affected(change, budget);
        assert_eq!(report.candidates, Err(Error::Incomplete));
        assert!(report.spent.steps <= budget.steps && report.spent.bytes <= budget.bytes);
    }
    assert_eq!(index.affected(change, unlimited()), full);
    let mut default = InvalidationIndex::new(limits()).unwrap();
    let mut explicit = InvalidationIndex::new_with_strategy(limits(), RoutingStrategy::PrefixV1).unwrap();
    let j = original(domain(1), vec![WitnessRequest::EmptyRange { start: 0, end: 9 }]);
    for i in [&mut default, &mut explicit] { i.register(1, Some(&j), false).unwrap(); }
    assert_eq!(default.strategy(), RoutingStrategy::PrefixV1);
    assert!(default.subtrees.is_empty());
    for steps in 0..10 {
        for bytes in [0, 72, 80, 160, 280, 1024] {
            let budget = RoutingBudget { steps, bytes };
            assert_eq!(default.affected(change, budget), explicit.affected(change, budget));
        }
    }
}

#[test]
fn failed_registration_preserves_all_summaries_and_successful_registration_rebuilds_them() {
    let mut index = InvalidationIndex::new_with_strategy(RoutingLimits { judgments: 3, dependencies: 3 }, RoutingStrategy::SubtreeV2).unwrap();
    let first = original(domain(1), vec![WitnessRequest::EmptyRange { start: 0, end: u64::MAX }]);
    index.register(9, Some(&first), false).unwrap();
    let change = WitnessChange::Key { domain: domain(1), key: 1000 };
    let before = index.affected(change, unlimited());
    assert_eq!(index.register(9, None, true), Err(Error::Duplicate));
    let large = original(domain(1), (1..4).map(|start| WitnessRequest::EmptyRange { start, end: 2000 }).collect());
    assert_eq!(index.register(10, Some(&large), false), Err(Error::Limit));
    assert_eq!(index.affected(change, unlimited()), before);
    for id in [3, 2] { index.register(id, Some(&first), false).unwrap(); }
    assert_eq!(index.affected(change, unlimited()).candidates.unwrap(), vec![9, 3, 2]);
    assert_eq!(index.register(4, None, true), Err(Error::Limit));
}

#[test]
fn upper_key_and_half_open_ranges_have_no_overflow_or_shared_owner_false_negative() {
    let mut index = tree();
    let a = original(domain(1), vec![WitnessRequest::EmptyRange { start: 0, end: u64::MAX },
        WitnessRequest::AbsentKey { key: u64::MAX }]);
    let b = original(domain(1), vec![WitnessRequest::RangeMembers { start: u64::MAX - 1, end: u64::MAX }]);
    index.register(8, Some(&a), false).unwrap(); index.register(2, Some(&b), false).unwrap();
    for key in [0, u64::MAX - 2, u64::MAX - 1, u64::MAX] {
        let change = WitnessChange::Key { domain: domain(1), key };
        let expected: Vec<_> = [(8, &a), (2, &b)].into_iter().filter(|(_, j)| scan(j, false, change)).map(|(id, _)| id).collect();
        assert_eq!(index.affected(change, unlimited()).candidates.unwrap(), expected);
    }
    for (start, end) in [(0, 1), (1, u64::MAX - 1), (u64::MAX - 1, u64::MAX)] {
        let change = WitnessChange::Range { domain: domain(1), start, end };
        let expected: Vec<_> = [(8, &a), (2, &b)].into_iter().filter(|(_, j)| scan(j, false, change)).map(|(id, _)| id).collect();
        assert_eq!(index.affected(change, unlimited()).candidates.unwrap(), expected);
    }
    assert_eq!(index.affected(WitnessChange::Range { domain: domain(1), start: 9, end: 9 }, unlimited()).candidates, Err(Error::InvalidInput));
}

#[test]
fn every_eager_invalidation_is_routed_including_new_negative_evidence() {
    let (old, frontier) = basis(domain(1), 1, vec![]);
    let originals: Vec<_> = (0..20).map(|start| WitnessJudgment::capture(&old, &frontier, vec![
        WitnessRequest::EmptyRange { start, end: start + 3 },
        WitnessRequest::RangeMembers { start: start + 4, end: start + 8 },
    ]).unwrap()).collect();
    let mut index = tree();
    for (id, j) in originals.iter().enumerate() { index.register(id as u64, Some(j), false).unwrap(); }
    for key in 0..30 {
        let (new, frontier) = basis(domain(1), 2, vec![SnapshotEntry::new(key, 1, vec![5]).unwrap()]);
        let routed = index.affected(WitnessChange::Key { domain: domain(1), key }, unlimited()).candidates.unwrap();
        for (id, j) in originals.iter().enumerate() {
            let changed = matches!(j.reuse_at(&new, &frontier).unwrap(), Reuse::Invalidated { .. });
            assert_eq!(routed.contains(&(id as u64)), changed);
        }
    }
}

#[test]
fn opaque_and_semantically_invalidated_registrations_skip_no_unselected_owner() {
    let mut index = tree();
    let j = original(domain(1), (0..MAX_WITNESSES).map(|n| WitnessRequest::EmptyRange { start: n as u64, end: 1000 }).collect());
    index.register(5, Some(&j), true).unwrap();
    index.register(8, Some(&j), false).unwrap();
    index.register(1, None, true).unwrap();
    let outside = WitnessChange::Key { domain: domain(2), key: 500 };
    assert_eq!(index.affected(outside, unlimited()).candidates.unwrap(), vec![5, 1]);
    let changed_epoch = WitnessChange::Key { domain: DomainProjection::new(3, 9, domain(1).projection()), key: 500 };
    let all = index.affected(changed_epoch, unlimited());
    assert_eq!(all.candidates.unwrap(), vec![5, 8, 1]);
    // Header + two registration passes + one summary + three output records.
    assert_eq!(all.spent, RoutingBudget { steps: 11, bytes: 72 + 6 * 80 + SUMMARY_BYTES + 3 * 8 });
}
