use super::*;
use crate::product_frontier::{FrontierStage, ProjectionKey, TrustedClosingMarker};
use crate::witness::{AdapterDomainInput, DomainClosure, DomainProjection, Invalidation, QueryRole, Reuse, SnapshotEntry};
use crate::witness::refinement::{RefinementBudget, RefinementOutcome, RefinementReport};

fn fixture(generation: u64) -> (ProductFrontiers, AdapterDomainInput) {
    let key = ProjectionKey { source: 1, branch: 2, projection: 3, source_epoch: 4 };
    let mut frontiers = ProductFrontiers::new(1, 8).unwrap();
    frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap();
    let marker = TrustedClosingMarker { key, final_sequence: 1, marker_generation: generation };
    frontiers.record_close(marker).unwrap();
    (frontiers, AdapterDomainInput::new(DomainProjection::new(1, 1, key), DomainClosure::Closed(marker)))
}

fn snapshot(domain: AdapterDomainInput, revision: u64, keys: &[u64]) -> WitnessSnapshot {
    WitnessSnapshot::new(revision, revision, 7, domain, keys.iter().map(|key| {
        SnapshotEntry::new(*key, 1, b"value".to_vec()).unwrap()
    }).collect()).unwrap()
}

fn requests() -> Vec<WitnessRequest> {
    vec![
        WitnessRequest::ExactValue { key: 0, role: QueryRole::PolicyInput },
        WitnessRequest::AbsentKey { key: 1 },
        WitnessRequest::RangeMembers { start: 2, end: 6 },
        WitnessRequest::EmptyRange { start: 6, end: 9 },
    ]
}

fn finish(mut cursor: WitnessRefinement<'_>, budget: RefinementBudget) -> RefinementReport {
    for _ in 0..100_000 {
        let before = cursor.total_work();
        let report = cursor.advance(budget);
        assert!(report.spent.steps <= budget.steps);
        assert!(report.spent.value_bytes <= budget.value_bytes);
        assert_eq!(report.total.steps, before.steps + report.spent.steps);
        assert_eq!(report.total.value_bytes, before.value_bytes + report.spent.value_bytes);
        if !matches!(report.outcome, RefinementOutcome::NeedsRefinement { .. }) {
            return report;
        }
        assert_ne!(report.spent, RefinementBudget::default());
    }
    panic!("bounded validation did not terminate");
}

fn ample() -> RefinementBudget {
    RefinementBudget { steps: u64::MAX, value_bytes: u64::MAX }
}

fn assert_eager(capture: &IndexedJudgment, current: &WitnessSnapshot, frontiers: &ProductFrontiers, outcome: RefinementOutcome) {
    match (capture.judgment().reuse_at(current, frontiers), outcome) {
        (Ok(Reuse::StillValid { .. }), RefinementOutcome::StillValid) => {}
        (Ok(Reuse::Invalidated { reason: eager, .. }), RefinementOutcome::Invalidated { reason, .. }) => assert_eq!(reason, eager),
        (Err(eager), RefinementOutcome::Refused(error)) => assert_eq!(error, eager),
        pair => panic!("indexed/eager mismatch: {pair:?}"),
    }
}

#[test]
fn unrelated_changes_skip_payload_comparison_but_not_current_closure() {
    let (frontiers, domain) = fixture(1);
    let first = snapshot(domain, 10, &[0, 2, 4]);
    let next = snapshot(domain, 11, &[0, 2, 4, 9]);
    let mut index = WitnessChangeIndex::new(&first, &frontiers, 2).unwrap();
    let capture = index.capture(&frontiers, requests()).unwrap();
    let ingestion = index.observe(&next, &frontiers).unwrap();
    assert_eq!(ingestion, ChangeCost { point_lookups: 7, value_bytes: 15, changed_keys: 1 });
    let (plan, cursor) = index.begin_refinement(&capture, &next, &frontiers, 4);
    assert_eq!(plan.selection, IndexSelection::Candidates { count: 0 });
    assert_eq!(plan.dependency_checks, 4);
    let report = finish(cursor, RefinementBudget { steps: 1, value_bytes: 0 });
    assert_eq!(report.outcome, RefinementOutcome::StillValid);
    assert_eq!(report.total.value_bytes, 0);
    let exact = finish(capture.judgment().begin_refinement(&next, &frontiers), ample());
    assert_eq!(exact.total.value_bytes, 15);
    let unavailable = ProductFrontiers::new(1, 8).unwrap();
    let (plan, cursor) = index.begin_refinement(&capture, &next, &unavailable, 4);
    assert_eq!(plan.selection, IndexSelection::Candidates { count: 0 });
    assert_eq!(finish(cursor, ample()).outcome, RefinementOutcome::Refused(Error::Incomplete));
}

