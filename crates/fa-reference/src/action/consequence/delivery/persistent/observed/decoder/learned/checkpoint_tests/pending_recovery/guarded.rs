//! Original guarded roles, actual paired resets and one original journal cut.
use super::*;
use crate::action::consequence::delivery::persistent::observed::{BaseEvent,
    guarded::{FileGuardSet, FileIdentityRequirement, FileCampaignRequirement,
        FileRecoveryRequirements, FileRecoveryFloor, FileOversightRoles,
        learned_recovery::FileGuardedLearnedRecovery}};
use crate::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame,
    identity::{ModelManifest, ModelPassport, IdentityAnchor}};
use crate::action::consequence::oversight::identity::{IdentityPolicy, IdentityStatus};
use crate::action::consequence::policy_campaign::ReplayLimits;
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};

fn guards(identity: bool, campaigns: bool) -> FileGuardSet {
    let manifest = ModelManifest { tenant: 1, model: 2, model_generation: 3, host_generation: 1,
        tokenizer_generation: 4, weights: [1; 32], adapters: [2; 32], tokenizer: [3; 32],
        architecture: [4; 32], numeric_profile: [5; 32] };
    let anchor = IdentityAnchor::new(10, CaptureProfile { tenant: 1, model: 2, model_generation: 3,
        tap: 4, layout_generation: 1 }, 5, vec![7], &[[-1.0, 1.0]]).unwrap();
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None, credential: None,
        identity: identity.then(|| FileIdentityRequirement {
            passport: ModelPassport::new(51, 1, manifest, vec![anchor]).unwrap(),
            policy: IdentityPolicy { observer_id: 99, timeout_ticks: 10, validity_ticks: 20, max_checks: 16 },
        }),
        campaigns: campaigns.then_some(FileCampaignRequirement {
            limits: ReplayLimits { cases: 16, input_bytes: 1_048_576 }, max_campaigns: 8 }),
    }
}
fn requirements(host: &FileOversight, guards: FileGuardSet) -> FileRecoveryRequirements {
    let control = host.inspect().control;
    FileRecoveryRequirements { guards, effective_policy: profile().delivery.policy, credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: host.revision(), control_sequence: control.sequence,
            authority_epoch: control.ledger.epoch } }
}
fn create(root: &Directory, config: &FileLearnedConfig, guards: &FileGuardSet)
    -> (FileOversight, FileOversightRoles)
{
    let (mut host, roles) = FileOversight::create_guarded_with_learned_generation(
        root.store(), profile(), guards, None, config.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, roles)
}
fn drive(run: &mut FileGuardedLearnedRecovery, quantum: usize) -> Result<(), JournalError> {
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        let before = run.progress();
        let next = run.advance(before.replayed_events, quantum)?;
        assert!(next.replayed_events - before.replayed_events <= quantum);
    }
    Ok(())
}
fn fresh_identity(host: &mut FileOversight, roles: &FileOversightRoles, old: &FileOversightRoles) {
    let expected = guards(true, true).identity.unwrap();
    let c = host.inspect().control;
    let actor = host.actor_snapshot().unwrap().actor_revision;
    let challenge = host.begin_identity_check(host.revision(), 17, c.sequence, actor).unwrap().unwrap();
    let revision = host.revision();
    assert!(old.identity_observer.as_ref().unwrap().observe_manifest(host, revision, &challenge,
        expected.passport.manifest().clone(), ElapsedTick(2)).is_err());
    assert_eq!(host.revision(), revision);
    let observer = roles.identity_observer.as_ref().unwrap();
    observer.observe_manifest(host, revision, &challenge, expected.passport.manifest().clone(),
        ElapsedTick(2)).unwrap().measurement.unwrap();
    let a = &expected.passport.anchors()[&10];
    let frame = SourceFrame::capture(FrameIdentity { profile: a.profile(), stream: a.stream(),
        sequence: 1, position: 0 }, &[0.0]).unwrap();
    let revision = host.revision();
    observer.observe_anchor(host, revision, &challenge, 10, &frame, ElapsedTick(2)).unwrap().measurement.unwrap();
    host.apply_identity_check(host.revision(), &challenge, c.sequence, c.ledger.epoch).unwrap();
    assert_ne!(host.identity_status().unwrap(), IdentityStatus::Missing);
}

