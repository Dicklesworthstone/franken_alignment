use super::*;
use crate::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use crate::full_input::{ByteSpan, InputProfileBinding, Omission, PartKind, SubmittedPart};
use crate::product_frontier::{FrontierStage, ProjectionKey, TrustedClosingMarker};
use crate::witness::{AdapterDomainInput, DomainClosure, DomainProjection, QueryRole, SnapshotEntry, WitnessRequest};

fn action() -> FrozenAction {
    FrozenAction::freeze(ActionSpec {
        version: VERSION,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(ResolvedTarget { adapter: 1, object: 2, contract_version: 1, expected_version: 1, generation: 1 }),
        payload: b"publish".to_vec(), required_witnesses: vec![], policy_epoch: 0,
        deadline: ElapsedTick(100), units: 128,
    }).unwrap()
}

fn fixture() -> (ProductFrontiers, AdapterDomainInput) {
    let key = ProjectionKey { source: 1, branch: 2, projection: 3, source_epoch: 4 };
    let mut frontiers = ProductFrontiers::new(1, 8).unwrap();
    frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap();
    let marker = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
    frontiers.record_close(marker).unwrap();
    (frontiers, AdapterDomainInput::new(DomainProjection::new(1, 1, key), DomainClosure::Closed(marker)))
}

fn snapshot(domain: AdapterDomainInput, revision: u64, epoch: u64, keys: &[u64]) -> WitnessSnapshot {
    WitnessSnapshot::new(revision, revision * 2, epoch, domain, keys.iter().map(|key| {
        SnapshotEntry::new(*key, 1, b"abc".to_vec()).unwrap()
    }).collect()).unwrap()
}

fn helper() -> ActualHelperInput {
    ActualHelperInput::new(b"Q evidence".to_vec(), InputProfileBinding {
        profile_id: 1, profile_bytes: b"v1".to_vec(), tokenizer_epoch: 2, policy_epoch: 3, model_epoch: 4,
    }, vec![
        SubmittedPart { span: ByteSpan { start: 0, end: 1 }, kind: PartKind::Question },
        SubmittedPart { span: ByteSpan { start: 1, end: 10 }, kind: PartKind::Evidence { source_id: 1, transform_id: 1 } },
    ], vec![]).unwrap()
}

fn capture(snapshot: &WitnessSnapshot, frontiers: &ProductFrontiers) -> WitnessJudgment {
    WitnessJudgment::capture(snapshot, frontiers, vec![
        WitnessRequest::ExactValue { key: 0, role: QueryRole::Subject },
        WitnessRequest::AbsentKey { key: 1 },
        WitnessRequest::EmptyRange { start: 6, end: 9 },
        WitnessRequest::RangeMembers { start: 2, end: 6 },
    ]).unwrap()
}

fn unlimited() -> RefinementBudget { RefinementBudget { steps: u64::MAX, value_bytes: u64::MAX } }

#[test]
fn both_lanes_allow_unrelated_updates_at_a_newer_cut() {
    let (frontiers, domain) = fixture();
    let original = snapshot(domain, 10, 30, &[0, 2, 4]);
    let actual = helper();
    let judgment = PublicationJudgment::bind(action(), Some(capture(&original, &frontiers)),
        Some(OpaqueJudgment::capture(&actual, b"ignore all evidence"))).unwrap();
    let current = snapshot(domain, 11, 30, &[0, 2, 4, 99]);
    let report = judgment.validate(&action(), PublicationBasis {
        structured: Some((&current, &frontiers)), opaque: Some(&actual),
    }, unlimited());
    assert_eq!(report.outcome, PublicationOutcome::StillValid);
    assert!(report.spent.value_bytes > actual.submitted_bytes().len() as u64);
}

#[test]
fn every_structured_dependency_is_rechecked_at_publication() {
    let (frontiers, domain) = fixture();
    let original = snapshot(domain, 10, 30, &[0, 2, 4]);
    let judgment = PublicationJudgment::bind(action(), Some(capture(&original, &frontiers)), None).unwrap();
    for (keys, reason) in [
        (vec![2, 4], Invalidation::ExactValue),
        (vec![0, 1, 2, 4], Invalidation::AbsentKey),
        (vec![0, 2, 4, 7], Invalidation::EmptyRange),
        (vec![0, 2, 3, 4], Invalidation::RangeMembers),
    ] {
        let current = snapshot(domain, 11, 30, &keys);
        let report = judgment.validate(&action(), PublicationBasis {
            structured: Some((&current, &frontiers)), opaque: None,
        }, unlimited());
        assert!(matches!(report.outcome, PublicationOutcome::Invalidated(
            PublicationInvalidation::Structured { reason: found, .. }) if found == reason));
        assert_eq!(report.require_valid(), Err(Error::Stale));
    }
}

