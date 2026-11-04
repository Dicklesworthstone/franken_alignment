//! Original learned numerics, real native congress, distinct human approval,
//! and canonical storage barriers around one explicit policy transaction.
use super::*;
use crate::action::{ActionState, consequence::policy_campaign::{ReplayLimits, PolicyReplayReport}};
use crate::action::consequence::delivery::persistent::{
    FileDeliverySnapshot, RecoveryReserve, governance::PolicyUpdate,
    observed::{journal,
        decoder::learned::{FileLearnedPolicyPromotionRequest, FileLearnedRecoveryStatus, bind_history},
        guarded::{FileCampaignRequirement, FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
            anchored::FileHistoryAnchor, learned_recovery::FileGuardedLearnedRecovery},
    },
};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::learned_host::HostedLearnedInspection;
use super::super::stream::stream_profile;

const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
    JournalIo::Rename, JournalIo::DirectorySync];

fn campaign() -> FileCampaignRequirement {
    FileCampaignRequirement { limits: ReplayLimits { cases: 16, input_bytes: 1_048_576 },
        max_campaigns: 8 }
}
fn candidate() -> Policy { Policy::new(2, vec![Predicate::PayloadAtMost(1024)]).unwrap() }

struct Case {
    config: FileLearnedConfig,
    required: FileRecoveryRequirements,
    anchor: FileHistoryAnchor,
    request: FileLearnedPolicyPromotionRequest,
    snapshot: FileDeliverySnapshot,
    numerical: HostedLearnedInspection,
    report: PolicyReplayReport,
    attempt: u64,
}

// A charged Unknown and an otherwise identical reserved action distinguish
// genuine liability preservation from indiscriminate cancellation/refunding.
fn prepared(root: &Directory, unknown: bool, denied: bool, next: Policy) -> Case {
    let config = stream_config();
    let (mut host, human) = FileOversight::create_with_learned_text_stream_with_reserve(
        root.store(), stream_delivery_profile(), config.clone(), RecoveryReserve::terminal()).unwrap();
    host.enable_policy_campaigns(host.revision(), campaign().limits, campaign().max_campaigns).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    for _ in 0..3 { step(&mut host); }
    let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut evidence = snapshot();
    if denied { evidence.values.insert(7, b"bad".to_vec()); }
    observe(&mut supervisor, evidence);
    let ticket = port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    if denied {
        assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Denied, .. }));
        let host = supervisor.host().unwrap();
        return retained(&host, config, next, 1);
    }
    let mut run = review(FileSupervisedDriver::new(supervisor), 71, 101, b"allow");
    complete(&mut run);
    let human_key = approve(&mut run, &human, 1001);
    let attempt = {
        let host = run.supervisor().host().unwrap();
        let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(71).unwrap().disposition
            else { panic!("learned request must retain its original attempt"); };
        attempt
    };
    if unknown {
        assert!(matches!(tick(&mut run, Some(&human_key)), FileDriverEvent::Dispatched { .. }));
        port.cancel(&ticket).unwrap();
        assert!(matches!(port.poll(&ticket), Knowledge::Unknown { .. }));
    } else {
        let mut host = run.supervisor_mut().host_mut().unwrap();
        let inputs = host.machine.broker.current_inputs(attempt).unwrap().unwrap().clone();
        let revision = host.revision();
        host.authorize(revision, attempt, &inputs, snapshot()).unwrap();
        assert_eq!(host.inspect().control.ledger.stages[&attempt], ActionState::Authorized);
    }
    let host = run.supervisor().host().unwrap();
    retained(&host, config, next, attempt)
}

fn retained(host: &FileOversight, config: FileLearnedConfig, next: Policy, attempt: u64) -> Case {
    let snapshot = host.inspect();
    let required = FileRecoveryRequirements {
        guards: FileGuardSet { stream: Some(stream_profile()), decoder: None, decoder_stop: None,
            source: None, identity: None, campaigns: Some(campaign()), credential: None },
        effective_policy: host.current_policy().unwrap().clone(), credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: snapshot.revision,
            control_sequence: snapshot.control.sequence, authority_epoch: snapshot.control.ledger.epoch },
    };
    let request = FileLearnedPolicyPromotionRequest::new(snapshot.revision, PolicyUpdate::new(
        901, snapshot.control.sequence, snapshot.control.ledger.epoch, next.clone()).unwrap()).unwrap();
    Case { config, required, anchor: host.history_anchor().unwrap(), request,
        numerical: host.learned_generation_inspection().unwrap().numerical,
        report: host.replay_candidate_policy(next, campaign().limits).unwrap(), snapshot, attempt }
}

