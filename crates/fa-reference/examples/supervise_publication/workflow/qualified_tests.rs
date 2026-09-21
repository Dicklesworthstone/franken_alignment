//! Reuse actual child-helper, reviewer-socket and publication fixtures. The
//! held-out labels below are synthetic test data, not calibrated model evidence.
use super::*;
use crate::workflow::continuation::submit_with_credibility;
use fa_reference::action::consequence::congress::{CredibilityBinding, CredibilityRequirements};
use fa_reference::action::consequence::congress::credibility::{
    Campaign, CaseSpec, CredibilityLedger, EvaluationLabel, EvaluationScope,
    HelperGeneration, LabelSource, LabelVerdict, Observation,
};
use fa_reference::action::consequence::delivery::persistent::credibility::CredibilityActivation;
use fa_reference::Error;
use std::collections::BTreeSet;

fn activation(config: &Config, sequence: u64, epoch: u64, operation: u64, generation: u64)
    -> CredibilityActivation
{
    // Campaign sequences are trusted reference inputs in this authority domain.
    // The first real review establishes sequence 1; activation advances to 2.
    let frontier = sequence.max(2);
    let helpers: BTreeMap<_, _> = config.profile.delivery.congress.members.iter().map(|(id, member)| (
        id.clone(), HelperGeneration {
            generation: config.profile.committee.members()[id].profile_at(0).model_epoch,
            cohort: member.cohort.clone(),
        },
    )).collect();
    let mut ledger = CredibilityLedger::new(Campaign {
        scope: EvaluationScope { campaign: operation, model_generation: 1,
            evaluator_generation: 1, held_out_manifest: [42; 32] },
        label_owner: "independent-evaluator".into(), helpers: helpers.clone(),
        strata: BTreeSet::from(["publication".into()]),
        cases: (1..=2).map(|id| CaseSpec { id, stratum: "publication".into(),
            evidence_root: [id as u8; 32], dispatch_sequence: frontier }).collect(),
    }).unwrap();
    for id in 1..=2 {
        let observation = if id == 1 { Observation::Clear }
            else { Observation::Hold { first_sequence: frontier - 1 } };
        ledger.record_observations(id, helpers.keys().map(|name| (name.clone(), observation)).collect()).unwrap();
        ledger.record_label(id, EvaluationLabel {
            owner: "independent-evaluator".into(), evaluator_generation: 1,
            source: LabelSource::IndependentEvaluation, evidence_root: [id as u8; 32],
            recorded_sequence: frontier,
            verdict: if id == 1 { LabelVerdict::Safe } else { LabelVerdict::Violation },
        }).unwrap();
    }
    let snapshot = ledger.seal(frontier).unwrap();
    CredibilityActivation {
        operation, expected_control_sequence: sequence, expected_epoch: epoch,
        scope: config.profile.delivery.scope, policy_generation: config.profile.delivery.policy.generation(),
        actor_profile: config.profile.delivery.actor.profile(),
        binding: CredibilityBinding { scope: snapshot.scope().clone(), label_owner: snapshot.label_owner().into(),
            helpers: snapshot.helpers().clone(), strata: snapshot.strata().clone(), reducer_generation: generation },
        stratum: "publication".into(),
        requirements: CredibilityRequirements { minimum_safe_cases: 1, minimum_violation_cases: 1,
            minimum_precision_ppm: 1_000_000, minimum_timely_recall_ppm: 1_000_000,
            maximum_false_positive_ppm: 0, base_weight: 5, lead_bonus_weight: 0,
            lead_saturation_sequences: 0, maximum_evidence_age: 100,
            maximum_member_share_ppm: 500_000, maximum_cohort_share_ppm: 500_000 },
        snapshot,
    }
}

fn prepared(root: &Directory, operation: u64, generation: u64, request: u64)
    -> (Config, Vec<u8>, PathBuf, CredibilityActivation)
{
    let config = configured(root, 0);
    let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    let after_open = before.control.ledger.epoch.checked_add(1).unwrap();
    let config = configured(root, after_open + 1);
    let activation = activation(&config, before.control.sequence, after_open, operation, generation);
    let path = root.0.join(format!("qualification-{operation}.bin"));
    std::fs::write(&path, activation.encode_reference().unwrap()).unwrap();
    let next = document(request, b"second", before.target, after_open + 1);
    (config, next, path, activation)
}

fn qualified_publication(root: &Directory) -> (Vec<u8>, PathBuf) {
    first_publication(root);
    let (config, next, path, activation) = prepared(root, 100, 2, 2);
    let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    let human = reviewer(&config, 2);
    let result = submit_with_credibility(config, &next, None, None, Some(&path), || ElapsedTick(1_001)).unwrap();
    let packet = human.join().unwrap();
    assert_executed(&result);
    assert_eq!(packet.action().spec().policy_epoch, activation.expected_epoch + 1);
    assert_eq!(packet.action().spec().payload, b"second");
    let config = configured(root, 2);
    let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(after.executions, 2);
    assert_eq!(after.target.expected_version, before.target.expected_version + 1);
    assert_eq!(after.control.ledger.charged, 11);
    assert_eq!(after.control.ledger.reserved, 0);
    assert_eq!(after.control.ledger.available, config.profile.delivery.total - 11);
    assert_eq!(after.control.sequence, before.control.sequence + 2); // Activation AND original review.
    assert_eq!(after.control.ledger.stages.len(), 2);
    (next, path)
}

