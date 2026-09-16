use super::*;
use crate::product_frontier::{FrontierStage, ProjectionKey, TrustedClosingMarker};
use crate::witness::{
    AdapterDomainInput, DomainClosure, DomainProjection, MAX_VALUE_BYTES, QueryRole, Reuse,
    WitnessRequest,
};

fn fixture() -> (ProductFrontiers, AdapterDomainInput) {
    let key = ProjectionKey { source: 1, branch: 2, projection: 3, source_epoch: 4 };
    let mut frontiers = ProductFrontiers::new(1, 8).unwrap();
    frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap();
    let marker = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
    frontiers.record_close(marker).unwrap();
    (frontiers, AdapterDomainInput::new(DomainProjection::new(1, 1, key), DomainClosure::Closed(marker)))
}

fn snapshot(domain: AdapterDomainInput, keys: &[u64]) -> WitnessSnapshot {
    WitnessSnapshot::new(10, 20, 30, domain, keys.iter().map(|key| {
        SnapshotEntry::new(*key, 1, b"abc".to_vec()).unwrap()
    }).collect()).unwrap()
}

fn finish(session: &mut WitnessRefinement<'_>, budget: RefinementBudget) -> RefinementReport {
    for _ in 0..100_000 {
        let before = session.total_work();
        let report = session.advance(budget);
        assert!(report.spent.steps <= budget.steps);
        assert!(report.spent.value_bytes <= budget.value_bytes);
        assert_eq!(report.total.steps, before.steps + report.spent.steps);
        assert_eq!(report.total.value_bytes, before.value_bytes + report.spent.value_bytes);
        assert_eq!(report.total, session.total_work());
        if !matches!(report.outcome, RefinementOutcome::NeedsRefinement { .. }) {
            let repeated = session.advance(budget);
            assert_eq!(repeated.outcome, report.outcome);
            assert_eq!(repeated.total, report.total);
            assert_eq!(repeated.spent, RefinementBudget::default());
            return report;
        }
        assert_ne!(report.spent, RefinementBudget::default(), "positive budget must progress");
    }
    panic!("bounded fixture did not terminate");
}

fn assert_matches_eager(judgment: &WitnessJudgment, current: &WitnessSnapshot, frontiers: &ProductFrontiers) {
    for budget in [
        RefinementBudget { steps: 1, value_bytes: 1 },
        RefinementBudget { steps: 3, value_bytes: 2 },
        RefinementBudget { steps: u64::MAX, value_bytes: u64::MAX },
    ] {
        let result = finish(&mut judgment.begin_refinement(current, frontiers), budget).outcome;
        match (judgment.reuse_at(current, frontiers), result) {
            (Ok(Reuse::StillValid { .. }), RefinementOutcome::StillValid) => {}
            (Ok(Reuse::Invalidated { reason: eager, .. }), RefinementOutcome::Invalidated { reason, .. }) => assert_eq!(reason, eager),
            (Err(eager), RefinementOutcome::Refused(error)) => assert_eq!(error, eager),
            pair => panic!("eager/refined mismatch: {pair:?}"),
        }
    }
}

#[test]
fn all_witness_kinds_match_eager_under_insert_delete_version_and_value_mutations() {
    let (frontiers, domain) = fixture();
    let original = snapshot(domain, &[0, 2, 4]);
    let judgment = WitnessJudgment::capture(&original, &frontiers, vec![
        WitnessRequest::ExactValue { key: 0, role: QueryRole::Subject },
        WitnessRequest::AbsentKey { key: 1 },
        WitnessRequest::EmptyRange { start: 6, end: 9 },
        WitnessRequest::RangeMembers { start: 2, end: 6 },
    ]).unwrap();
    assert_matches_eager(&judgment, &original, &frontiers);
    for key in 0..10 {
        for operation in 0..5 {
            let mut current = original.clone();
            current.revision += 1;
            current.control_cut += 1;
            match operation {
                0 => { current.values.remove(&key); }
                1 => { current.values.insert(key, SnapshotEntry::new(key, 1, b"abc".to_vec()).unwrap()); }
                2 => { current.values.insert(key, SnapshotEntry::new(key, 2, b"abc".to_vec()).unwrap()); }
                3 => { current.values.insert(key, SnapshotEntry::new(key, 1, b"abx".to_vec()).unwrap()); }
                4 => { current.values.insert(key, SnapshotEntry::new(key, 1, vec![]).unwrap()); }
                _ => unreachable!(),
            }
            assert_matches_eager(&judgment, &current, &frontiers);
        }
    }
}