#[test]
fn cooperative_normal_recovery_preserves_every_declared_role_and_original_pause() {
    for identity in [false, true] { for campaigns in [false, true] {
        let root = Directory::new(); let config = config(false, 1); let g = guards(identity, campaigns);
        let (mut host, _) = create(&root, &config, &g); step(&mut host).unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let expected = requirements(&host, g); let bytes = canonical(&root); drop(host);
        let mut run = FileOversight::begin_open_guarded_with_learned_generation(
            root.store(), profile(), &expected, &config).unwrap();
        assert_eq!(run.progress().replayed_events, 0);
        assert_eq!(run.advance(1, 1).err(), Some(Error::Stale.into()));
        assert_eq!(run.advance(0, 0).err(), Some(Error::InvalidInput.into()));
        assert!(FileOversight::begin_open_with_learned_generation(root.store(), profile(), &config).is_err());
        drive(&mut run, 1).unwrap(); assert_eq!(canonical(&root), bytes);
        let (host, roles) = run.finish().unwrap();
        assert_eq!(host.revision(), expected.minimum.journal_revision + 1);
        assert_eq!(roles.identity_observer.is_some(), identity);
        assert_eq!(roles.policy_governor.is_some(), campaigns);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert!(host.learned_generation_inspection().unwrap().paused); assert!(!host.clock_ready());
        assert_eq!(host.inspect().executions, 0);
    } }
}

#[test]
fn anchored_pending_reset_recovers_all_roles_and_requires_fresh_identity_and_both_keys() {
    let root = Directory::new(); let config = config(false, 1); let g = guards(true, true);
    let (mut host, old) = create(&root, &config, &g);
    step(&mut host).unwrap(); let checkpoint = capture(&mut host, 1); step(&mut host).unwrap();
    let intent = instruction(&host, 1, 900); begin(&mut host, &checkpoint, &intent);
    let mut expected = requirements(&host, g); let cut = host.revision();
    let anchor = host.history_anchor().unwrap(); let before = canonical(&root); drop(host);
    let mut run = FileOversight::begin_open_guarded_anchored_with_learned_generation(
        root.store(), profile(), &expected, &config, &anchor).unwrap();
    // Caller mutations cannot downgrade or invalidate the independently frozen contract.
    expected.guards.identity = None; expected.minimum.journal_revision = u64::MAX;
    drive(&mut run, 2).unwrap(); assert_eq!(canonical(&root), before);
    let (mut host, roles) = run.finish_pending_reset(&intent).unwrap();
    assert_eq!(host.revision(), cut + 2); assert_eq!(anchor.revision(), cut);
    assert!(roles.identity_observer.is_some() && roles.policy_governor.is_some());
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
    assert!(host.pending_learned_reset().unwrap().is_none());
    let extended = host.history_anchor_after(&anchor).unwrap(); assert_eq!(extended.revision(), host.revision());
    assert!(!host.clock_ready()); resume(&mut host, 2);
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    step(&mut host).unwrap(); fresh_identity(&mut host, &roles, &old);
    let (action, input, automatic, request) = prepared(&mut host);
    let revision = host.revision(); assert!(old.human.approve(&mut host, revision, &request).is_err());
    assert!(host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(2)).is_err());
    let human = roles.human.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
    host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(3)).unwrap();
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().payload, b"visible"); assert_eq!(host.inspect().control.ledger.charged, 16);
    // Both governors are real role objects: only the newly provisioned one may
    // reject a current original campaign. Rejection grants no effect permission.
    let c = host.inspect().control;
    let update = crate::action::consequence::delivery::persistent::PolicyUpdate::new(800,
        c.sequence, c.ledger.epoch,
        Policy::new(2, vec![Predicate::PayloadAtMost(128)]).unwrap()).unwrap();
    let campaign = host.request_policy_campaign(host.revision(), &update).unwrap();
    let revision = host.revision();
    assert!(old.policy_governor.as_ref().unwrap().reject(&mut host, revision, &campaign).is_err());
    roles.policy_governor.as_ref().unwrap().reject(&mut host, revision, &campaign).unwrap();
    assert_eq!(host.inspect().executions, 1);
}

