use super::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::replay::{
    ReplayStatus,
    comparison::verified::campaign::stepped::{CasePhase, MAX_STEP_OPERATIONS},
};

#[test]
fn segmented_and_whole_case_campaigns_have_identical_original_reports() {
    let model = model();
    let quiet = policy(&model, false, 1);
    let alarm = policy(&model, true, 1);
    let mut original = source(&model, quiet.clone());
    for position in 0..2 { original.advance(position).unwrap(); }
    let saved = original.checkpoint(CheckpointLimits::default()).unwrap();
    let cases = || vec![
        case(1, &saved, &alarm, ComparisonLimits::default()),
        case(2, &saved, &quiet, ComparisonLimits { positions: 0, ..ComparisonLimits::default() }),
        case(3, &saved, &quiet, ComparisonLimits { state_bytes: 0, ..ComparisonLimits::default() }),
        case(4, &saved, &quiet, ComparisonLimits::default()),
    ];
    let mut control = ComparisonCampaign::new(cases(), CampaignBudget::default()).unwrap();
    let summary = control.run_to_completion().unwrap();
    for batch in [1, 2, 7, MAX_STEP_OPERATIONS] {
        let mut stepped = ComparisonCampaign::new(cases(), CampaignBudget::default()).unwrap().into_stepped().unwrap();
        while stepped.progress().summary.status != CampaignStatus::Complete {
            let before = stepped.progress();
            assert_eq!(stepped.advance(before.revision, 0).unwrap(), before);
            let after = stepped.advance(before.revision, batch).unwrap();
            assert!(after.operations - before.operations <= batch as u64);
            assert_eq!(after.revision, before.revision + 1);
        }
        assert_eq!(stepped.progress().summary, summary);
        assert_eq!(stepped.reports(), control.reports());
        assert_eq!(stepped.run_to_completion().unwrap(), summary);
    }
    assert_eq!(original.generation().position(), 2);
}

#[test]
fn operations_are_bounded_and_paired_reports_wait_for_complete_verification() {
    let model = model();
    let quiet = policy(&model, false, 1);
    let mut original = source(&model, quiet.clone());
    for position in 0..3 { original.advance(position).unwrap(); }
    let saved = original.checkpoint(CheckpointLimits::default()).unwrap();
    let mut run = ComparisonCampaign::new(vec![case(1, &saved, &quiet, ComparisonLimits::default())],
        CampaignBudget::default()).unwrap().into_stepped().unwrap();
    let initial = run.progress();
    assert_eq!(run.advance(0, 0).unwrap(), initial);
    for compared in 1..=2 {
        let before = run.progress();
        let after = run.advance(before.revision, 1).unwrap();
        let active = after.active.unwrap();
        assert_eq!(active.phase, CasePhase::Verification);
        assert_eq!(active.preparation.compared_positions, compared);
        assert_eq!(active.preparation.reserved_and_accepted_work.admitted_tokens, compared as u64);
        assert!(!active.preparation.verification_complete);
        assert!(active.comparison.is_none());
        assert_eq!(run.advance(before.revision, 1), Err(Error::Stale));
        assert_eq!(run.advance(after.revision, MAX_STEP_OPERATIONS + 1), Err(Error::Limit));
        assert_eq!(run.progress(), after);
    }
    let verified = run.advance(run.revision(), 1).unwrap();
    let active = verified.active.unwrap();
    assert_eq!(active.phase, CasePhase::Comparison);
    assert!(active.preparation.verification_complete);
    assert_eq!(active.comparison.unwrap().attempted_positions, 0);
    let paired = run.advance(run.revision(), 1).unwrap();
    assert_eq!(paired.active.unwrap().comparison.unwrap().attempted_positions, 1);
    assert_eq!(paired.operations, 4);
    assert!(run.reports().is_empty());
    assert_eq!(run.run_to_completion().unwrap().matched_stops, 1);
}

