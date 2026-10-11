//! Public-API controls for cooperative complete-tail ingestion and exact reuse.
//! These tests do not authenticate adapters or exercise production authority.
use fa_reference::Error;
use fa_reference::product_frontier::{
    FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker,
};
use fa_reference::witness::{
    AdapterDomainInput, DomainClosure, DomainProjection, Invalidation, QueryRole, Reuse,
    SnapshotEntry, WitnessRequest, WitnessSnapshot,
};
use fa_reference::witness::refinement::{RefinementBudget, RefinementOutcome, WitnessRefinement};
use fa_reference::witness::refinement::index::{
    ChangeCost, ChangeObservationOutcome, IndexFallback, IndexSelection, IndexedJudgment,
    WitnessChangeIndex,
};

fn fixture(generation: u64) -> (ProductFrontiers, AdapterDomainInput) {
    let key = ProjectionKey { source: 1, branch: 2, projection: 3, source_epoch: 4 };
    let mut frontiers = ProductFrontiers::new(1, 8).unwrap();
    frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap();
    let marker = TrustedClosingMarker { key, final_sequence: 1, marker_generation: generation };
    frontiers.record_close(marker).unwrap();
    let input = AdapterDomainInput::new(DomainProjection::new(1, 1, key), DomainClosure::Closed(marker));
    (frontiers, input)
}

fn rows() -> Vec<(u64, u64, Vec<u8>)> {
    vec![(0, 1, b"value".to_vec()), (2, 1, b"value".to_vec()), (4, 1, b"value".to_vec())]
}

fn snapshot(input: AdapterDomainInput, revision: u64, rows: &[(u64, u64, Vec<u8>)]) -> WitnessSnapshot {
    let entries = rows.iter().map(|(key, version, value)| {
        SnapshotEntry::new(*key, *version, value.clone()).unwrap()
    }).collect();
    WitnessSnapshot::new(revision, revision, 7, input, entries).unwrap()
}

fn requests() -> Vec<WitnessRequest> {
    vec![
        WitnessRequest::ExactValue { key: 0, role: QueryRole::PolicyInput },
        WitnessRequest::AbsentKey { key: 1 },
        WitnessRequest::RangeMembers { start: 2, end: 6 },
        WitnessRequest::EmptyRange { start: 6, end: 9 },
    ]
}

fn ingest<'a>(
    index: &mut WitnessChangeIndex<'a>,
    target: &'a WitnessSnapshot,
    frontiers: &ProductFrontiers,
    budget: RefinementBudget,
) -> ChangeCost {
    let mut pending = index.begin_observe(target, frontiers);
    for _ in 0..1_024 {
        let before = pending.total_work();
        let report = pending.advance(budget);
        assert!(report.spent.steps <= budget.steps);
        assert!(report.spent.value_bytes <= budget.value_bytes);
        assert_eq!(report.total.steps, before.steps + report.spent.steps);
        assert_eq!(report.total.value_bytes, before.value_bytes + report.spent.value_bytes);
        match report.outcome {
            ChangeObservationOutcome::Ready => return pending.commit().unwrap(),
            ChangeObservationOutcome::NeedsWork { .. } => assert!(report.spent.steps > 0),
            other => panic!("unexpected ingestion result: {other:?}"),
        }
    }
    panic!("small ingestion did not terminate");
}

fn finish(mut cursor: WitnessRefinement<'_>) -> RefinementOutcome {
    for _ in 0..1_024 {
        let report = cursor.advance(RefinementBudget { steps: 1, value_bytes: 1 });
        if !matches!(report.outcome, RefinementOutcome::NeedsRefinement { .. }) {
            return report.outcome;
        }
        assert!(report.spent.steps > 0);
    }
    panic!("small exact validation did not terminate");
}

fn matches_eager(
    captured: &IndexedJudgment,
    target: &WitnessSnapshot,
    frontiers: &ProductFrontiers,
    outcome: RefinementOutcome,
) {
    match (captured.judgment().reuse_at(target, frontiers), outcome) {
        (Ok(Reuse::StillValid { .. }), RefinementOutcome::StillValid) => {}
        (Ok(Reuse::Invalidated { reason: expected, .. }), RefinementOutcome::Invalidated { reason, .. }) => {
            assert_eq!(reason, expected);
        }
        (Err(expected), RefinementOutcome::Refused(error)) => assert_eq!(error, expected),
        pair => panic!("cooperative indexed/eager mismatch: {pair:?}"),
    }
}

