use super::*;
use crate::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use crate::full_input::{ByteSpan, InputProfileBinding, Omission, PartKind, SubmittedPart,
    MAX_OMISSIONS, MAX_PROFILE_BYTES, MAX_SUBMITTED_BYTES, MAX_SUBMITTED_PARTS};
use crate::product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker};
use crate::witness::{AdapterDomainInput, DomainClosure, DomainProjection, Invalidation,
    QueryRole, SnapshotEntry, WitnessJudgment, WitnessRequest, WitnessSnapshot};

fn ample() -> RefinementBudget { RefinementBudget { steps: u64::MAX, value_bytes: u64::MAX } }
fn action() -> FrozenAction {
    FrozenAction::freeze(ActionSpec {
        version: VERSION,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(ResolvedTarget { adapter: 1, object: 2, contract_version: 1, expected_version: 1, generation: 1 }),
        payload: b"publish".to_vec(), required_witnesses: vec![], policy_epoch: 0,
        deadline: ElapsedTick(100), units: 128,
    }).unwrap()
}
fn input() -> ActualHelperInput {
    ActualHelperInput::new(b"Q evidence".to_vec(), InputProfileBinding {
        profile_id: 1, profile_bytes: b"v1".to_vec(), tokenizer_epoch: 2, policy_epoch: 3, model_epoch: 4,
    }, vec![
        SubmittedPart { span: ByteSpan { start: 0, end: 1 }, kind: PartKind::Question },
        SubmittedPart { span: ByteSpan { start: 1, end: 5 }, kind: PartKind::Evidence { source_id: 1, transform_id: 1 } },
        SubmittedPart { span: ByteSpan { start: 5, end: 10 }, kind: PartKind::ToolSchema { schema_id: 1 } },
    ], vec![
        Omission::ClosedAbsent { domain_id: 71, trusted_closure_marker_id: 1 },
        Omission::Gapped { domain_id: 72, first_missing: 9 },
        Omission::Unsupported { domain_id: 73 },
        Omission::Redacted { domain_id: 74, transform_id: 1 },
    ]).unwrap()
}
fn fixture(revision: u64, keys: &[u64]) -> (WitnessSnapshot, ProductFrontiers) {
    let key = ProjectionKey { source: 1, branch: 2, projection: 3, source_epoch: 4 };
    let mut frontiers = ProductFrontiers::new(1, 8).unwrap();
    frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap();
    let marker = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
    frontiers.record_close(marker).unwrap();
    let domain = AdapterDomainInput::new(DomainProjection::new(1, 1, key), DomainClosure::Closed(marker));
    (WitnessSnapshot::new(revision, revision, 7, domain, keys.iter().map(|key| {
        SnapshotEntry::new(*key, 1, b"abc".to_vec()).unwrap()
    }).collect()).unwrap(), frontiers)
}
fn capture(snapshot: &WitnessSnapshot, frontiers: &ProductFrontiers) -> WitnessJudgment {
    WitnessJudgment::capture(snapshot, frontiers, vec![
        WitnessRequest::ExactValue { key: 0, role: QueryRole::Subject },
        WitnessRequest::AbsentKey { key: 1 },
        WitnessRequest::EmptyRange { start: 6, end: 9 },
        WitnessRequest::RangeMembers { start: 2, end: 6 },
    ]).unwrap()
}
fn finish(cursor: &mut PublicationRefinement<'_>, poll: RefinementBudget) -> PublicationProgress {
    for _ in 0..100_000 {
        let before = cursor.total_work();
        let report = cursor.advance(poll);
        assert!(report.spent.steps <= poll.steps);
        assert!(report.spent.value_bytes <= poll.value_bytes);
        assert_eq!(report.total.steps, before.steps + report.spent.steps);
        assert_eq!(report.total.value_bytes, before.value_bytes + report.spent.value_bytes);
        assert!(report.total.steps <= cursor.ceiling().steps);
        assert!(report.total.value_bytes <= cursor.ceiling().value_bytes);
        if !matches!(report.outcome, PublicationOutcome::NeedsRefinement { .. }) { return report; }
        assert!(report.spent.steps > 0, "test supplied enough work for progress");
    }
    panic!("bounded comparison did not terminate");
}
fn mutations(original: &ActualHelperInput) -> Vec<ActualHelperInput> {
    let mut result = Vec::new();
    for index in 0..original.submitted_bytes().len() {
        let mut bytes = original.submitted_bytes().to_vec(); bytes[index] ^= 1;
        result.push(ActualHelperInput::new(bytes, original.input_profile().clone(),
            original.ordered_parts().to_vec(), original.omissions().to_vec()).unwrap());
    }
    for index in 0..original.input_profile().profile_bytes.len() {
        let mut profile = original.input_profile().clone(); profile.profile_bytes[index] ^= 1;
        result.push(ActualHelperInput::new(original.submitted_bytes().to_vec(), profile,
            original.ordered_parts().to_vec(), original.omissions().to_vec()).unwrap());
    }
    for mutation in 0..18 {
        let mut bytes = original.submitted_bytes().to_vec();
        let mut profile = original.input_profile().clone();
        let mut parts = original.ordered_parts().to_vec();
        let mut omissions = original.omissions().to_vec();
        match mutation {
            0 => profile.profile_id += 1,
            1 => profile.tokenizer_epoch += 1,
            2 => profile.policy_epoch += 1,
            3 => profile.model_epoch += 1,
            4 => profile.profile_bytes.push(0),
            5 => { bytes.push(0); parts[2].span.end += 1; }
            6 => parts[1].kind = PartKind::Evidence { source_id: 2, transform_id: 1 },
            7 => parts[1].kind = PartKind::Evidence { source_id: 1, transform_id: 2 },
            8 => parts[2].kind = PartKind::ToolSchema { schema_id: 2 },
            9 => { parts[1].span.end = 4; parts[2].span.start = 4; }
            10 => parts[2].kind = PartKind::Instruction,
            11 => omissions[0] = Omission::ClosedAbsent { domain_id: 71, trusted_closure_marker_id: 2 },
            12 => omissions[1] = Omission::Gapped { domain_id: 72, first_missing: 10 },
            13 => omissions[2] = Omission::Unsupported { domain_id: 75 },
            14 => omissions[3] = Omission::Redacted { domain_id: 74, transform_id: 2 },
            15 => omissions[0] = Omission::Unsupported { domain_id: 71 },
            16 => omissions.swap(0, 1),
            17 => { omissions.pop(); }
            _ => unreachable!(),
        }
        result.push(ActualHelperInput::new(bytes, profile, parts, omissions).unwrap());
    }
    result
}

