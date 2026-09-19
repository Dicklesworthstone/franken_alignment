use super::*;
use crate::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::oversight::publication::{PublicationBasis, PublicationOutcome};
use crate::full_input::{ByteSpan, InputProfileBinding, Omission, PartKind, SubmittedPart};
use crate::product_frontier::ProjectionKey;
use crate::witness::{DomainClosure, DomainProjection, QueryRole, MAX_SNAPSHOT_ENTRIES, MAX_VALUE_BYTES};
use crate::witness::refinement::RefinementBudget;

fn key() -> ProjectionKey { ProjectionKey { source: 1, branch: 2, projection: 3, source_epoch: 4 } }
fn marker() -> TrustedClosingMarker { TrustedClosingMarker { key: key(), final_sequence: 1, marker_generation: 7 } }
fn domain(closure: DomainClosure) -> AdapterDomainInput {
    AdapterDomainInput::new(DomainProjection::new(11, 12, key()), closure)
}
fn input(revision: u64, epoch: u64, keys: &[u64], frontiers: &ProductFrontiers) -> FileWitnessInput {
    FileWitnessInput::new(revision, revision, epoch, domain(DomainClosure::Closed(marker())),
        keys.iter().map(|key| SnapshotEntry::new(*key, 1, b"value".to_vec()).unwrap()).collect(), frontiers).unwrap()
}
fn helper() -> ActualHelperInput {
    let kinds = [PartKind::Question, PartKind::Prompt, PartKind::Instruction,
        PartKind::ToolSchema { schema_id: 8 }, PartKind::Evidence { source_id: 9, transform_id: 10 },
        PartKind::Delimiter, PartKind::Other];
    ActualHelperInput::new(b"QPISEDX".to_vec(), InputProfileBinding {
        profile_id: 2, profile_bytes: vec![0, 255, 2], tokenizer_epoch: 3, policy_epoch: 4, model_epoch: 5,
    }, kinds.into_iter().enumerate().map(|(i, kind)| SubmittedPart {
        span: ByteSpan { start: i, end: i + 1 }, kind,
    }).collect(), vec![
        Omission::ClosedAbsent { domain_id: 1, trusted_closure_marker_id: 9 },
        Omission::Gapped { domain_id: 2, first_missing: 3 },
        Omission::Unsupported { domain_id: 3 },
        Omission::Redacted { domain_id: 4, transform_id: 5 },
    ]).unwrap()
}
fn action() -> FrozenAction {
    FrozenAction::freeze(ActionSpec {
        version: VERSION,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(ResolvedTarget { adapter: 1, object: 2, contract_version: 1, expected_version: 1, generation: 1 }),
        payload: b"publish".to_vec(), required_witnesses: vec![], policy_epoch: 0,
        deadline: ElapsedTick(100), units: 128,
    }).unwrap()
}
fn requests() -> Vec<WitnessRequest> {
    vec![WitnessRequest::ExactValue { key: 0, role: QueryRole::Subject },
        WitnessRequest::AbsentKey { key: 1 }, WitnessRequest::EmptyRange { start: 6, end: 9 },
        WitnessRequest::RangeMembers { start: 2, end: 6 }]
}
fn validate(evidence: &FilePublicationEvidence, input: &FilePublicationInputs) -> Result<(), Error> {
    let current = input.materialize()?;
    evidence.capture_for(action())?.validate(&action(), PublicationBasis {
        structured: current.structured.as_ref().map(|(snapshot, frontiers)| (snapshot, frontiers)),
        opaque: current.opaque.as_ref(),
    }, RefinementBudget { steps: u64::MAX, value_bytes: u64::MAX }).require_valid()
}
fn fixture() -> FilePublicationEvidence {
    let frontiers = replay_frontiers(Some(marker())).unwrap();
    FilePublicationEvidence::new(FilePublicationInputs::new(
        Some(input(10, 30, &[4, 0, 2], &frontiers)), Some(helper())), requests()).unwrap()
}

#[test]
fn original_capture_and_both_lanes_round_trip_without_rebinding_to_current_state() {
    let original = fixture();
    let encoded = original.to_bytes().unwrap();
    let replayed = FilePublicationEvidence::from_bytes(&encoded).unwrap();
    assert_eq!(replayed, original);
    assert_eq!(replayed.to_bytes().unwrap(), encoded);
    assert_eq!(validate(&replayed, replayed.original()), Ok(()));
    let frontiers = replay_frontiers(Some(marker())).unwrap();
    let unrelated = FilePublicationInputs::new(Some(input(11, 30, &[0, 2, 4, 99], &frontiers)), Some(helper()));
    let unrelated = FilePublicationInputs::from_bytes(&unrelated.to_bytes().unwrap()).unwrap();
    assert_eq!(validate(&replayed, &unrelated), Ok(()));
    for keys in [vec![2, 4], vec![0, 1, 2, 4], vec![0, 2, 4, 7], vec![0, 2, 3, 4]] {
        let changed = FilePublicationInputs::new(Some(input(11, 30, &keys, &frontiers)), Some(helper()));
        let changed = FilePublicationInputs::from_bytes(&changed.to_bytes().unwrap()).unwrap();
        assert_eq!(validate(&replayed, &changed), Err(Error::Stale));
    }
}