#[test]
fn mutation_matrix_matches_eager_with_index_and_planning_fallback() {
    let (frontiers, domain) = fixture(1);
    let first = snapshot(domain, 10, &[0, 2, 4]);
    for key in 0..10 {
        for operation in 0..5 {
            let mut next = first.clone();
            next.revision += 1;
            next.control_cut += 1;
            match operation {
                0 => { next.values.remove(&key); }
                1 => { next.values.insert(key, SnapshotEntry::new(key, 1, b"value".to_vec()).unwrap()); }
                2 => { next.values.insert(key, SnapshotEntry::new(key, 2, b"value".to_vec()).unwrap()); }
                3 => { next.values.insert(key, SnapshotEntry::new(key, 1, b"other".to_vec()).unwrap()); }
                4 => { next.values.insert(key, SnapshotEntry::new(key, 1, vec![]).unwrap()); }
                _ => unreachable!(),
            }
            let mut index = WitnessChangeIndex::new(&first, &frontiers, 2).unwrap();
            let capture = index.capture(&frontiers, requests()).unwrap();
            index.observe(&next, &frontiers).unwrap();
            for budget in [0, 1, u64::MAX] {
                let (plan, cursor) = index.begin_refinement(&capture, &next, &frontiers, budget);
                assert!(plan.dependency_checks <= budget);
                let result = finish(cursor, RefinementBudget { steps: 1, value_bytes: 1 });
                assert_eager(&capture, &next, &frontiers, result.outcome);
            }
        }
    }
}

#[test]
fn candidate_membership_does_not_mislabel_an_aba_as_final_invalidation() {
    let (frontiers, domain) = fixture(1);
    let first = snapshot(domain, 10, &[0, 2, 4]);
    let changed = snapshot(domain, 11, &[0, 1, 2, 4]);
    let restored = snapshot(domain, 12, &[0, 2, 4]);
    let mut index = WitnessChangeIndex::new(&first, &frontiers, 2).unwrap();
    let capture = index.capture(&frontiers, requests()).unwrap();
    index.observe(&changed, &frontiers).unwrap();
    index.observe(&restored, &frontiers).unwrap();
    let (plan, cursor) = index.begin_refinement(&capture, &restored, &frontiers, 100);
    assert_eq!(plan.selection, IndexSelection::Candidates { count: 1 });
    assert_eq!(finish(cursor, ample()).outcome, RefinementOutcome::StillValid);
}

#[test]
fn omitted_revision_refuses_ingestion_and_explicit_target_uses_exact_fallback() {
    let (frontiers, domain) = fixture(1);
    let first = snapshot(domain, 10, &[0, 2, 4]);
    let gap = snapshot(domain, 12, &[0, 1, 2, 4]);
    let mut index = WitnessChangeIndex::new(&first, &frontiers, 2).unwrap();
    let capture = index.capture(&frontiers, requests()).unwrap();
    assert_eq!(index.observe(&gap, &frontiers), Err(Error::Stale));
    assert_eq!(index.current_revision(), 10);
    assert_eq!(index.retained_revisions(), 0);
    let (plan, cursor) = index.begin_refinement(&capture, &gap, &frontiers, 100);
    assert_eq!(plan.selection, IndexSelection::ExactFallback(IndexFallback::TargetNotIndexed));
    assert_eq!(finish(cursor, ample()).outcome, RefinementOutcome::Invalidated {
        reason: Invalidation::AbsentKey, witness: Some(1),
    });
}