#[test]
fn every_opaque_byte_epoch_part_and_omission_matches_the_independent_eager_oracle() {
    let original = input();
    let opaque = OpaqueJudgment::capture(&original, b"only the question matters");
    let judgment = PublicationJudgment::bind(action(), None, Some(opaque.clone())).unwrap();
    let changed = mutations(&original);
    assert_eq!(changed.len(), 30);
    for actual in std::iter::once(&original).chain(changed.iter()) {
        let equal = opaque.valid_at(actual);
        assert_eq!(opaque_equal(&opaque, actual), Ok(equal));
        let basis = PublicationBasis { structured: None, opaque: Some(actual) };
        for poll in [RefinementBudget { steps: 1, value_bytes: 1 },
            RefinementBudget { steps: 3, value_bytes: 2 }, ample()]
        {
            let mut cursor = judgment.begin_refinement(basis, ample());
            let report = finish(&mut cursor, poll);
            assert_eq!(report.outcome, if equal { PublicationOutcome::StillValid } else {
                PublicationOutcome::Invalidated(PublicationInvalidation::OpaqueInput)
            });
            let again = cursor.advance(ample());
            assert_eq!(again.outcome, report.outcome);
            assert_eq!(again.total, report.total);
            assert_eq!(again.spent, RefinementBudget::default());
        }
    }
}

#[test]
fn both_lanes_share_each_poll_and_resume_without_recomparing_a_byte() {
    let (original, frontiers) = fixture(10, &[0, 2, 4]);
    let (current, current_frontiers) = fixture(11, &[0, 2, 4, 99]);
    let actual = input();
    let judgment = PublicationJudgment::bind(action(), Some(capture(&original, &frontiers)),
        Some(OpaqueJudgment::capture(&actual, b""))).unwrap();
    let basis = PublicationBasis { structured: Some((&current, &current_frontiers)), opaque: Some(&actual) };
    let mut cursor = judgment.begin_refinement(basis, ample());
    assert_eq!(cursor.action(), judgment.action());
    let zero = cursor.advance(RefinementBudget::default());
    assert!(matches!(zero.outcome, PublicationOutcome::NeedsRefinement { .. }));
    assert_eq!(zero.spent, RefinementBudget::default());
    assert_eq!(zero.total, RefinementBudget::default());
    let report = finish(&mut cursor, RefinementBudget { steps: 1, value_bytes: 1 });
    assert_eq!(report.outcome, PublicationOutcome::StillValid);
    assert_eq!(report.total.value_bytes, 9 + 10 + 2);
    assert_eq!(cursor.cancel(), report.total);
    assert_eq!(judgment.validate(judgment.action(), basis, ample()).outcome, PublicationOutcome::StillValid);
}