#[test]
fn cancelling_partial_verification_retains_work_and_every_queued_case() {
    let model = model();
    let quiet = policy(&model, false, 1);
    let mut original = source(&model, quiet.clone());
    for position in 0..3 { original.advance(position).unwrap(); }
    let saved = original.checkpoint(CheckpointLimits::default()).unwrap();
    let mut run = ComparisonCampaign::new(vec![case(1, &saved, &quiet, ComparisonLimits::default()),
        case(2, &saved, &quiet, ComparisonLimits::default())], CampaignBudget::default()).unwrap().into_stepped().unwrap();
    let initial = run.progress();
    run.advance(0, 1).unwrap();
    let cancelled = run.cancel(1).unwrap();
    assert_eq!(cancelled.summary.status, CampaignStatus::Complete);
    assert_eq!((cancelled.summary.completed, cancelled.summary.pending, cancelled.summary.cancelled), (2, 0, 2));
    assert_eq!(cancelled.summary.reserved, initial.summary.reserved);
    assert_eq!(cancelled.operations, 1);
    assert!(cancelled.active.is_none());
    assert_eq!(run.reports()[0].preparation.compared_positions, 1);
    assert_eq!(run.reports()[0].preparation.reserved_and_accepted_work.admitted_tokens, 1);
    assert_eq!(run.reports()[1].preparation.reserved_and_accepted_work.admitted_tokens, 0);
    for row in run.reports() {
        assert_eq!(row.outcome, CaseOutcome::Cancelled);
        assert!(!row.preparation.verification_complete);
        assert!(row.comparison.is_none());
    }
    assert_eq!(run.cancel(run.revision()).unwrap(), cancelled);
    assert_eq!(run.cancel(1), Err(Error::Stale));
    assert_eq!(run.advance(run.revision(), 1), Err(Error::WrongState));
    assert_eq!(run.run_to_completion().unwrap(), cancelled.summary);
}

#[test]
fn cancelling_after_paired_work_keeps_original_partial_reports_without_certifying_a_stop() {
    let model = model();
    let quiet = policy(&model, false, 1);
    let mut original = source(&model, quiet.clone());
    original.advance(0).unwrap();
    let saved = original.checkpoint(CheckpointLimits::default()).unwrap();
    let mut run = ComparisonCampaign::new(vec![case(1, &saved, &quiet, ComparisonLimits::default()),
        case(2, &saved, &quiet, ComparisonLimits::default())], CampaignBudget::default()).unwrap().into_stepped().unwrap();
    let partial = run.advance(0, 3).unwrap(); // One baseline token and two paired positions.
    let before = partial.active.unwrap();
    assert_eq!(before.comparison.unwrap().attempted_positions, 2);
    let cancelled = run.cancel(run.revision()).unwrap();
    let row = run.reports()[0];
    assert_eq!(row.preparation, before.preparation);
    assert_eq!(row.comparison, before.comparison);
    assert_eq!(row.comparison.unwrap().status, ComparisonStatus::Active);
    assert_eq!(row.outcome, CaseOutcome::Cancelled);
    assert_eq!(cancelled.operations, 3);
    assert_eq!(cancelled.summary.matched_stops, 0);
    assert_eq!(cancelled.summary.cancelled, 2);
    assert!(run.reports()[1].comparison.is_none());
    assert_eq!(original.generation().position(), 1);
}