#[test]
fn public_capsule_roundtrip_rebuilds_evidence_and_rejects_truncation_tail_and_size() {
    let root = Directory::new();
    let config = configured(&root, 2);
    let original = activation(&config, 1, 1, 100, 2);
    let bytes = original.encode_reference().unwrap();
    assert_eq!(CredibilityActivation::decode_reference(&bytes).unwrap(), original);
    for end in 0..bytes.len() {
        assert!(CredibilityActivation::decode_reference(&bytes[..end]).is_err());
    }
    let mut trailing = bytes.clone(); trailing.push(0);
    assert!(CredibilityActivation::decode_reference(&trailing).is_err());
    let mut bad_domain = bytes; bad_domain[0] ^= 1;
    assert!(CredibilityActivation::decode_reference(&bad_domain).is_err());
    assert_eq!(CredibilityActivation::decode_reference(&vec![0; CredibilityActivation::MAX_ENCODED_BYTES + 1]), Err(Error::Limit));
    // Encoding/decoding never opens the authority or validates a claimed scope.
    let mut foreign = original; foreign.scope.tenant += 1;
    assert_eq!(CredibilityActivation::decode_reference(&foreign.encode_reference().unwrap()).unwrap(), foreign);
    assert!(!config.store.exists());
}

#[test]
fn real_qualified_continuation_runs_original_helpers_two_keys_and_publication() {
    let root = Directory::new();
    qualified_publication(&root);
}

#[test]
fn qualified_actor_retry_needs_neither_capsule_source_helpers_nor_new_review() {
    let root = Directory::new();
    let (original, path) = qualified_publication(&root);
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_file(root.0.join("evidence.json")).unwrap();
    let mut config = configured(&root, 3); config.programs.clear();
    let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    let retry = submit_with_credibility(config, &original, None, None, Some(&path), || ElapsedTick(200_000)).unwrap();
    assert_executed(&retry);
    let mut config = configured(&root, 4); config.programs.clear();
    let conflict = document(2, b"altered", before.target, 2);
    let result = submit_with_credibility(config, &conflict, None, None, Some(&path), || ElapsedTick(200_001)).unwrap();
    assert_eq!(result.response.result, Err(WireError::IdempotencyConflict));
    let config = configured(&root, 4);
    let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(after.executions, before.executions);
    assert_eq!(after.control.sequence, before.control.sequence);
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(after.payload, before.payload);
    assert!(!config.socket(2).exists());
}

#[test]
fn invalid_capsule_or_original_epoch_refuses_before_admission_without_fallback() {
    for defect in 0..6 {
        let root = Directory::new();
        first_publication(&root);
        let (mut config, mut next, path, mut request) = prepared(&root, 100, 2, 2);
        let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        match defect {
            0 => { std::fs::remove_file(&path).unwrap(); }
            1 => { std::fs::write(&path, b"not a canonical qualification").unwrap(); }
            2 => { request.requirements.minimum_safe_cases = 2; }
            3 => { request.scope.tenant += 1; }
            4 => { request.expected_control_sequence += 1; }
            _ => { next = document(2, b"second", before.target, request.expected_epoch); }
        }
        if (2..=4).contains(&defect) {
            std::fs::write(&path, request.encode_reference().unwrap()).unwrap();
        }
        // The errors must not launch helpers or proceed to source acquisition.
        config.programs.clear();
        std::fs::remove_file(root.0.join("evidence.json")).unwrap();
        assert!(submit_with_credibility(config, &next, None, None, Some(&path), || ElapsedTick(1_001)).is_err());
        let config = configured(&root, 2);
        let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(after.revision, before.revision + 1); // Only the required recovery fence.
        assert_eq!(after.control.sequence, before.control.sequence);
        assert_eq!(after.control.ledger.stages.len(), 1);
        assert_eq!(after.executions, 1);
        assert_eq!(after.control.ledger.charged, 5);
        assert!(!config.socket(2).exists());
    }
}

#[test]
fn historical_activation_cannot_reopen_new_work_but_new_qualification_can() {
    let root = Directory::new();
    let (_, old_path) = qualified_publication(&root);
    let (mut config, next, _, _) = prepared(&root, 101, 3, 3);
    let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    config.programs.clear();
    assert!(submit_with_credibility(config, &next, None, None, Some(&old_path), || ElapsedTick(1_002)).is_err());
    let config = configured(&root, 4);
    let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(after.control.sequence, before.control.sequence);
    assert_eq!(after.executions, 2);
    assert_eq!(after.control.ledger.stages.len(), 2);
    let (config, fresh, path, _) = prepared(&root, 102, 3, 3);
    let human = reviewer(&config, 3);
    let result = submit_with_credibility(config, &fresh, None, None, Some(&path), || ElapsedTick(1_003)).unwrap();
    human.join().unwrap();
    assert_executed(&result);
    let config = configured(&root, 5);
    let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(after.executions, 3);
    assert_eq!(after.control.ledger.charged, 17);
}

#[test]
fn omitting_the_option_cannot_downgrade_an_already_qualified_store() {
    let root = Directory::new();
    qualified_publication(&root);
    let mut config = configured(&root, 3); config.programs.clear();
    let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    let next = document(3, b"not permitted", before.target, before.control.ledger.epoch + 1);
    let result = submit_existing(config, &next, None, None, || ElapsedTick(1_002)).unwrap();
    assert!(!matches!(&result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    let config = configured(&root, 3);
    let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(after.executions, 2);
    assert_eq!(after.control.ledger.charged, 11);
    assert_eq!(after.control.ledger.stages.len(), 2);
    assert!(!config.socket(3).exists());
}