#[test]
fn byte_starvation_pauses_at_the_exact_opaque_offset_and_cancellation_preserves_work() {
    let actual = input();
    let judgment = PublicationJudgment::bind(action(), None, Some(OpaqueJudgment::capture(&actual, b""))).unwrap();
    let basis = PublicationBasis { structured: None, opaque: Some(&actual) };
    let mut cursor = judgment.begin_refinement(basis, ample());
    let prefix = cursor.advance(RefinementBudget { steps: 100, value_bytes: 4 });
    assert_eq!(prefix.total.value_bytes, 4);
    assert_eq!(prefix.outcome, PublicationOutcome::NeedsRefinement { minimum: RefinementBudget { steps: 1, value_bytes: 1 } });
    let paused = cursor.advance(RefinementBudget { steps: 100, value_bytes: 0 });
    assert_eq!(paused.total, prefix.total);
    assert_eq!(paused.spent, RefinementBudget::default());
    assert_eq!(cursor.cancel(), prefix.total);
    // Starting a genuinely new comparison does not inherit an old prefix result.
    let mut fresh = judgment.begin_refinement(basis, ample());
    assert_eq!(fresh.total_work(), RefinementBudget::default());
    assert_eq!(finish(&mut fresh, ample()).total.value_bytes, 12);
}

#[test]
fn repeated_poll_allowances_cannot_refill_a_frozen_total_ceiling() {
    let actual = input();
    let judgment = PublicationJudgment::bind(action(), None, Some(OpaqueJudgment::capture(&actual, b""))).unwrap();
    let basis = PublicationBasis { structured: None, opaque: Some(&actual) };
    let poll = RefinementBudget { steps: 1, value_bytes: 1 };
    let total = finish(&mut judgment.begin_refinement(basis, ample()), poll).total;
    for ceiling in [RefinementBudget::default(),
        RefinementBudget { steps: total.steps - 1, ..total },
        RefinementBudget { value_bytes: total.value_bytes - 1, ..total }, total]
    {
        let mut cursor = judgment.begin_refinement(basis, ceiling);
        let result = finish(&mut cursor, poll);
        assert_eq!(result.outcome, if ceiling == total { PublicationOutcome::StillValid }
            else { PublicationOutcome::Refused(Error::Limit) });
        let repeated = cursor.advance(ample());
        assert_eq!(repeated.outcome, result.outcome);
        assert_eq!(repeated.total, result.total);
        assert_eq!(repeated.spent, RefinementBudget::default());
    }
}

#[test]
fn structured_refusals_stop_before_opaque_work_and_keep_original_invalidation_reasons() {
    let (original, frontiers) = fixture(10, &[0, 2, 4]);
    let actual = input();
    let judgment = PublicationJudgment::bind(action(), Some(capture(&original, &frontiers)),
        Some(OpaqueJudgment::capture(&actual, b""))).unwrap();
    for (keys, reason) in [
        (vec![2, 4], Invalidation::ExactValue), (vec![0, 1, 2, 4], Invalidation::AbsentKey),
        (vec![0, 2, 4, 7], Invalidation::EmptyRange), (vec![0, 2, 3, 4], Invalidation::RangeMembers),
    ] {
        let (current, frontiers) = fixture(11, &keys);
        let basis = PublicationBasis { structured: Some((&current, &frontiers)), opaque: Some(&actual) };
        let report = finish(&mut judgment.begin_refinement(basis, ample()), RefinementBudget { steps: 1, value_bytes: 1 });
        assert!(matches!(report.outcome, PublicationOutcome::Invalidated(
            PublicationInvalidation::Structured { reason: found, .. }) if found == reason));
        assert!(report.total.value_bytes <= 9);
        assert_eq!(report.outcome, judgment.validate(judgment.action(), basis, ample()).outcome);
    }
    let missing = ProductFrontiers::new(1, 8).unwrap();
    let basis = PublicationBasis { structured: Some((&original, &missing)), opaque: Some(&actual) };
    let report = finish(&mut judgment.begin_refinement(basis, ample()), ample());
    assert_eq!(report.outcome, PublicationOutcome::Refused(Error::Incomplete));
    assert_eq!(report.total.value_bytes, 3);
}

#[test]
fn missing_and_extra_lanes_cannot_rewrite_the_frozen_contract() {
    let (snapshot, frontiers) = fixture(10, &[0, 2, 4]);
    let actual = input();
    let judgment = PublicationJudgment::bind(action(), Some(capture(&snapshot, &frontiers)),
        Some(OpaqueJudgment::capture(&actual, b""))).unwrap();
    for basis in [PublicationBasis::default(),
        PublicationBasis { structured: Some((&snapshot, &frontiers)), opaque: None },
        PublicationBasis { structured: None, opaque: Some(&actual) }]
    {
        let report = judgment.begin_refinement(basis, ample()).advance(ample());
        assert_eq!(report.outcome, PublicationOutcome::Refused(Error::Incomplete));
        assert_eq!(report.total, RefinementBudget::default());
    }
    let opaque = PublicationJudgment::bind(action(), None, Some(OpaqueJudgment::capture(&actual, b""))).unwrap();
    let report = opaque.begin_refinement(PublicationBasis {
        structured: Some((&snapshot, &frontiers)), opaque: Some(&actual),
    }, ample()).advance(ample());
    assert_eq!(report.outcome, PublicationOutcome::Refused(Error::Binding));
    assert_eq!(report.total, RefinementBudget::default());
}