#[test]
fn zero_budget_is_not_validity_and_byte_exhaustion_does_not_restart_lookup() {
    let (frontiers, domain) = fixture();
    let original = snapshot(domain, &[0]);
    let judgment = WitnessJudgment::capture(&original, &frontiers, vec![
        WitnessRequest::ExactValue { key: 0, role: QueryRole::Subject },
    ]).unwrap();
    let mut session = judgment.begin_refinement(&original, &frontiers);
    let zero = session.advance(RefinementBudget::default());
    assert_eq!(zero.outcome, RefinementOutcome::NeedsRefinement {
        minimum: RefinementBudget { steps: 1, value_bytes: 0 },
    });
    assert_eq!(zero.total, RefinementBudget::default());
    let paused = session.advance(RefinementBudget { steps: 100, value_bytes: 0 });
    assert_eq!(paused.spent, RefinementBudget { steps: 3, value_bytes: 0 });
    assert_eq!(paused.outcome, RefinementOutcome::NeedsRefinement {
        minimum: RefinementBudget { steps: 1, value_bytes: 1 },
    });
    let still_paused = session.advance(RefinementBudget { steps: 100, value_bytes: 0 });
    assert_eq!(still_paused.total, paused.total);
    assert_eq!(still_paused.spent, RefinementBudget::default());
    let finished = finish(&mut session, RefinementBudget { steps: 1, value_bytes: 1 });
    assert_eq!(finished.outcome, RefinementOutcome::StillValid);
    assert_eq!(finished.total, RefinementBudget { steps: 6, value_bytes: 3 });
}

#[test]
fn last_byte_mutation_in_maximum_value_stays_pending_until_examined() {
    let (frontiers, domain) = fixture();
    let original = WitnessSnapshot::new(10, 20, 30, domain, vec![
        SnapshotEntry::new(0, 1, vec![7; MAX_VALUE_BYTES]).unwrap(),
    ]).unwrap();
    let judgment = WitnessJudgment::capture(&original, &frontiers, vec![
        WitnessRequest::ExactValue { key: 0, role: QueryRole::Subject },
    ]).unwrap();
    let mut current = original.clone();
    *current.values.get_mut(&0).unwrap().value.last_mut().unwrap() = 8;
    let mut session = judgment.begin_refinement(&current, &frontiers);
    let first = session.advance(RefinementBudget { steps: 100, value_bytes: (MAX_VALUE_BYTES - 1) as u64 });
    assert!(matches!(first.outcome, RefinementOutcome::NeedsRefinement { .. }));
    assert_eq!(first.total.value_bytes, (MAX_VALUE_BYTES - 1) as u64);
    let last = session.advance(RefinementBudget { steps: 1, value_bytes: 1 });
    assert_eq!(last.outcome, RefinementOutcome::Invalidated {
        reason: Invalidation::ExactValue, witness: Some(0),
    });
    assert_eq!(last.total.value_bytes, MAX_VALUE_BYTES as u64);
}

#[test]
fn range_cursor_preserves_progress_and_checks_the_unseen_suffix() {
    let (frontiers, domain) = fixture();
    let original = snapshot(domain, &[2, 4]);
    let judgment = WitnessJudgment::capture(&original, &frontiers, vec![
        WitnessRequest::RangeMembers { start: 2, end: 6 },
    ]).unwrap();
    let finished = finish(&mut judgment.begin_refinement(&original, &frontiers),
        RefinementBudget { steps: 1, value_bytes: 1 });
    // Basis, witness, frontier, three range advances, six one-byte comparisons.
    assert_eq!(finished.total, RefinementBudget { steps: 12, value_bytes: 6 });
    let mut phantom = original.clone();
    phantom.values.insert(5, SnapshotEntry::new(5, 1, vec![]).unwrap());
    let mut session = judgment.begin_refinement(&phantom, &frontiers);
    let prefix = session.advance(RefinementBudget { steps: 7, value_bytes: 6 });
    assert!(matches!(prefix.outcome, RefinementOutcome::NeedsRefinement { .. }));
    assert_eq!(prefix.total.value_bytes, 6);
    let suffix = session.advance(RefinementBudget { steps: 1, value_bytes: 0 });
    assert_eq!(suffix.outcome, RefinementOutcome::Invalidated {
        reason: Invalidation::RangeMembers, witness: Some(0),
    });
}