#[test]
fn cancellation_preserves_completed_rows_and_does_not_compute_unstarted_cases() {
    let model = model();
    let quiet = policy(&model, false, 1);
    let original = source(&model, quiet.clone());
    let saved = original.checkpoint(CheckpointLimits::default()).unwrap();
    let cases = || (1..=3).map(|id| case(id, &saved, &quiet, ComparisonLimits::default())).collect();
    let mut unstarted = ComparisonCampaign::new(cases(), CampaignBudget::default()).unwrap().into_stepped().unwrap();
    let cancelled = unstarted.cancel(0).unwrap();
    assert_eq!(cancelled.operations, 0);
    assert_eq!(cancelled.summary.cancelled, 3);
    assert!(unstarted.reports().iter().all(|row| !row.preparation.verification_complete));
    let mut partial = ComparisonCampaign::new(cases(), CampaignBudget::default()).unwrap().into_stepped().unwrap();
    partial.advance(0, 5).unwrap(); // Empty verification plus four original paired steps.
    assert_eq!(partial.reports().len(), 1);
    let first = partial.reports()[0];
    assert_eq!(first.outcome, CaseOutcome::MatchedStop(GenerationStop::TokenLimit));
    let after = partial.cancel(partial.revision()).unwrap();
    assert_eq!(partial.reports()[0], first);
    assert_eq!((after.summary.matched_stops, after.summary.cancelled, after.summary.completed), (1, 2, 3));
    assert_eq!(after.operations, 5);
    assert_eq!(partial.reports().iter().map(|row| row.id).collect::<Vec<_>>(), vec![1, 2, 3]);
    assert_eq!(partial.reports()[1].preparation.reserved_and_accepted_work.admitted_tokens, 0);
}

#[test]
fn corrupt_archive_verification_is_a_retained_failure_not_a_skipped_trial() {
    let model = model();
    let quiet = policy(&model, false, 1);
    let mut original = source(&model, quiet.clone());
    original.run_to_stop().unwrap();
    let saved = original.checkpoint(CheckpointLimits::default()).unwrap();
    let bytes = saved.encode_archive(ArchiveLimits::default()).unwrap();
    let mut corrupted = bytes.clone();
    // V1 ends with canonical cache expectation bytes. They are deliberately
    // parsed as comparisons, never installed or blessed by archive decoding.
    *corrupted.last_mut().unwrap() ^= 1;
    let intended = source(&model, quiet.clone());
    let bad = intended.decode_archive(&corrupted, ArchiveLimits::default()).unwrap();
    let good = intended.decode_archive(&bytes, ArchiveLimits::default()).unwrap();
    let inputs = vec![CampaignCase::new(1, bad.begin_policy_comparison(quiet.clone(),
        ReplayBudget::default(), ComparisonLimits::default()).unwrap()).unwrap(),
        CampaignCase::new(2, good.begin_policy_comparison(quiet,
            ReplayBudget::default(), ComparisonLimits::default()).unwrap()).unwrap()];
    let mut run = ComparisonCampaign::new(inputs, CampaignBudget::default()).unwrap().into_stepped().unwrap();
    let summary = run.run_to_completion().unwrap();
    assert_eq!((summary.completed, summary.verification_failures, summary.matched_stops), (2, 1, 1));
    let failure = run.reports()[0];
    assert_eq!(failure.outcome, CaseOutcome::VerificationFailed(Error::Binding));
    assert_eq!(failure.preparation.status, ReplayStatus::Failed(Error::Binding));
    assert_eq!(failure.preparation.compared_positions, 4);
    assert_eq!(failure.preparation.reserved_and_accepted_work.admitted_tokens, 4);
    assert!(!failure.preparation.verification_complete);
    assert!(failure.comparison.is_none());
    assert_eq!(intended.generation().work().admitted_tokens, 0);
}

#[test]
fn even_an_empty_archive_must_pass_the_original_state_comparison() {
    let model = model();
    let quiet = policy(&model, false, 1);
    let original = source(&model, quiet.clone());
    let saved = original.checkpoint(CheckpointLimits::default()).unwrap();
    let mut bytes = saved.encode_archive(ArchiveLimits::default()).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    let bad = original.decode_archive(&bytes, ArchiveLimits::default()).unwrap();
    let inputs = vec![CampaignCase::new(1, bad.begin_policy_comparison(quiet.clone(),
        ReplayBudget::default(), ComparisonLimits::default()).unwrap()).unwrap(),
        case(2, &saved, &quiet, ComparisonLimits::default())];
    let mut run = ComparisonCampaign::new(inputs, CampaignBudget::default()).unwrap().into_stepped().unwrap();
    let initial = run.progress();
    assert_eq!(run.advance(0, 0).unwrap(), initial);
    run.advance(0, 1).unwrap();
    assert_eq!(run.reports().len(), 1);
    assert_eq!(run.reports()[0].outcome, CaseOutcome::VerificationFailed(Error::Binding));
    assert_eq!(run.reports()[0].preparation.compared_positions, 0);
    assert!(run.reports()[0].comparison.is_none());
    let summary = run.run_to_completion().unwrap();
    assert_eq!((summary.verification_failures, summary.matched_stops), (1, 1));
}