#[test]
fn full_guard_policy_and_floor_mismatches_latch_failure_without_completing_reset_or_cleaning() {
    let root = Directory::new(); let config = config(false, 1); let g = guards(true, true);
    let (mut host, _) = create(&root, &config, &g); step(&mut host).unwrap(); let saved = capture(&mut host, 1);
    let intent = instruction(&host, 1, 900); begin(&mut host, &saved, &intent);
    let expected = requirements(&host, g); let before = canonical(&root); drop(host);
    std::fs::write(root.store().join("delivery.pending"), b"unacknowledged staging").unwrap();
    for field in 0..7 {
        let mut wrong = expected.clone();
        match field {
            0 => wrong.guards.identity = None,
            1 => wrong.guards.campaigns = None,
            2 => wrong.guards.identity.as_mut().unwrap().policy.observer_id += 1,
            3 => wrong.effective_policy = Policy::new(2, vec![Predicate::PayloadAtMost(128)]).unwrap(),
            4 => wrong.minimum.journal_revision += 1,
            5 => wrong.minimum.control_sequence += 1,
            _ => wrong.minimum.authority_epoch += 1,
        }
        let mut run = FileOversight::begin_open_guarded_with_learned_generation(
            root.store(), profile(), &wrong, &config).unwrap();
        let error = drive(&mut run, 3).unwrap_err();
        assert!(matches!(run.progress().status, FileLearnedRecoveryStatus::Failed(_)));
        assert_eq!(run.advance(run.progress().replayed_events, 1).err(), Some(error.clone()));
        assert_eq!(run.finish_pending_reset(&intent).err(), Some(error));
        assert_eq!(canonical(&root), before);
        assert_eq!(std::fs::read(root.store().join("delivery.pending")).unwrap(), b"unacknowledged staging");
    }
    let mut run = FileOversight::begin_open_guarded_with_learned_generation(
        root.store(), profile(), &expected, &config).unwrap();
    drive(&mut run, 1).unwrap(); let (host, roles) = run.finish_pending_reset(&intent).unwrap();
    assert!(roles.identity_observer.is_some() && roles.policy_governor.is_some());
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
    assert!(!root.store().join("delivery.pending").exists());
}

#[test]
fn exact_anchor_rejects_valid_equal_counter_forks_before_replay_and_accepts_original_successors() {
    let root = Directory::new(); let config = config(false, 1); let g = guards(false, true);
    let (mut host, _) = create(&root, &config, &g); step(&mut host).unwrap(); let saved = capture(&mut host, 1);
    let intent = instruction(&host, 1, 900); begin(&mut host, &saved, &intent);
    let prefix = host.history_anchor().unwrap();
    let mut fork_events = host.events.clone(); let identity = host.store.identity().to_path_buf();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let anchored = host.history_anchor().unwrap(); let expected = requirements(&host, g);
    let original = canonical(&root); drop(host);
    fork_events.push(Event::Core(BaseEvent::Time(ElapsedTick(3))));
    Machine::replay(&profile(), &fork_events).unwrap();
    let fork = journal::encode(&profile(), &identity, &fork_events).unwrap();
    std::fs::write(root.store().join("delivery.bin"), &fork).unwrap();
    let error = FileOversight::begin_open_guarded_anchored_with_learned_generation(
        root.store(), profile(), &expected, &config, &anchored).unwrap_err();
    assert_eq!(error, Error::Binding.into()); assert_eq!(canonical(&root), fork);
    // Both forks legitimately extend the older prefix, which authenticates
    // neither suffix. The tighter anchor is not silently replaced by this one.
    let mut run = FileOversight::begin_open_guarded_anchored_with_learned_generation(
        root.store(), profile(), &expected, &config, &prefix).unwrap();
    drive(&mut run, 2).unwrap(); drop(run); assert_eq!(canonical(&root), fork);
    std::fs::write(root.store().join("delivery.bin"), &original).unwrap();
    let mut run = FileOversight::begin_open_guarded_anchored_with_learned_generation(
        root.store(), profile(), &expected, &config, &anchored).unwrap();
    drive(&mut run, 2).unwrap();
    let (host, _) = run.finish_pending_reset(&intent).unwrap();
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    assert_eq!(host.revision(), anchored.revision() + 2);
}

