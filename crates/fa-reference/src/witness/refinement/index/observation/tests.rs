use super::*;
use crate::product_frontier::{FrontierStage, ProjectionKey, TrustedClosingMarker};
use crate::witness::{
    AdapterDomainInput, DomainClosure, DomainProjection, MAX_SNAPSHOT_ENTRIES, MAX_VALUE_BYTES,
};

fn fixture() -> (ProductFrontiers, AdapterDomainInput) {
    let key = ProjectionKey { source: 1, branch: 2, projection: 3, source_epoch: 4 };
    let mut frontiers = ProductFrontiers::new(1, 8).unwrap();
    frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap();
    let marker = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
    frontiers.record_close(marker).unwrap();
    let input = AdapterDomainInput::new(DomainProjection::new(1, 1, key), DomainClosure::Closed(marker));
    (frontiers, input)
}

fn snapshot(input: AdapterDomainInput, revision: u64, keys: &[u64], bytes: usize) -> WitnessSnapshot {
    WitnessSnapshot::new(revision, revision, 7, input, keys.iter().map(|key| {
        SnapshotEntry::new(*key, 1, vec![17; bytes]).unwrap()
    }).collect()).unwrap()
}

fn ample() -> RefinementBudget {
    RefinementBudget { steps: u64::MAX, value_bytes: u64::MAX }
}

fn ready(pending: &mut ChangeObservation<'_, '_>, budget: RefinementBudget) -> ChangeObservationReport {
    for _ in 0..100_000 {
        let before = pending.total_work();
        let report = pending.advance(budget);
        assert!(report.spent.steps <= budget.steps);
        assert!(report.spent.value_bytes <= budget.value_bytes);
        assert_eq!(report.total.steps, before.steps + report.spent.steps);
        assert_eq!(report.total.value_bytes, before.value_bytes + report.spent.value_bytes);
        assert_eq!(report.cost.value_bytes, report.total.value_bytes);
        match report.outcome {
            ChangeObservationOutcome::Ready => return report,
            ChangeObservationOutcome::NeedsWork { .. } => assert!(report.spent.steps > 0),
            other => panic!("unexpected observation outcome: {other:?}"),
        }
    }
    panic!("bounded observation did not terminate");
}

#[test]
fn zero_work_and_premature_commit_never_publish_or_evict() {
    let (frontiers, input) = fixture();
    let first = snapshot(input, 10, &[0], 16);
    let next = snapshot(input, 11, &[0, 1], 16);
    let mut index = WitnessChangeIndex::new(&first, &frontiers, 1).unwrap();
    let mut pending = index.begin_observe(&next, &frontiers);
    let zero = pending.advance(RefinementBudget::default());
    assert_eq!(zero.outcome, ChangeObservationOutcome::NeedsWork {
        minimum: RefinementBudget { steps: 1, value_bytes: 0 },
    });
    assert_eq!(zero.spent, RefinementBudget::default());
    assert_eq!(zero.total, RefinementBudget::default());
    assert_eq!(zero.cost, ChangeCost::default());
    assert_eq!(pending.commit(), Err(Error::Incomplete));
    assert_eq!(index.current_revision(), 10);
    assert_eq!(index.oldest_covered_revision(), 10);
    assert_eq!(index.retained_revisions(), 0);
    assert_eq!(index.observe(&next, &frontiers).unwrap().changed_keys, 1);
}