#[test]
fn live_synchronous_comparison_preserves_legacy_costs_budget_admission_and_action_binding() {
    let (snapshot, frontiers) = fixture(10, &[0, 2, 4]);
    let actual = input();
    let structured = capture(&snapshot, &frontiers);
    let old_work = structured.begin_refinement(&snapshot, &frontiers).advance(ample()).spent;
    let judgment = PublicationJudgment::bind(action(), Some(structured), Some(OpaqueJudgment::capture(&actual, b""))).unwrap();
    let expected = RefinementBudget {
        steps: old_work.steps + 1 + actual.ordered_parts().len() as u64 + actual.omissions().len() as u64,
        value_bytes: old_work.value_bytes + 12,
    };
    for current in std::iter::once(actual.clone()).chain(mutations(&actual)) {
        let basis = PublicationBasis { structured: Some((&snapshot, &frontiers)), opaque: Some(&current) };
        let result = judgment.validate(judgment.action(), basis, expected);
        assert_eq!(result.spent, expected, "legacy full charge even on an early mismatch");
        assert_eq!(result.outcome, if current == actual { PublicationOutcome::StillValid } else {
            PublicationOutcome::Invalidated(PublicationInvalidation::OpaqueInput)
        });
        for ceiling in [RefinementBudget { steps: expected.steps - 1, ..expected },
            RefinementBudget { value_bytes: expected.value_bytes - 1, ..expected }]
        {
            let refused = judgment.validate(judgment.action(), basis, ceiling);
            assert!(matches!(refused.outcome, PublicationOutcome::NeedsRefinement { .. }));
            assert_eq!(refused.spent, old_work, "never spend a partial legacy opaque allowance");
        }
    }
    let mut altered = action().spec().clone(); altered.payload.push(0);
    let result = judgment.validate(&FrozenAction::freeze(altered).unwrap(), PublicationBasis {
        structured: Some((&snapshot, &frontiers)), opaque: Some(&actual),
    }, ample());
    assert_eq!(result.outcome, PublicationOutcome::Refused(Error::Binding));
    assert_eq!(result.spent, RefinementBudget::default());
}

#[test]
fn maximum_opaque_view_completes_without_repeated_byte_work_or_dropped_metadata() {
    let mut profile = input().input_profile().clone();
    profile.profile_bytes = vec![7; MAX_PROFILE_BYTES];
    let parts = (0..MAX_SUBMITTED_PARTS).map(|index| SubmittedPart {
        span: ByteSpan { start: index, end: if index + 1 == MAX_SUBMITTED_PARTS { MAX_SUBMITTED_BYTES } else { index + 1 } },
        kind: if index == 0 { PartKind::Question } else { PartKind::Other },
    }).collect();
    let omissions = (0..MAX_OMISSIONS).map(|domain_id| Omission::Unsupported { domain_id: domain_id as u64 }).collect();
    let original = ActualHelperInput::new(vec![11; MAX_SUBMITTED_BYTES], profile, parts, omissions).unwrap();
    let judgment = PublicationJudgment::bind(action(), None, Some(OpaqueJudgment::capture(&original, b""))).unwrap();
    let basis = PublicationBasis { structured: None, opaque: Some(&original) };
    let report = finish(&mut judgment.begin_refinement(basis, ample()), RefinementBudget { steps: 3, value_bytes: 64 });
    assert_eq!(report.outcome, PublicationOutcome::StillValid);
    assert_eq!(report.total.value_bytes, (MAX_PROFILE_BYTES + MAX_SUBMITTED_BYTES) as u64);
    let mut changed = original.omissions().to_vec();
    changed[MAX_OMISSIONS - 1] = Omission::Unsupported { domain_id: u64::MAX };
    let different = ActualHelperInput::new(original.submitted_bytes().to_vec(), original.input_profile().clone(),
        original.ordered_parts().to_vec(), changed).unwrap();
    let report = finish(&mut judgment.begin_refinement(PublicationBasis {
        structured: None, opaque: Some(&different),
    }, ample()), RefinementBudget { steps: 3, value_bytes: 64 });
    assert_eq!(report.outcome, PublicationOutcome::Invalidated(PublicationInvalidation::OpaqueInput));
    assert_eq!(report.total.value_bytes, (MAX_PROFILE_BYTES + MAX_SUBMITTED_BYTES) as u64);
}