#[test]
fn incomplete_frontiers_and_conservative_summaries_do_not_become_absence_on_decode() {
    let evidence = fixture();
    let mut gapped = ProductFrontiers::new(1, 8).unwrap();
    gapped.accept(key(), FrontierStage::Authenticated, 2).unwrap();
    let current = FilePublicationInputs::new(Some(input(11, 30, &[0, 2, 4], &gapped)), Some(helper()));
    let decoded = FilePublicationInputs::from_bytes(&current.to_bytes().unwrap()).unwrap();
    assert_eq!(decoded.structured().unwrap().admitted_close(), None);
    assert_eq!(validate(&evidence, &decoded), Err(Error::Incomplete));
    let closed = replay_frontiers(Some(marker())).unwrap();
    let summary = FileWitnessInput::new(11, 11, 30, domain(DomainClosure::ConservativeSummary),
        vec![SnapshotEntry::new(0, 1, b"value".to_vec()).unwrap()], &closed).unwrap();
    let summary = FilePublicationInputs::new(Some(summary), None);
    let decoded = FilePublicationInputs::from_bytes(&summary.to_bytes().unwrap()).unwrap();
    assert!(matches!(decoded.structured().unwrap().snapshot().domain_input().closure(), DomainClosure::ConservativeSummary));
    assert_eq!(FilePublicationEvidence::new(decoded, vec![WitnessRequest::AbsentKey { key: 1 }]), Err(Error::Incomplete));
}

#[test]
fn full_helper_bytes_metadata_omissions_and_semantics_survive_the_wire() {
    let evidence = fixture();
    let actual = helper();
    for mutation in 0..5 {
        let mut bytes = actual.submitted_bytes().to_vec();
        let mut profile = actual.input_profile().clone();
        let mut parts = actual.ordered_parts().to_vec();
        let mut omissions = actual.omissions().to_vec();
        match mutation {
            0 => bytes[6] ^= 1,
            1 => profile.model_epoch += 1,
            2 => profile.profile_bytes[0] ^= 1,
            3 => parts[6].kind = PartKind::Prompt,
            4 => omissions.pop().map(|_| ()).unwrap(),
            _ => unreachable!(),
        }
        let changed = ActualHelperInput::new(bytes, profile, parts, omissions).unwrap();
        let current = FilePublicationInputs::new(evidence.original.structured.clone(), Some(changed));
        let decoded = FilePublicationInputs::from_bytes(&current.to_bytes().unwrap()).unwrap();
        assert_eq!(decoded, current);
        assert_eq!(validate(&evidence, &decoded), Err(Error::Stale));
    }
}

#[test]
fn truncations_trailing_bytes_wrong_versions_and_cross_packet_types_refuse() {
    let evidence = fixture();
    let bytes = evidence.to_bytes().unwrap();
    for length in 0..bytes.len() { assert!(FilePublicationEvidence::from_bytes(&bytes[..length]).is_err()); }
    let mut trailing = bytes.clone(); trailing.push(0);
    assert!(FilePublicationEvidence::from_bytes(&trailing).is_err());
    let mut version = bytes.clone(); version[7] = b'2';
    assert_eq!(FilePublicationEvidence::from_bytes(&version), Err(Error::Binding));
    assert!(FilePublicationInputs::from_bytes(&bytes).is_err());
    assert!(FilePublicationEvidence::from_bytes(&evidence.original.to_bytes().unwrap()).is_err());
}

#[test]
fn replay_work_and_snapshot_bounds_have_positive_and_negative_neighbors() {
    let close = TrustedClosingMarker { final_sequence: MAX_REPLAY_PREFIX, ..marker() };
    let frontiers = replay_frontiers(Some(close)).unwrap();
    let original = FileWitnessInput::new(0, 0, 0, domain(DomainClosure::Closed(close)),
        vec![SnapshotEntry::new(u64::MAX, 1, vec![7; MAX_VALUE_BYTES]).unwrap()], &frontiers).unwrap();
    let evidence = FilePublicationEvidence::new(FilePublicationInputs::new(Some(original), None), vec![
        WitnessRequest::ExactValue { key: u64::MAX, role: QueryRole::PredicateInput },
        WitnessRequest::AbsentKey { key: 0 },
    ]).unwrap();
    let decoded = FilePublicationEvidence::from_bytes(&evidence.to_bytes().unwrap()).unwrap();
    assert_eq!(validate(&decoded, decoded.original()), Ok(()));
    assert_eq!(replay_frontiers(Some(TrustedClosingMarker { final_sequence: MAX_REPLAY_PREFIX + 1, ..close })), Err(Error::Limit));
    let too_many = (0..=MAX_SNAPSHOT_ENTRIES).map(|key| SnapshotEntry::new(key as u64, 1, vec![]).unwrap()).collect();
    assert_eq!(FileWitnessInput::new(0, 0, 0, domain(DomainClosure::Unknown), too_many, &frontiers), Err(Error::Limit));
}

#[test]
fn absent_lanes_are_preserved_but_cannot_form_empty_or_contradictory_bindings() {
    let missing = FilePublicationInputs::new(None, None);
    assert_eq!(FilePublicationInputs::from_bytes(&missing.to_bytes().unwrap()).unwrap(), missing);
    assert_eq!(FilePublicationEvidence::new(missing, vec![]), Err(Error::Incomplete));
    assert_eq!(FilePublicationEvidence::new(FilePublicationInputs::new(None, Some(helper())), requests()), Err(Error::Binding));
    let original = fixture();
    assert_eq!(FilePublicationEvidence::new(original.original.clone(), vec![WitnessRequest::AbsentKey { key: 0 }]), Err(Error::Binding));
    let duplicate = vec![WitnessRequest::AbsentKey { key: 1 }; 2];
    assert_eq!(FilePublicationEvidence::new(original.original.clone(), duplicate), Err(Error::Duplicate));
    let judgment = original.capture_for(action()).unwrap();
    assert_eq!(judgment.validate(&action(), PublicationBasis::default(), RefinementBudget::default()).outcome,
        PublicationOutcome::Refused(Error::Incomplete));
}