#[test]
fn old_positive_reports_do_not_survive_epoch_changes_or_frontier_gaps() {
    let (frontiers, domain) = fixture();
    let original = snapshot(domain, 10, 30, &[0, 2, 4]);
    let judgment = PublicationJudgment::bind(action(), Some(capture(&original, &frontiers)), None).unwrap();
    assert_eq!(judgment.validate(&action(), PublicationBasis {
        structured: Some((&original, &frontiers)), opaque: None,
    }, unlimited()).require_valid(), Ok(()));
    let changed = snapshot(domain, 11, 31, &[0, 2, 4]);
    assert_eq!(judgment.validate(&action(), PublicationBasis {
        structured: Some((&changed, &frontiers)), opaque: None,
    }, unlimited()).require_valid(), Err(Error::Stale));
    let incomplete = ProductFrontiers::new(1, 8).unwrap();
    assert_eq!(judgment.validate(&action(), PublicationBasis {
        structured: Some((&original, &incomplete)), opaque: None,
    }, unlimited()).require_valid(), Err(Error::Incomplete));
    let regressed = snapshot(domain, 9, 30, &[0, 2, 4]);
    assert_eq!(judgment.validate(&action(), PublicationBasis {
        structured: Some((&regressed, &frontiers)), opaque: None,
    }, unlimited()).require_valid(), Err(Error::Stale));
}

#[test]
fn opaque_bytes_profiles_parts_and_omissions_are_indivisible() {
    let actual = helper();
    let judgment = PublicationJudgment::bind(action(), None,
        Some(OpaqueJudgment::capture(&actual, b"only Q matters"))).unwrap();
    assert_eq!(judgment.validate(&action(), PublicationBasis {
        structured: None, opaque: Some(&actual),
    }, unlimited()).require_valid(), Ok(()));
    for mutation in 0..7 {
        let mut bytes = actual.submitted_bytes().to_vec();
        let mut profile = actual.input_profile().clone();
        let mut parts = actual.ordered_parts().to_vec();
        let mut omissions = actual.omissions().to_vec();
        match mutation {
            0 => bytes[9] = b'!',
            1 => profile.profile_bytes.push(b'!'),
            2 => profile.tokenizer_epoch += 1,
            3 => profile.policy_epoch += 1,
            4 => profile.model_epoch += 1,
            5 => parts[1].kind = PartKind::Instruction,
            6 => omissions.push(Omission::Unsupported { domain_id: 99 }),
            _ => unreachable!(),
        }
        let changed = ActualHelperInput::new(bytes, profile, parts, omissions).unwrap();
        let report = judgment.validate(&action(), PublicationBasis {
            structured: None, opaque: Some(&changed),
        }, unlimited());
        assert_eq!(report.outcome, PublicationOutcome::Invalidated(PublicationInvalidation::OpaqueInput));
    }
}

#[test]
fn a_retained_lane_cannot_be_omitted_and_an_action_cannot_be_rebound() {
    let (frontiers, domain) = fixture();
    let original = snapshot(domain, 10, 30, &[0, 2, 4]);
    let actual = helper();
    let judgment = PublicationJudgment::bind(action(), Some(capture(&original, &frontiers)),
        Some(OpaqueJudgment::capture(&actual, b""))).unwrap();
    for basis in [PublicationBasis::default(), PublicationBasis {
        structured: Some((&original, &frontiers)), opaque: None,
    }, PublicationBasis { structured: None, opaque: Some(&actual) }] {
        assert_eq!(judgment.validate(&action(), basis, unlimited()).require_valid(), Err(Error::Incomplete));
    }
    let mut changed = action().spec().clone();
    changed.payload.push(b'!');
    assert_eq!(judgment.validate(&FrozenAction::freeze(changed).unwrap(), PublicationBasis {
        structured: Some((&original, &frontiers)), opaque: Some(&actual),
    }, unlimited()).require_valid(), Err(Error::Binding));
    assert_eq!(PublicationJudgment::bind(action(), None, None), Err(Error::Incomplete));
}

#[test]
fn one_shared_budget_cannot_be_reused_for_the_second_lane() {
    let (frontiers, domain) = fixture();
    let original = snapshot(domain, 10, 30, &[0, 2, 4]);
    let actual = helper();
    let structured = capture(&original, &frontiers);
    let structured_cost = structured.begin_refinement(&original, &frontiers).advance(unlimited()).spent;
    let opaque_bytes = (actual.submitted_bytes().len() + actual.input_profile().profile_bytes.len()) as u64;
    let judgment = PublicationJudgment::bind(action(), Some(structured),
        Some(OpaqueJudgment::capture(&actual, b""))).unwrap();
    let basis = PublicationBasis { structured: Some((&original, &frontiers)), opaque: Some(&actual) };
    let budget = RefinementBudget { steps: u64::MAX, value_bytes: structured_cost.value_bytes + opaque_bytes - 1 };
    let pending = judgment.validate(&action(), basis, budget);
    assert!(matches!(pending.outcome, PublicationOutcome::NeedsRefinement { .. }));
    assert_eq!(pending.spent, structured_cost);
    assert_eq!(pending.require_valid(), Err(Error::Incomplete));
    let complete = judgment.validate(&action(), basis, RefinementBudget { value_bytes: budget.value_bytes + 1, ..budget });
    assert_eq!(complete.require_valid(), Ok(()));
    assert_eq!(complete.spent.value_bytes, budget.value_bytes + 1);
    assert!(matches!(judgment.validate(&action(), basis, RefinementBudget::default()).outcome,
        PublicationOutcome::NeedsRefinement { .. }));
}