#[test]
fn every_guarded_storage_failure_returns_no_role_bundle_and_never_publishes_reset_without_fence() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = config(false, 1); let g = guards(true, true);
        let (mut host, _) = create(&root, &config, &g); step(&mut host).unwrap(); let saved = capture(&mut host, 1);
        let intent = instruction(&host, 1, 900); begin(&mut host, &saved, &intent);
        let expected = requirements(&host, g); let anchor = host.history_anchor().unwrap(); drop(host);
        let mut run = FileOversight::begin_open_guarded_anchored_with_learned_generation(
            root.store(), profile(), &expected, &config, &anchor).unwrap();
        drive(&mut run, 1).unwrap(); run.fail_once(barrier);
        let error = run.finish_pending_reset(&intent).unwrap_err();
        assert!(matches!(error, JournalError::Io(ref failure) if failure.operation == barrier));
        let disk = FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap();
        let visible = barrier == JournalIo::DirectorySync;
        assert_eq!(disk.revision, anchor.revision() + if visible { 2 } else { 0 });
        let mut run = FileOversight::begin_open_guarded_anchored_with_learned_generation(
            root.store(), profile(), &expected, &config, &anchor).unwrap();
        drive(&mut run, 3).unwrap(); let (host, roles) = run.finish_pending_reset(&intent).unwrap();
        assert!(roles.identity_observer.is_some() && roles.policy_governor.is_some());
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
        assert_eq!(host.revision(), anchor.revision() + if visible { 3 } else { 2 });
        assert_eq!(host.inspect().executions, 0); assert!(!host.clock_ready());
    }
}

#[test]
fn guarded_early_finish_and_drop_do_not_advance_or_quarantine_the_pending_instruction() {
    let root = Directory::new(); let config = config(false, 1); let g = guards(true, true);
    let (mut host, _) = create(&root, &config, &g); step(&mut host).unwrap(); let saved = capture(&mut host, 1);
    let intent = instruction(&host, 1, 900); begin(&mut host, &saved, &intent);
    let expected = requirements(&host, g); let before = canonical(&root); drop(host);
    let mut run = FileOversight::begin_open_guarded_with_learned_generation(
        root.store(), profile(), &expected, &config).unwrap();
    run.advance(0, 1).unwrap();
    assert_eq!(run.finish_pending_reset(&intent).err(), Some(Error::Incomplete.into()));
    assert_eq!(canonical(&root), before);
    let mut run = FileOversight::begin_open_guarded_with_learned_generation(
        root.store(), profile(), &expected, &config).unwrap(); drive(&mut run, 2).unwrap(); drop(run);
    assert_eq!(canonical(&root), before);
    let mut run = FileOversight::begin_open_guarded_with_learned_generation(
        root.store(), profile(), &expected, &config).unwrap(); drive(&mut run, 2).unwrap();
    // Explicit ordinary finish keeps the original conservative quarantine.
    let (host, _) = run.finish().unwrap(); assert!(host.pending_learned_reset().unwrap().unwrap().interrupted);
    drop(host);
    let mut run = FileOversight::begin_open_guarded_with_learned_generation(
        root.store(), profile(), &expected, &config).unwrap(); drive(&mut run, 2).unwrap();
    let before = canonical(&root);
    assert_eq!(run.finish_pending_reset(&intent).err(), Some(Error::WrongState.into()));
    assert_eq!(canonical(&root), before);
}