fn ready(root: &Directory, case: &Case) -> FileGuardedLearnedRecovery {
    ready_with_profile(root, case, stream_delivery_profile())
}
fn ready_with_profile(root: &Directory, case: &Case, profile: FileOversightProfile)
    -> FileGuardedLearnedRecovery
{
    let mut run = FileOversight::begin_open_guarded_anchored_with_learned_generation(
        root.store(), profile, &case.required, &case.config, &case.anchor).unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal()).unwrap()
        .require_policy_promotion(case.request.clone()).unwrap();
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1).unwrap();
    }
    assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Ready);
    run
}
fn canonical(root: &Directory) -> Vec<u8> { std::fs::read(root.store().join("delivery.bin")).unwrap() }

#[test]
fn original_campaign_promotion_cancels_only_reserved_rights_and_keeps_unknown_charged() {
    for unknown in [false, true] {
        let root = Directory::new(); let mut case = prepared(&root, unknown, false, candidate());
        assert_eq!(case.report.cases().len(), 2, "original proposal AND native review retained");
        let run = ready(&root, &case);
        let expected = run.policy_promotion_review().unwrap().clone();
        assert_eq!(expected.snapshot(), &case.snapshot);
        assert_eq!(expected.report(), &case.report);
        let result = run.finish_promoted_policy(&expected, false).unwrap();
        assert_eq!(result.appended_events(), 3);
        assert_eq!(result.snapshot().revision, case.snapshot.revision + 3);
        assert_eq!(result.current_policy(), &candidate());
        assert_eq!(result.receipt().update(), case.request.update());
        assert_eq!(result.snapshot().executions, 0);
        assert_eq!(result.snapshot().control.ledger.charged, case.snapshot.control.ledger.charged);
        assert!(result.snapshot().control.ledger.epoch > case.snapshot.control.ledger.epoch);
        if unknown {
            assert!(case.snapshot.control.ledger.charged > 0);
            assert!(result.receipt().change().cancelled.is_empty());
            assert_eq!(result.snapshot().control.ledger.stages[&case.attempt], ActionState::Unknown);
        } else {
            assert!(case.snapshot.control.ledger.reserved > 0);
            assert_eq!(result.receipt().change().cancelled, vec![case.attempt]);
            assert_eq!(result.snapshot().control.ledger.reserved, 0);
            assert_eq!(result.snapshot().control.ledger.stages[&case.attempt], ActionState::Cancelled);
        }
        let after = canonical(&root);
        case.required.effective_policy = candidate();
        let repeated = ready(&root, &case);
        // The ORIGINAL cut, not the newer policy/cancelled inventory, is what
        // the governor compares on a lost-output retry.
        assert_eq!(repeated.policy_promotion_review().unwrap(), &expected);
        let repeated = repeated.finish_promoted_policy(&expected, false).unwrap();
        assert_eq!(repeated.appended_events(), 0);
        assert_eq!(repeated.receipt(), result.receipt());
        assert_eq!(repeated.history_anchor(), result.history_anchor());
        assert_eq!(canonical(&root), after);
        let (host, _) = FileOversight::open_with_learned_generation(
            root.store(), stream_delivery_profile(), &case.config).unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, case.numerical);
        assert_eq!(host.inspect().control.ledger.charged, case.snapshot.control.ledger.charged);
    }
}

#[test]
fn full_denied_corpus_needs_explicit_relaxation_and_shadow_never_becomes_approval() {
    let root = Directory::new(); let case = prepared(&root, false, true, candidate());
    let before = canonical(&root);
    let run = ready(&root, &case); let expected = run.policy_promotion_review().unwrap().clone();
    assert!(!expected.report().newly_reviewable().is_empty());
    assert_eq!(run.finish_promoted_policy(&expected, false).err(), Some(Error::Binding.into()));
    assert_eq!(canonical(&root), before);
    let result = ready(&root, &case).finish_promoted_policy(&expected, true).unwrap();
    assert!(result.accepted_newly_reviewable());
    assert_eq!(result.snapshot().control.ledger.stages[&case.attempt], ActionState::Denied);
    let root = Directory::new();
    let case = prepared(&root, false, false, Policy::new(2, vec![Predicate::Absent { key: 999 }]).unwrap());
    let before = canonical(&root); let run = ready(&root, &case);
    let expected = run.policy_promotion_review().unwrap().clone();
    assert!(expected.report().requires_shadow());
    assert_eq!(run.finish_promoted_policy(&expected, true).err(), Some(Error::Incomplete.into()));
    assert_eq!(canonical(&root), before);
}