#[test]
fn public_mutation_matrix_preserves_reuse_for_all_polling_and_planning_budgets() {
    let (frontiers, input) = fixture(1);
    let first = snapshot(input, 10, &rows());
    for key in [0, 1, 2, 4, 7, 9, u64::MAX] {
        for operation in 0..6 {
            let mut changed = rows();
            changed.retain(|(existing, _, _)| *existing != key);
            match operation {
                0 => {}
                1 => changed.push((key, 1, b"value".to_vec())),
                2 => changed.push((key, 2, b"value".to_vec())),
                3 => changed.push((key, 1, b"other".to_vec())),
                4 => changed.push((key, 1, b"valuE".to_vec())),
                5 => changed.push((key, 1, vec![])),
                _ => unreachable!(),
            }
            let next = snapshot(input, 11, &changed);
            for budget in [
                RefinementBudget { steps: 1, value_bytes: 1 },
                RefinementBudget { steps: 3, value_bytes: 2 },
                RefinementBudget { steps: u64::MAX, value_bytes: u64::MAX },
            ] {
                let mut index = WitnessChangeIndex::new(&first, &frontiers, 2).unwrap();
                let captured = index.capture(&frontiers, requests()).unwrap();
                ingest(&mut index, &next, &frontiers, budget);
                assert_eq!(index.current_revision(), 11);
                for planning in [0, 1, u64::MAX] {
                    let (report, cursor) = index.begin_refinement(&captured, &next, &frontiers, planning);
                    assert!(report.dependency_checks <= planning);
                    matches_eager(&captured, &next, &frontiers, finish(cursor));
                }
            }
        }
    }
}

#[test]
fn cancelled_insertion_cannot_become_a_skip_mask_or_a_false_absence() {
    let (frontiers, input) = fixture(1);
    let first = snapshot(input, 10, &rows());
    let mut changed = rows();
    changed.push((1, 1, b"inserted".to_vec()));
    let next = snapshot(input, 11, &changed);
    let mut index = WitnessChangeIndex::new(&first, &frontiers, 1).unwrap();
    let captured = index.capture(&frontiers, requests()).unwrap();
    let mut pending = index.begin_observe(&next, &frontiers);
    let report = pending.advance(RefinementBudget { steps: 3, value_bytes: 1 });
    assert!(matches!(report.outcome, ChangeObservationOutcome::NeedsWork { .. }));
    assert_eq!(report.total.value_bytes, 1);
    assert_eq!(pending.cancel().total, report.total);
    assert_eq!(index.current_revision(), 10);
    let (plan, cursor) = index.begin_refinement(&captured, &next, &frontiers, 100);
    assert_eq!(plan.selection, IndexSelection::ExactFallback(IndexFallback::TargetNotIndexed));
    assert_eq!(finish(cursor), RefinementOutcome::Invalidated {
        reason: Invalidation::AbsentKey, witness: Some(1),
    });
    // The cancellation does not poison still-valid historical evidence.
    let (plan, cursor) = index.begin_refinement(&captured, &first, &frontiers, 0);
    assert_eq!(plan.selection, IndexSelection::Candidates { count: 0 });
    assert_eq!(finish(cursor), RefinementOutcome::StillValid);
    ingest(&mut index, &next, &frontiers, RefinementBudget { steps: 1, value_bytes: 1 });
    let (plan, cursor) = index.begin_refinement(&captured, &next, &frontiers, 100);
    assert_eq!(plan.selection, IndexSelection::Candidates { count: 1 });
    assert_eq!(finish(cursor), RefinementOutcome::Invalidated {
        reason: Invalidation::AbsentKey, witness: Some(1),
    });
}

