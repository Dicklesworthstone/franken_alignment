//! Qualification-aware planning plus original checked-publication composition.
use super::*;
use crate::proposal::{self, Basis};
use crate::publication::PublicationProfile;
use fa_reference::action::{ActionSpec, FrozenAction, VERSION};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::{
    FileCaptureIdentity, FilePublicationCapture,
};
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{
    FilePublicationInputs, FileWitnessInput,
};
use fa_reference::action::consequence::oversight::actor_wire::decode_command;
use fa_reference::product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker};
use fa_reference::witness::{AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry};

fn proposed(bytes: &[u8]) -> ActorProposal {
    let Command::Submit { proposal, .. } = decode_command(bytes).unwrap() else { panic!("expected proposal"); };
    proposal
}
fn bytes(config: &Config) -> Vec<u8> {
    std::fs::read(config.store.join("delivery.bin")).unwrap()
}

#[test]
fn planned_document_names_both_epochs_and_executes_without_editing_actor_bytes() {
    let root = Directory::new();
    first_publication(&root);
    let (config, _, path, request) = prepared(&root, 100, 2, 2);
    let before = bytes(&config);
    let ordinary = proposal::document(&config, 2, b"second".to_vec(), 12_000,
        ElapsedTick(1_001), Basis::Existing).unwrap();
    let planned = proposal::document_qualified(&config, 2, b"second".to_vec(), 12_000,
        ElapsedTick(1_001), &request).unwrap();
    let ordinary = proposed(&ordinary);
    let qualified = proposed(&planned);
    assert_eq!(qualified.expected_policy_epoch, ordinary.expected_policy_epoch + 1);
    assert_eq!(qualified.target, ordinary.target);
    assert_eq!(qualified.payload, ordinary.payload);
    assert_eq!(qualified.deadline, ordinary.deadline);
    assert_eq!(qualified.units, ordinary.units);
    assert_eq!(bytes(&config), before);
    assert!(!planned.windows(b"independent-evaluator".len()).any(|w| w == b"independent-evaluator"));
    let human = reviewer(&config, 2);
    let result = submit_with_credibility(config, &planned, None, None, Some(&path), || ElapsedTick(1_001)).unwrap();
    let packet = human.join().unwrap();
    assert_executed(&result);
    assert_eq!(packet.action().spec().deadline, qualified.deadline);
    assert_eq!(packet.action().spec().policy_epoch, qualified.expected_policy_epoch);
}

#[test]
fn planner_is_read_only_even_while_the_real_owner_holds_the_journal_lock() {
    let root = Directory::new();
    first_publication(&root);
    let config = configured(&root, 0);
    let (host, _) = FileOversight::open(&config.store, config.profile.clone()).unwrap();
    let before = host.inspect();
    let canonical = bytes(&config);
    let request = activation(&config, before.control.sequence, before.control.ledger.epoch + 1, 100, 2);
    let planned = proposal::document_qualified(&config, 2, b"next".to_vec(), 1_000,
        ElapsedTick(1_001), &request).unwrap();
    assert_eq!(proposed(&planned).expected_policy_epoch, before.control.ledger.epoch + 2);
    assert_eq!(host.inspect(), before);
    assert_eq!(bytes(&config), canonical);
}

#[test]
fn foreign_or_stale_planning_inputs_never_mutate_the_store() {
    let root = Directory::new();
    first_publication(&root);
    let (config, _, _, request) = prepared(&root, 100, 2, 2);
    let canonical = bytes(&config);
    for defect in 0..3 {
        let mut bad = request.clone();
        match defect {
            0 => bad.scope.tenant += 1,
            1 => bad.expected_control_sequence += 1,
            _ => bad.expected_epoch += 1,
        }
        assert!(proposal::document_qualified(&config, 2, b"next".to_vec(), 1_000,
            ElapsedTick(1_001), &bad).is_err());
        assert_eq!(bytes(&config), canonical);
    }
    for (request_id, ttl, now) in [(0, 1, 1), (2, 0, 1), (2, 15_001, 1), (2, 1, u64::MAX)] {
        assert!(proposal::document_qualified(&config, request_id, vec![1], ttl,
            ElapsedTick(now), &request).is_err());
    }
    assert_eq!(bytes(&config), canonical);
}

#[test]
fn planning_does_not_certify_insufficient_evidence_or_create_missing_authorities() {
    let root = Directory::new();
    let config = configured(&root, 2);
    let request = activation(&config, 1, 1, 100, 2);
    assert!(proposal::document_qualified(&config, 2, vec![1], 1_000,
        ElapsedTick(1_001), &request).is_err());
    assert!(!config.store.exists());
    first_publication(&root);
    let (mut config, _, path, mut request) = prepared(&root, 100, 2, 2);
    request.requirements.minimum_safe_cases = 2;
    std::fs::write(&path, request.encode_reference().unwrap()).unwrap();
    let planned = proposal::document_qualified(&config, 2, b"next".to_vec(), 12_000,
        ElapsedTick(1_001), &request).unwrap();
    config.programs.clear();
    assert!(submit_with_credibility(config, &planned, None, None, Some(&path), || ElapsedTick(1_001)).is_err());
    let config = configured(&root, 2);
    let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(after.executions, 1);
    assert_eq!(after.control.ledger.charged, 5);
    assert_eq!(after.control.ledger.stages.len(), 1);
}