#[test]
fn all_storage_barriers_publish_either_all_three_original_events_or_none() {
    for barrier in BARRIERS {
        let root = Directory::new(); let mut case = prepared(&root, true, false, candidate());
        let before = canonical(&root);
        let run = ready(&root, &case); let expected = run.policy_promotion_review().unwrap().clone();
        run.fail_once(barrier);
        assert!(matches!(run.finish_promoted_policy(&expected, false),
            Err(JournalError::Io(failure)) if failure.operation == barrier));
        if barrier == JournalIo::DirectorySync { case.required.effective_policy = candidate(); }
        else { assert_eq!(canonical(&root), before); }
        let retried = ready(&root, &case);
        assert_eq!(retried.policy_promotion_review().unwrap(), &expected);
        let result = retried.finish_promoted_policy(&expected, false).unwrap();
        assert_eq!(result.appended_events(), if barrier == JournalIo::DirectorySync { 0 } else { 3 });
        assert_eq!(result.snapshot().revision, case.snapshot.revision + 3);
        assert_eq!(result.snapshot().control.ledger.charged, case.snapshot.control.ledger.charged);
    }
}

#[test]
fn original_report_identity_acceptance_and_canonical_bytes_are_exact_retry_bindings() {
    let first = Directory::new(); let mut case = prepared(&first, false, false, candidate());
    let other = Directory::new();
    let other_case = prepared(&other, false, false, Policy::new(3, vec![Predicate::PayloadAtMost(1)]).unwrap());
    let foreign = ready(&other, &other_case).policy_promotion_review().unwrap().clone();
    let before = canonical(&first);
    assert_eq!(ready(&first, &case).finish_promoted_policy(&foreign, false).err(), Some(Error::Binding.into()));
    assert_eq!(canonical(&first), before);
    let run = ready(&first, &case); let expected = run.policy_promotion_review().unwrap().clone();
    let mut changed = before.clone(); changed[0] ^= 1;
    std::fs::write(first.store().join("delivery.bin"), &changed).unwrap();
    assert_eq!(run.finish_promoted_policy(&expected, false).err(), Some(Error::Binding.into()));
    assert_eq!(canonical(&first), changed);
    std::fs::write(first.store().join("delivery.bin"), &before).unwrap();
    ready(&first, &case).finish_promoted_policy(&expected, false).unwrap();
    case.required.effective_policy = candidate();
    let after = canonical(&first);
    assert_eq!(ready(&first, &case).finish_promoted_policy(&expected, true).err(), Some(Error::Binding.into()));
    assert_eq!(canonical(&first), after);
    assert_eq!(ready(&first, &case).finish_promoted_policy(&expected, false).unwrap().appended_events(), 0);
}

#[test]
fn campaign_promotion_cannot_spend_the_terminal_event_or_byte_reserve() {
    for ordinary in [0, 1, 2, 3] {
        let root = Directory::new(); let mut case = prepared(&root, true, false, candidate());
        let run = ready(&root, &case); let expected = run.policy_promotion_review().unwrap().clone();
        drop(run);
        let mut profile = stream_delivery_profile();
        let store = crate::action::consequence::delivery::persistent::storage::Store::open(&root.store()).unwrap();
        let mut history = journal::decode(&profile, store.identity(), &canonical(&root)).unwrap();
        bind_history(&mut history, &case.config).unwrap();
        profile.delivery.limits.events = history.len() + ordinary + RecoveryReserve::terminal().events;
        let original = journal::encode(&profile, store.identity(), &history).unwrap();
        case.anchor = FileHistoryAnchor::from_acknowledged(history.len(), original.clone());
        drop(store);
        std::fs::write(root.store().join("delivery.bin"), &original).unwrap();
        let result = ready_with_profile(&root, &case, profile).finish_promoted_policy(&expected, false);
        if ordinary < 3 {
            assert_eq!(result.err(), Some(Error::Limit.into()));
            assert_eq!(canonical(&root), original);
        } else { assert_eq!(result.unwrap().appended_events(), 3); }
    }
    let root = Directory::new(); let mut case = prepared(&root, true, false, candidate());
    let run = ready(&root, &case); let expected = run.policy_promotion_review().unwrap().clone();
    drop(run);
    let mut profile = stream_delivery_profile();
    let store = crate::action::consequence::delivery::persistent::storage::Store::open(&root.store()).unwrap();
    let before = canonical(&root);
    let mut history = journal::decode(&profile, store.identity(), &before).unwrap();
    bind_history(&mut history, &case.config).unwrap();
    profile.delivery.limits.bytes = before.len() + RecoveryReserve::terminal().bytes;
    let original = journal::encode(&profile, store.identity(), &history).unwrap();
    case.anchor = FileHistoryAnchor::from_acknowledged(history.len(), original.clone());
    drop(store);
    std::fs::write(root.store().join("delivery.bin"), &original).unwrap();
    assert_eq!(ready_with_profile(&root, &case, profile).finish_promoted_policy(&expected, false).err(),
        Some(Error::Limit.into()));
    assert_eq!(canonical(&root), original);
}