#[test]
fn byte_exhaustion_keeps_the_exact_offset_and_never_recompares_a_prefix() {
    let (frontiers, input) = fixture();
    let first = snapshot(input, 10, &[0], MAX_VALUE_BYTES);
    let next = snapshot(input, 11, &[0], MAX_VALUE_BYTES);
    let mut index = WitnessChangeIndex::new(&first, &frontiers, 1).unwrap();
    let mut pending = index.begin_observe(&next, &frontiers);
    let paused = pending.advance(RefinementBudget { steps: 100, value_bytes: 0 });
    assert_eq!(paused.spent, RefinementBudget { steps: 2, value_bytes: 0 });
    assert_eq!(paused.cost.point_lookups, 1);
    assert_eq!(paused.outcome, ChangeObservationOutcome::NeedsWork {
        minimum: RefinementBudget { steps: 1, value_bytes: 1 },
    });
    let unchanged = pending.advance(RefinementBudget { steps: 100, value_bytes: 0 });
    assert_eq!(unchanged.total, paused.total);
    assert_eq!(unchanged.spent, RefinementBudget::default());
    let done = ready(&mut pending, RefinementBudget { steps: 1, value_bytes: 1 });
    assert_eq!(done.cost.value_bytes, MAX_VALUE_BYTES as u64);
    assert_eq!(done.cost.point_lookups, 2);
    assert_eq!(done.cost.changed_keys, 0);
    assert_eq!(pending.index.current_revision(), 10);
    let repeated = pending.advance(RefinementBudget::default());
    assert_eq!(repeated.outcome, ChangeObservationOutcome::Ready);
    assert_eq!(repeated.spent, RefinementBudget::default());
    assert_eq!(repeated.total, done.total);
    assert_eq!(pending.commit(), Ok(done.cost));
    assert_eq!(index.current_revision(), 11);
}

#[test]
fn dropping_or_cancelling_at_every_step_keeps_complete_history() {
    let (frontiers, input) = fixture();
    let first = snapshot(input, 10, &[0], 4);
    let middle = snapshot(input, 11, &[0, 1], 4);
    let next = snapshot(input, 12, &[0, 1, 2], 4);
    for advances in 0..24 {
        let mut index = WitnessChangeIndex::new(&first, &frontiers, 1).unwrap();
        index.observe(&middle, &frontiers).unwrap();
        let mut pending = index.begin_observe(&next, &frontiers);
        for _ in 0..advances {
            let _ = pending.advance(RefinementBudget { steps: 1, value_bytes: 1 });
        }
        let work = pending.total_work();
        assert_eq!(pending.index.current_revision(), 11);
        assert_eq!(pending.index.oldest_covered_revision(), 10);
        if advances % 2 == 0 {
            let cancelled = pending.cancel();
            assert_eq!(cancelled.outcome, ChangeObservationOutcome::Cancelled);
            assert_eq!(cancelled.spent, RefinementBudget::default());
            assert_eq!(cancelled.total, work);
        } else {
            drop(pending);
        }
        assert_eq!(index.current_revision(), 11);
        assert_eq!(index.oldest_covered_revision(), 10);
        assert_eq!(index.retained_revisions(), 1);
        assert_eq!(index.deltas.front().unwrap().keys, BTreeSet::from([1]));
        assert_eq!(index.observe(&next, &frontiers).unwrap().changed_keys, 1);
        assert_eq!(index.current_revision(), 12);
        assert_eq!(index.oldest_covered_revision(), 11);
        assert_eq!(index.deltas.front().unwrap().keys, BTreeSet::from([2]));
    }
}

#[test]
fn completed_but_uncommitted_observation_can_still_be_cancelled() {
    let (frontiers, input) = fixture();
    let first = snapshot(input, 10, &[0], 4);
    let next = snapshot(input, 11, &[0, 1], 4);
    let mut index = WitnessChangeIndex::new(&first, &frontiers, 1).unwrap();
    let mut pending = index.begin_observe(&next, &frontiers);
    let done = ready(&mut pending, ample());
    let cancelled = pending.cancel();
    assert_eq!(cancelled.outcome, ChangeObservationOutcome::Cancelled);
    assert_eq!(cancelled.cost, done.cost);
    assert_eq!(cancelled.total, done.total);
    assert_eq!(index.current_revision(), 10);
    assert_eq!(index.retained_revisions(), 0);
}