#[test]
fn completed_aba_and_later_eviction_still_require_exact_final_value_validation() {
    let (frontiers, input) = fixture(1);
    let first = snapshot(input, 10, &rows());
    let mut changed = rows();
    changed.push((1, 1, b"inserted".to_vec()));
    let inserted = snapshot(input, 11, &changed);
    let restored = snapshot(input, 12, &rows());
    let mut unrelated_rows = rows();
    unrelated_rows.push((9, 1, b"unrelated".to_vec()));
    let unrelated = snapshot(input, 13, &unrelated_rows);
    let mut index = WitnessChangeIndex::new(&first, &frontiers, 2).unwrap();
    let captured = index.capture(&frontiers, requests()).unwrap();
    let budget = RefinementBudget { steps: 2, value_bytes: 1 };
    ingest(&mut index, &inserted, &frontiers, budget);
    ingest(&mut index, &restored, &frontiers, budget);
    let (plan, cursor) = index.begin_refinement(&captured, &restored, &frontiers, 100);
    assert_eq!(plan.selection, IndexSelection::Candidates { count: 1 });
    assert_eq!(finish(cursor), RefinementOutcome::StillValid);
    // A ready update is not a committed eviction.
    let mut pending = index.begin_observe(&unrelated, &frontiers);
    assert_eq!(pending.advance(RefinementBudget {
        steps: u64::MAX, value_bytes: u64::MAX,
    }).outcome, ChangeObservationOutcome::Ready);
    assert_eq!(pending.cancel().outcome, ChangeObservationOutcome::Cancelled);
    assert_eq!(index.oldest_covered_revision(), 10);
    ingest(&mut index, &unrelated, &frontiers, budget);
    assert_eq!(index.oldest_covered_revision(), 11);
    let (plan, cursor) = index.begin_refinement(&captured, &unrelated, &frontiers, 100);
    assert_eq!(plan.selection, IndexSelection::ExactFallback(IndexFallback::HistoryEvicted));
    assert_eq!(finish(cursor), RefinementOutcome::StillValid);
}

#[test]
fn unchanged_keys_do_not_bypass_current_negative_frontier_evidence() {
    let (frontiers, input) = fixture(1);
    let (replacement_frontiers, replacement_input) = fixture(2);
    let first = snapshot(input, 10, &rows());
    let next = snapshot(replacement_input, 11, &rows());
    let mut index = WitnessChangeIndex::new(&first, &frontiers, 1).unwrap();
    let captured = index.capture(&frontiers, requests()).unwrap();
    assert_eq!(ingest(&mut index, &next, &replacement_frontiers, RefinementBudget {
        steps: 1, value_bytes: 1,
    }).changed_keys, 0);
    let (plan, cursor) = index.begin_refinement(&captured, &next, &replacement_frontiers, 100);
    assert_eq!(plan.selection, IndexSelection::Candidates { count: 0 });
    assert_eq!(finish(cursor), RefinementOutcome::Invalidated {
        reason: Invalidation::ClosingFrontier, witness: Some(1),
    });
    let missing = ProductFrontiers::new(1, 8).unwrap();
    let (_, cursor) = index.begin_refinement(&captured, &next, &missing, 100);
    assert_eq!(finish(cursor), RefinementOutcome::Refused(Error::Incomplete));
    let exact = index.capture(&replacement_frontiers, vec![WitnessRequest::ExactValue {
        key: 0, role: QueryRole::Subject,
    }]).unwrap();
    let (_, cursor) = index.begin_refinement(&exact, &next, &missing, 0);
    assert_eq!(finish(cursor), RefinementOutcome::StillValid);
}

#[test]
fn refused_unknown_target_is_never_treated_as_an_empty_successful_update() {
    let (frontiers, input) = fixture(1);
    let first = snapshot(input, 10, &rows());
    let unknown_input = AdapterDomainInput::new(input.domain(), DomainClosure::Unknown);
    let next = snapshot(unknown_input, 11, &rows());
    let mut index = WitnessChangeIndex::new(&first, &frontiers, 1).unwrap();
    let captured = index.capture(&frontiers, requests()).unwrap();
    let mut pending = index.begin_observe(&next, &frontiers);
    assert_eq!(pending.advance(RefinementBudget {
        steps: 1, value_bytes: 0,
    }).outcome, ChangeObservationOutcome::Refused(Error::Incomplete));
    assert_eq!(pending.commit(), Err(Error::Incomplete));
    assert_eq!(index.current_revision(), 10);
    let (plan, cursor) = index.begin_refinement(&captured, &next, &frontiers, 100);
    assert_eq!(plan.selection, IndexSelection::ExactFallback(IndexFallback::TargetNotIndexed));
    assert_eq!(finish(cursor), RefinementOutcome::Refused(Error::Incomplete));
}