#[test]
fn unknown_and_summary_closures_refuse_with_retained_partial_work() {
    let (frontiers, domain) = fixture();
    let original = snapshot(domain, &[0]);
    let judgment = WitnessJudgment::capture(&original, &frontiers, vec![
        WitnessRequest::ExactValue { key: 0, role: QueryRole::PolicyInput },
        WitnessRequest::AbsentKey { key: 7 },
    ]).unwrap();
    for closure in [DomainClosure::Unknown, DomainClosure::ConservativeSummary] {
        let mut current = original.clone();
        current.domain_input = AdapterDomainInput::new(domain.domain(), closure);
        let report = finish(&mut judgment.begin_refinement(&current, &frontiers),
            RefinementBudget { steps: 100, value_bytes: 100 });
        assert_eq!(report.outcome, RefinementOutcome::Refused(Error::Incomplete));
        assert_eq!(report.total, RefinementBudget { steps: 6, value_bytes: 3 });
        assert_matches_eager(&judgment, &current, &frontiers);
    }
}

#[test]
fn exact_values_do_not_require_negative_closure_even_at_maximum_key() {
    let (frontiers, domain) = fixture();
    let unknown = AdapterDomainInput::new(domain.domain(), DomainClosure::Unknown);
    let original = snapshot(unknown, &[u64::MAX]);
    let judgment = WitnessJudgment::capture(&original, &frontiers, vec![
        WitnessRequest::ExactValue { key: u64::MAX, role: QueryRole::Subject },
    ]).unwrap();
    assert_matches_eager(&judgment, &original, &frontiers);
}

#[test]
fn basis_mismatches_and_stale_cuts_match_the_original_priority() {
    let (frontiers, domain) = fixture();
    let original = snapshot(domain, &[0]);
    let judgment = WitnessJudgment::capture(&original, &frontiers, vec![]).unwrap();
    for case in 0..6 {
        let mut current = original.clone();
        match case {
            0 => { current.revision -= 1; current.semantic_epoch += 1; }
            1 => current.control_cut -= 1,
            2 => current.semantic_epoch += 1,
            3 => current.domain_input.domain.domain_id += 1,
            4 => current.domain_input.domain.domain_epoch += 1,
            5 => current.domain_input.domain.projection.branch += 1,
            _ => unreachable!(),
        }
        assert_matches_eager(&judgment, &current, &frontiers);
        let result = finish(&mut judgment.begin_refinement(&current, &frontiers),
            RefinementBudget { steps: 1, value_bytes: 0 });
        assert_eq!(result.total.steps, 1);
        if let RefinementOutcome::Invalidated { witness, .. } = result.outcome {
            assert_eq!(witness, None);
        }
    }
}

#[test]
fn empty_ranges_and_empty_values_need_steps_but_no_value_byte_budget() {
    let (frontiers, domain) = fixture();
    let original = WitnessSnapshot::new(10, 20, 30, domain, vec![
        SnapshotEntry::new(0, 1, vec![]).unwrap(),
    ]).unwrap();
    let judgment = WitnessJudgment::capture(&original, &frontiers, vec![
        WitnessRequest::ExactValue { key: 0, role: QueryRole::Subject },
        WitnessRequest::RangeMembers { start: 0, end: 1 },
        WitnessRequest::RangeMembers { start: 1, end: 2 },
        WitnessRequest::EmptyRange { start: 2, end: u64::MAX },
    ]).unwrap();
    let result = finish(&mut judgment.begin_refinement(&original, &frontiers),
        RefinementBudget { steps: 1, value_bytes: 0 });
    assert_eq!(result.outcome, RefinementOutcome::StillValid);
    assert_eq!(result.total.value_bytes, 0);
    assert_matches_eager(&judgment, &original, &frontiers);
}

#[test]
fn arithmetic_overflow_is_terminal_and_never_advances_unpaid_work() {
    let (frontiers, domain) = fixture();
    let original = snapshot(domain, &[0]);
    let judgment = WitnessJudgment::capture(&original, &frontiers, vec![
        WitnessRequest::ExactValue { key: 0, role: QueryRole::Subject },
    ]).unwrap();
    let mut session = judgment.begin_refinement(&original, &frontiers);
    session.total.steps = u64::MAX;
    let result = session.advance(RefinementBudget { steps: 1, value_bytes: 1 });
    assert_eq!(result.outcome, RefinementOutcome::Refused(Error::Overflow));
    assert_eq!(result.spent, RefinementBudget::default());
    assert!(matches!(session.stage, Stage::Basis));
    let mut bytes = judgment.begin_refinement(&original, &frontiers);
    bytes.advance(RefinementBudget { steps: 3, value_bytes: 0 });
    bytes.total.value_bytes = u64::MAX;
    let result = bytes.advance(RefinementBudget { steps: 1, value_bytes: 1 });
    assert_eq!(result.outcome, RefinementOutcome::Refused(Error::Overflow));
    assert_eq!(result.spent, RefinementBudget::default());
    assert!(matches!(bytes.stage, Stage::Compare { offset: 0, .. }));
}