#[test]
fn admission_refusals_are_sticky_and_do_no_payload_work() {
    let (frontiers, input) = fixture();
    let first = snapshot(input, 10, &[0], 16);
    for case in 0..8 {
        let mut next = snapshot(input, 11, &[0, 1], 16);
        let expected = match case {
            0 => { next.revision = 12; Error::Stale }
            1 => { next.control_cut = 9; Error::Stale }
            2 => { next.semantic_epoch += 1; Error::Binding }
            3 => { next.domain_input.domain.domain_id += 1; Error::Binding }
            4 => { next.domain_input.domain.domain_epoch += 1; Error::Binding }
            5 => { next.domain_input.domain.projection.branch += 1; Error::Binding }
            6 => { next.domain_input.closure = DomainClosure::Unknown; Error::Incomplete }
            7 => { next.domain_input.closure = DomainClosure::ConservativeSummary; Error::Incomplete }
            _ => unreachable!(),
        };
        let mut index = WitnessChangeIndex::new(&first, &frontiers, 1).unwrap();
        let mut pending = index.begin_observe(&next, &frontiers);
        let report = pending.advance(ample());
        assert_eq!(report.outcome, ChangeObservationOutcome::Refused(expected));
        assert_eq!(report.spent, RefinementBudget { steps: 1, value_bytes: 0 });
        assert_eq!(report.cost, ChangeCost::default());
        let repeated = pending.advance(ample());
        assert_eq!(repeated.outcome, report.outcome);
        assert_eq!(repeated.total, report.total);
        assert_eq!(repeated.spent, RefinementBudget::default());
        assert_eq!(pending.commit(), Err(expected));
        assert_eq!(index.current_revision(), 10);
        assert_eq!(index.retained_revisions(), 0);
    }
    let missing_frontiers = ProductFrontiers::new(1, 8).unwrap();
    let next = snapshot(input, 11, &[0], 16);
    let mut index = WitnessChangeIndex::new(&first, &frontiers, 1).unwrap();
    let mut pending = index.begin_observe(&next, &missing_frontiers);
    assert_eq!(pending.advance(ample()).outcome, ChangeObservationOutcome::Refused(Error::Incomplete));
    assert_eq!(pending.cancel().outcome, ChangeObservationOutcome::Refused(Error::Incomplete));
    assert_eq!(index.current_revision(), 10);
}

#[test]
fn revision_overflow_is_an_admission_failure_not_a_wrapped_delta() {
    let (frontiers, input) = fixture();
    let first = snapshot(input, u64::MAX, &[], 0);
    let next = snapshot(input, 0, &[], 0);
    let mut index = WitnessChangeIndex::new(&first, &frontiers, 1).unwrap();
    let mut pending = index.begin_observe(&next, &frontiers);
    let report = pending.advance(ample());
    assert_eq!(report.outcome, ChangeObservationOutcome::Refused(Error::Overflow));
    assert_eq!(report.total.steps, 1);
    assert_eq!(report.cost, ChangeCost::default());
    assert_eq!(pending.commit(), Err(Error::Overflow));
    assert_eq!(index.current_revision(), u64::MAX);
}