#[test]
fn evicted_history_never_becomes_an_empty_change_set() {
    let (frontiers, domain) = fixture(1);
    let first = snapshot(domain, 10, &[0, 2, 4]);
    let changed = snapshot(domain, 11, &[0, 1, 2, 4]);
    let next = snapshot(domain, 12, &[0, 1, 2, 4, 9]);
    let mut index = WitnessChangeIndex::new(&first, &frontiers, 1).unwrap();
    let capture = index.capture(&frontiers, requests()).unwrap();
    index.observe(&changed, &frontiers).unwrap();
    index.observe(&next, &frontiers).unwrap();
    assert_eq!(index.retained_revisions(), 1);
    assert_eq!(index.oldest_covered_revision(), 11);
    let (plan, cursor) = index.begin_refinement(&capture, &next, &frontiers, 100);
    assert_eq!(plan.selection, IndexSelection::ExactFallback(IndexFallback::HistoryEvicted));
    assert_eq!(finish(cursor, ample()).outcome, RefinementOutcome::Invalidated {
        reason: Invalidation::AbsentKey, witness: Some(1),
    });
}

#[test]
fn planning_exhaustion_discards_all_partial_skip_decisions() {
    let (frontiers, domain) = fixture(1);
    let first = snapshot(domain, 10, &[0, 2, 4]);
    let next = snapshot(domain, 11, &[0, 2, 4, 7]);
    let mut index = WitnessChangeIndex::new(&first, &frontiers, 1).unwrap();
    let capture = index.capture(&frontiers, requests()).unwrap();
    index.observe(&next, &frontiers).unwrap();
    let (plan, cursor) = index.begin_refinement(&capture, &next, &frontiers, 3);
    assert_eq!(plan.dependency_checks, 3);
    assert_eq!(plan.selection, IndexSelection::ExactFallback(IndexFallback::PlanningBudget));
    let result = finish(cursor, ample());
    assert_eq!(result.total.value_bytes, 15);
    assert_eq!(result.outcome, RefinementOutcome::Invalidated {
        reason: Invalidation::EmptyRange, witness: Some(3),
    });
}

#[test]
fn unknown_and_summary_snapshots_never_enter_history_or_establish_absence() {
    let (frontiers, domain) = fixture(1);
    let first = snapshot(domain, 10, &[0, 2, 4]);
    for closure in [DomainClosure::Unknown, DomainClosure::ConservativeSummary] {
        let unknown = snapshot(AdapterDomainInput::new(domain.domain(), closure), 11, &[0, 2, 4]);
        assert!(matches!(WitnessChangeIndex::new(&unknown, &frontiers, 1), Err(Error::Incomplete)));
        let mut index = WitnessChangeIndex::new(&first, &frontiers, 1).unwrap();
        let capture = index.capture(&frontiers, requests()).unwrap();
        assert_eq!(index.observe(&unknown, &frontiers), Err(Error::Incomplete));
        let (plan, cursor) = index.begin_refinement(&capture, &unknown, &frontiers, 100);
        assert_eq!(plan.selection, IndexSelection::ExactFallback(IndexFallback::TargetNotIndexed));
        assert_eq!(finish(cursor, ample()).outcome, RefinementOutcome::Refused(Error::Incomplete));
        let exact = index.capture(&frontiers, vec![WitnessRequest::ExactValue { key: 0, role: QueryRole::Subject }]).unwrap();
        let (_, cursor) = index.begin_refinement(&exact, &unknown, &frontiers, 100);
        assert_eq!(finish(cursor, ample()).outcome, RefinementOutcome::StillValid);
    }
}