fn checked_profile(root: &Directory) -> PublicationProfile {
    PublicationProfile::decode(format!(r#"{{"schema":"fa.supervised-witnesses/1","source":91,
        "original":"{}/original.bin","current":"{}/current.bin",
        "limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},
        "requests":[{{"kind":"absent_key","key":1}}]}}"#,
        root.0.display(), root.0.display()).as_bytes()).unwrap()
}
fn capture(config: &Config, document: &[u8], attempt: u64, generation: u64, keys: &[u64])
    -> FilePublicationCapture
{
    let proposal = proposed(document);
    // Only execution fields enter the ORIGINAL action-frame encoder. This is
    // immutable fixture data, not an independently constructed permit/judgment.
    let action = FrozenAction::freeze(ActionSpec { version: VERSION,
        scope: config.profile.delivery.scope, target: Some(proposal.target),
        payload: proposal.payload, units: proposal.units, deadline: proposal.deadline,
        policy_epoch: proposal.expected_policy_epoch, required_witnesses: Vec::new() }).unwrap();
    let key = ProjectionKey { source: 40, branch: action.spec().scope.branch, projection: 7, source_epoch: 1 };
    let close = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap();
    frontiers.record_close(close).unwrap();
    let input = FileWitnessInput::new(generation, generation, 20,
        AdapterDomainInput::new(DomainProjection::new(40, 1, key), DomainClosure::Closed(close)),
        keys.iter().map(|key| SnapshotEntry::new(*key, 1, b"original".to_vec()).unwrap()).collect(),
        &frontiers).unwrap();
    FilePublicationCapture::new(attempt, FileCaptureIdentity { source: 91, generation },
        &action, FilePublicationInputs::new(Some(input), None)).unwrap()
}
fn checked_first(root: &Directory) {
    let config = configured(root, 0);
    evidence(root, &config);
    let document = document(1, b"first", config.profile.delivery.target, 0);
    let original = capture(&config, &document, 1, 1, &[0]).to_bytes().unwrap();
    std::fs::write(root.0.join("original.bin"), &original).unwrap();
    std::fs::write(root.0.join("current.bin"), &original).unwrap();
    let profile = checked_profile(root);
    let human = reviewer(&config, 1);
    let result = workflow::run_with_publication(config, &document, false, None, Some(&profile), || ElapsedTick(1_000)).unwrap();
    human.join().unwrap();
    assert_executed(&result);
}

#[test]
fn qualified_checked_workflow_still_enforces_original_negative_witnesses() {
    for phantom in [false, true] {
        let root = Directory::new();
        checked_first(&root);
        let (config, _, path, request) = prepared(&root, 100, 2, 2);
        let planned = proposal::document_qualified(&config, 2, b"second".to_vec(), 12_000,
            ElapsedTick(1_001), &request).unwrap();
        let original = capture(&config, &planned, 2, 2, &[0]);
        let keys: &[u64] = if phantom { &[0, 1] } else { &[0, 99] };
        let current = capture(&config, &planned, 2, 3, keys);
        std::fs::write(root.0.join("original.bin"), original.to_bytes().unwrap()).unwrap();
        std::fs::write(root.0.join("current.bin"), current.to_bytes().unwrap()).unwrap();
        let profile = checked_profile(&root);
        let human = reviewer(&config, 2);
        let result = submit_with_credibility(config, &planned, None, Some(&profile), Some(&path), || ElapsedTick(1_001)).unwrap();
        human.join().unwrap();
        if !phantom { assert_executed(&result); }
        else { assert!(!matches!(&result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }))); }
        let config = configured(&root, 2);
        let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(after.executions, if phantom { 1 } else { 2 });
        assert_eq!(after.control.ledger.charged, if phantom { 5 } else { 11 });
        assert_eq!(after.control.ledger.reserved, 0);
        assert_eq!(after.payload, if phantom { b"first".to_vec() } else { b"second".to_vec() });
    }
}

#[test]
fn qualification_cannot_remove_a_stored_checked_profile() {
    let root = Directory::new();
    checked_first(&root);
    let (mut config, next, path, _) = prepared(&root, 100, 2, 2);
    let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    config.programs.clear();
    assert!(submit_with_credibility(config, &next, None, None, Some(&path), || ElapsedTick(1_001)).is_err());
    let config = configured(&root, 2);
    let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(after.executions, before.executions);
    assert_eq!(after.control.sequence, before.control.sequence);
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(after.control.ledger.stages.len(), 1);
    assert!(!config.socket(2).exists());
}