#[test]
fn differential_mutations_match_the_independent_original_whole_value_algorithm() {
    let (frontiers, input) = fixture();
    let first = snapshot(input, 10, &[0, 2, 4], 5);
    for key in [0, 1, 2, 3, 4, 5, u64::MAX] {
        for operation in 0..7 {
            let mut next = snapshot(input, 11, &[0, 2, 4], 5);
            match operation {
                0 => { next.values.remove(&key); }
                1 => { next.values.insert(key, SnapshotEntry::new(key, 1, vec![17; 5]).unwrap()); }
                2 => { next.values.insert(key, SnapshotEntry::new(key, 2, vec![17; 5]).unwrap()); }
                3 => { next.values.insert(key, SnapshotEntry::new(key, 1, vec![99, 17, 17, 17, 17]).unwrap()); }
                4 => { next.values.insert(key, SnapshotEntry::new(key, 1, vec![17, 17, 17, 17, 99]).unwrap()); }
                5 => { next.values.insert(key, SnapshotEntry::new(key, 1, vec![17; 6]).unwrap()); }
                6 => { next.values.insert(key, SnapshotEntry::new(key, 1, vec![]).unwrap()); }
                _ => unreachable!(),
            }
            let (expected_keys, expected_cost) = super::super::changed_keys(&first, &next);
            for budget in [RefinementBudget { steps: 1, value_bytes: 1 }, ample()] {
                let mut index = WitnessChangeIndex::new(&first, &frontiers, 1).unwrap();
                let mut pending = index.begin_observe(&next, &frontiers);
                let report = ready(&mut pending, budget);
                assert_eq!(pending.keys, expected_keys);
                assert_eq!(report.cost.point_lookups, expected_cost.point_lookups);
                assert_eq!(report.cost.changed_keys, expected_cost.changed_keys);
                assert!(report.cost.value_bytes <= expected_cost.value_bytes);
                if budget == ample() { assert_eq!(report.cost, expected_cost); }
                assert_eq!(pending.commit(), Ok(report.cost));
                assert_eq!(index.deltas.front().unwrap().keys, expected_keys);
            }
        }
    }
}

#[test]
fn maximum_disjoint_snapshots_need_no_value_budget_and_retain_only_keys() {
    let (frontiers, input) = fixture();
    let old_keys: Vec<u64> = (0..MAX_SNAPSHOT_ENTRIES as u64).collect();
    let new_keys: Vec<u64> = (MAX_SNAPSHOT_ENTRIES as u64..2 * MAX_SNAPSHOT_ENTRIES as u64).collect();
    let first = snapshot(input, 10, &old_keys, MAX_VALUE_BYTES);
    let next = snapshot(input, 11, &new_keys, MAX_VALUE_BYTES);
    let mut index = WitnessChangeIndex::new(&first, &frontiers, 1).unwrap();
    let mut pending = index.begin_observe(&next, &frontiers);
    let report = ready(&mut pending, RefinementBudget { steps: 1, value_bytes: 0 });
    assert_eq!(report.cost, ChangeCost {
        point_lookups: 2 * MAX_SNAPSHOT_ENTRIES as u64,
        value_bytes: 0,
        changed_keys: 2 * MAX_SNAPSHOT_ENTRIES,
    });
    assert_eq!(pending.keys.len(), 2 * MAX_SNAPSHOT_ENTRIES);
    assert_eq!(report.total.steps, 2 * MAX_SNAPSHOT_ENTRIES as u64 + 3);
    assert_eq!(pending.commit(), Ok(report.cost));
}

#[test]
fn maximum_equal_payloads_are_charged_once_across_bounded_polls() {
    let (frontiers, input) = fixture();
    let keys: Vec<u64> = (0..MAX_SNAPSHOT_ENTRIES as u64).collect();
    let first = snapshot(input, 10, &keys, MAX_VALUE_BYTES);
    let next = snapshot(input, 11, &keys, MAX_VALUE_BYTES);
    let mut index = WitnessChangeIndex::new(&first, &frontiers, 1).unwrap();
    let mut pending = index.begin_observe(&next, &frontiers);
    let report = ready(&mut pending, RefinementBudget { steps: 3, value_bytes: 64 });
    assert_eq!(report.cost.value_bytes, (MAX_SNAPSHOT_ENTRIES * MAX_VALUE_BYTES) as u64);
    assert_eq!(report.cost.point_lookups, 2 * MAX_SNAPSHOT_ENTRIES as u64);
    assert_eq!(report.cost.changed_keys, 0);
    assert!(pending.keys.is_empty());
    assert_eq!(pending.commit(), Ok(report.cost));
}