#[test]
fn equal_numbered_replacements_and_foreign_histories_cannot_forge_capture_binding() {
    let (frontiers, domain) = fixture(1);
    let first = snapshot(domain, 10, &[0, 2, 4]);
    let forged = snapshot(domain, 10, &[0, 1, 2, 4]);
    let index = WitnessChangeIndex::new(&first, &frontiers, 1).unwrap();
    let capture = index.capture(&frontiers, requests()).unwrap();
    let (plan, cursor) = index.begin_refinement(&capture, &forged, &frontiers, 0);
    assert_eq!(plan.selection, IndexSelection::ExactFallback(IndexFallback::TargetNotIndexed));
    assert!(matches!(finish(cursor, ample()).outcome, RefinementOutcome::Invalidated { reason: Invalidation::AbsentKey, .. }));
    let foreign = WitnessChangeIndex::new(&forged, &frontiers, 1).unwrap();
    let (plan, cursor) = foreign.begin_refinement(&capture, &forged, &frontiers, 0);
    assert_eq!(plan.selection, IndexSelection::ExactFallback(IndexFallback::ForeignHistory));
    assert!(matches!(finish(cursor, ample()).outcome, RefinementOutcome::Invalidated { reason: Invalidation::AbsentKey, .. }));
}

#[test]
fn a_changed_closing_marker_invalidates_even_when_no_keys_changed() {
    let (frontiers, domain) = fixture(1);
    let (new_frontiers, new_domain) = fixture(2);
    let first = snapshot(domain, 10, &[0, 2, 4]);
    let next = snapshot(new_domain, 11, &[0, 2, 4]);
    let mut index = WitnessChangeIndex::new(&first, &frontiers, 1).unwrap();
    let capture = index.capture(&frontiers, requests()).unwrap();
    assert_eq!(index.observe(&next, &new_frontiers).unwrap().changed_keys, 0);
    let (plan, cursor) = index.begin_refinement(&capture, &next, &new_frontiers, 100);
    assert_eq!(plan.selection, IndexSelection::Candidates { count: 0 });
    assert_eq!(finish(cursor, ample()).outcome, RefinementOutcome::Invalidated {
        reason: Invalidation::ClosingFrontier, witness: Some(1),
    });
}

#[test]
fn profile_changes_and_regressions_preserve_history_and_exact_basis_checks() {
    let (frontiers, domain) = fixture(1);
    let first = snapshot(domain, 10, &[0, 2, 4]);
    for case in 0..6 {
        let mut next = snapshot(domain, 11, &[0, 2, 4]);
        let expected = match case {
            0 => { next.semantic_epoch += 1; Error::Binding }
            1 => { next.domain_input.domain.domain_id += 1; Error::Binding }
            2 => { next.domain_input.domain.domain_epoch += 1; Error::Binding }
            3 => { next.domain_input.domain.projection.branch += 1; Error::Binding }
            4 => { next.control_cut = 9; Error::Stale }
            5 => { next.revision = 9; Error::Stale }
            _ => unreachable!(),
        };
        let mut index = WitnessChangeIndex::new(&first, &frontiers, 1).unwrap();
        let capture = index.capture(&frontiers, requests()).unwrap();
        assert_eq!(index.observe(&next, &frontiers), Err(expected));
        assert_eq!(index.current_revision(), 10);
        let (_, cursor) = index.begin_refinement(&capture, &next, &frontiers, 100);
        assert_eager(&capture, &next, &frontiers, finish(cursor, ample()).outcome);
    }
}

#[test]
fn capacity_revision_overflow_and_zero_work_are_explicit() {
    let (frontiers, domain) = fixture(1);
    let first = snapshot(domain, u64::MAX, &[0, 2, 4]);
    for capacity in [0, MAX_CHANGE_DELTAS + 1, usize::MAX] {
        assert!(matches!(WitnessChangeIndex::new(&first, &frontiers, capacity), Err(Error::Limit)));
    }
    let mut index = WitnessChangeIndex::new(&first, &frontiers, 1).unwrap();
    assert_eq!(index.observe(&first, &frontiers), Err(Error::Overflow));
    let capture = index.capture(&frontiers, requests()).unwrap();
    let (plan, mut cursor) = index.begin_refinement(&capture, &first, &frontiers, 0);
    assert_eq!(plan.selection, IndexSelection::Candidates { count: 0 });
    assert_eq!(plan.dependency_checks, 0);
    let zero = cursor.advance(RefinementBudget::default());
    assert!(matches!(zero.outcome, RefinementOutcome::NeedsRefinement { .. }));
    assert_eq!(finish(cursor, RefinementBudget { steps: 1, value_bytes: 0 }).outcome, RefinementOutcome::StillValid);
}