#[test]
fn a_started_campaign_cannot_be_readopted_as_a_fresh_stepped_roster() {
    let model = model();
    let quiet = policy(&model, false, 1);
    let original = source(&model, quiet.clone());
    let saved = original.checkpoint(CheckpointLimits::default()).unwrap();
    let mut run = ComparisonCampaign::new(vec![case(1, &saved, &quiet, ComparisonLimits::default()),
        case(2, &saved, &quiet, ComparisonLimits::default())], CampaignBudget::default()).unwrap();
    run.run_next(1).unwrap();
    assert!(matches!(run.into_stepped(), Err(Error::WrongState)));
}

#[test]
fn an_exhausted_step_finishes_only_its_case_without_advancing_the_next_one() {
    let model = model();
    let quiet = policy(&model, false, 1);
    let original = source(&model, quiet.clone());
    let saved = original.checkpoint(CheckpointLimits::default()).unwrap();
    let mut run = ComparisonCampaign::new(vec![
        case(1, &saved, &quiet, ComparisonLimits { positions: 0, ..ComparisonLimits::default() }),
        case(2, &saved, &quiet, ComparisonLimits::default()),
    ], CampaignBudget::default()).unwrap().into_stepped().unwrap();
    let verified = run.advance(0, 1).unwrap();
    assert!(verified.active.unwrap().preparation.verification_complete);
    assert_eq!(verified.active.unwrap().comparison.unwrap().attempted_positions, 0);
    let exhausted = run.advance(run.revision(), 1).unwrap();
    assert_eq!(run.reports()[0].outcome, CaseOutcome::Exhausted);
    assert_eq!(exhausted.summary.status, CampaignStatus::Ready { next_case: 2 });
    assert!(exhausted.active.is_none());
    assert_eq!(exhausted.operations, 2);
    assert_eq!(run.advance(run.revision(), 0).unwrap(), exhausted);
    let summary = run.run_to_completion().unwrap();
    assert_eq!((summary.exhausted, summary.matched_stops), (1, 1));
}

#[test]
fn changing_streams_does_not_multiply_the_same_declared_evaluation_origin() {
    let model = model();
    let quiet = policy(&model, false, 1);
    let make_source = |stream, origin| {
        let spec = GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
            policy: SamplingPolicy::new(1, 1, 3, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173 }).unwrap();
        model.replayable_monitored_generation(stream, origin, spec, quiet.clone(),
            GenerationBudget::default(), GenerationTelemetryBudget::default()).unwrap()
    };
    let inputs = [(21, 201), (22, 201), (23, 202)].into_iter().enumerate().map(|(index, (stream, origin))| {
        let source = make_source(stream, origin);
        let saved = source.checkpoint(CheckpointLimits::default()).unwrap();
        case(index as u64 + 1, &saved, &quiet, ComparisonLimits::default())
    }).collect();
    let mut run = ComparisonCampaign::new(inputs, CampaignBudget::default()).unwrap().into_stepped().unwrap();
    let summary = run.run_to_completion().unwrap();
    assert_eq!((summary.matched_stops, summary.completed, summary.declared_lineages), (3, 3, 2));
    assert_eq!(run.reports().iter().map(|row| (row.lineage.stream, row.lineage.evaluation_origin))
        .collect::<Vec<_>>(), vec![(21, 201), (22, 201), (23, 202)]);
}
