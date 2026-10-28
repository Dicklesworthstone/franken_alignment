//! Explicit original stream reset completion before the recovery fence.
//! Real numerical/publication paths with synthetic weights and review ballots;
//! no detector effectiveness or operating-system isolation claim is implied.
use super::*;
use fa_reference::action::consequence::delivery::{
    StopRequest,
    credential_broker::{BrokerRouteBinding, CredentialRotationRequest},
    persistent::observed::{
        credential::{FILE_OVERSIGHT_CREDENTIAL_PROFILE, FileCredentialPolicy},
        decoder::learned::{
            FileLearnedRecovery, FileLearnedRecoveryProgress, FileLearnedRecoveryStatus,
            checkpoint::FileLearnedResetIntent,
        },
        guarded::{
            FileCredentialEpoch, FileCredentialRegistration, FileGuardSet,
            FileRecoveryFloor, FileRecoveryRequirements,
            learned_recovery::FileGuardedLearnedRecovery,
        },
    },
};
use fa_reference::perimeter_inventory::LoadedPerimeterInventory;

#[derive(Clone, Copy, Debug)]
enum Family { Core, Guarded }

enum Recovery {
    Core(FileLearnedRecovery),
    Guarded(FileGuardedLearnedRecovery),
}
impl Recovery {
    fn progress(&self) -> FileLearnedRecoveryProgress {
        match self { Self::Core(run) => run.progress(), Self::Guarded(run) => run.progress() }
    }
    fn advance(&mut self, count: usize) -> Result<FileLearnedRecoveryProgress, JournalError> {
        let previous = self.progress().replayed_events;
        match self {
            Self::Core(run) => run.advance(previous, count),
            Self::Guarded(run) => run.advance(previous, count),
        }
    }
    fn ready(mut self) -> Self {
        while self.progress().status == FileLearnedRecoveryStatus::Replaying {
            self.advance(3).unwrap();
        }
        assert_eq!(self.progress().status, FileLearnedRecoveryStatus::Ready);
        self
    }
    fn finish(self) -> Result<(FileOversight, FileHumanReviewer), JournalError> {
        match self {
            Self::Core(run) => run.finish(),
            Self::Guarded(run) => run.finish().map(|(host, roles)| (host, roles.human)),
        }
    }
    fn finish_reset(self, intent: &FileLearnedResetIntent, first: u64)
        -> Result<(FileOversight, FileHumanReviewer), JournalError>
    {
        match self {
            Self::Core(run) => run.finish_pending_stream_reset(intent, first),
            Self::Guarded(run) => run.finish_pending_stream_reset(intent, first)
                .map(|(host, roles)| (host, roles.human)),
        }
    }
    fn finish_legacy(self, intent: &FileLearnedResetIntent)
        -> Result<(FileOversight, FileHumanReviewer), JournalError>
    {
        match self {
            Self::Core(run) => run.finish_pending_reset(intent),
            Self::Guarded(run) => run.finish_pending_reset(intent)
                .map(|(host, roles)| (host, roles.human)),
        }
    }
}

fn guards() -> FileGuardSet {
    FileGuardSet { stream: Some(stream()), decoder: None, decoder_stop: None,
        source: None, identity: None, campaigns: None, credential: None }
}
fn start(family: Family, root: &Directory, recipe: &FileLearnedConfig)
    -> (FileOversight, FileHumanReviewer)
{
    match family {
        Family::Core => restart_owner(root, recipe),
        Family::Guarded => {
            let (mut host, roles) = FileOversight::create_guarded_with_learned_generation(
                root.store(), restart_profile(), &guards(), None, recipe.clone()).unwrap();
            host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
            (host, roles.human)
        }
    }
}
fn requirements(host: &FileOversight) -> FileRecoveryRequirements {
    // Retained by the supervisor BEFORE dropping the live owner. No recovery
    // requirement is inferred from the rollbackable history being opened.
    let view = host.inspect(); let credential = host.credential_status().unwrap();
    let mut selected = guards();
    selected.credential = credential.as_ref().map(|status| status.policy.clone());
    FileRecoveryRequirements { guards: selected,
        effective_policy: host.current_policy().unwrap().clone(),
        credential_epoch: credential.map(|status| FileCredentialEpoch {
            generation: status.generation, revoked: status.revoked }),
        minimum: FileRecoveryFloor { journal_revision: host.revision(),
            control_sequence: view.control.sequence, authority_epoch: view.control.ledger.epoch } }
}
fn begin(family: Family, root: &Directory, recipe: &FileLearnedConfig,
    expected: &FileRecoveryRequirements) -> Recovery
{
    match family {
        Family::Core => Recovery::Core(FileOversight::begin_open_with_learned_generation(
            root.store(), restart_profile(), recipe).unwrap()),
        Family::Guarded => Recovery::Guarded(FileOversight::begin_open_guarded_with_learned_generation(
            root.store(), restart_profile(), expected, recipe).unwrap()),
    }
}
fn acknowledge(host: &mut FileOversight, saved: &FileLearnedStreamCheckpoint, first: u64)
    -> FileLearnedResetIntent
{
    let intent = FileLearnedResetIntent::for_recovery(saved.id(),
        reset_instruction(host, 900), restart_budget()).unwrap();
    host.begin_learned_stream_reset(host.revision(), saved, intent.control().clone(),
        intent.budget(), first).unwrap();
    let pending = host.pending_learned_reset().unwrap().unwrap();
    assert_eq!(pending.intent, intent); assert!(!pending.interrupted);
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
    intent
}
fn pending(host: &mut FileOversight) -> (FileLearnedResetIntent, u64) {
    step(host); let saved = capture_stream(host, 1); step(host);
    let first = incident_frontier(host, 800);
    (acknowledge(host, &saved, first), first)
}

#[test]
fn pending_stream_recovery_completes_once_then_requires_time_resume_generation_and_both_new_keys() {
    for family in [Family::Core, Family::Guarded] {
        let recipe = finite_recipe(6, false, true); let root = Directory::new();
        let (mut host, old_reviewer) = start(family, &root, &recipe);
        step(&mut host); let saved = capture_stream(&mut host, 1);
        let original = host.learned_generation_inspection().unwrap().numerical;
        step(&mut host); let first = incident_frontier(&mut host, 800); generate(&mut host);
        let old_action = message(&mut host, 1);
        let (old_inputs, old_automatic, old_offer) = prepared(&mut host, 1, &old_action);
        let revision = host.revision();
        let old_human = old_reviewer.approve(&mut host, revision, &old_offer).unwrap();
        let spent = host.learned_generation_inspection().unwrap().numerical;
        let intent = acknowledge(&mut host, &saved, first);
        let expected = requirements(&host); let cut = host.revision();
        let audience = host.stream_snapshot().unwrap(); let before = root.files(); drop(host);
        let run = begin(family, &root, &recipe, &expected).ready();
        assert_eq!(root.files(), before);
        let (mut host, reviewer) = run.finish_reset(&intent, first).unwrap();
        assert_eq!(host.revision(), cut + 2); assert!(!host.clock_ready());
        assert!(host.pending_learned_reset().unwrap().is_none());
        assert!(host.learned_generation_inspection().unwrap().paused);
        let result = host.learned_reset_result(900).unwrap().unwrap();
        assert!(result.control.restored); assert_eq!(result.control.cancelled, vec![1]);
        assert_eq!(result.control.incident_count, 1);
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
        let restored = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(restored.position, original.position);
        assert_eq!(restored.sampled_draws, original.sampled_draws);
        assert_eq!(restored.work, original.work);
        assert_eq!(restored.cumulative_work, spent.cumulative_work);
        assert!(restored.cumulative_telemetry.source_check_values > spent.cumulative_telemetry.source_check_values);
        assert_eq!(restored.availability, LearnedAvailability::Empty);
        assert_eq!(host.stream_snapshot().unwrap().confirmed, audience.confirmed);
        assert_eq!(host.stream_snapshot().unwrap().published, audience.published);
        assert_eq!(host.inspect().control.ledger.charged, audience.publication.control.ledger.charged);
        assert!(host.inspect().control.ledger.epoch > expected.minimum.authority_epoch);
        assert_eq!(host.current_policy().unwrap(), &expected.effective_policy);
        assert!(host.check_learned_stream_checkpoint(&saved).is_err());
        let bytes = root.files();
        assert!(host.dispatch(host.revision(), &old_automatic, &old_human,
            &old_action, &old_inputs, snapshot()).is_err());
        assert!(host.resume_learned_generation(host.revision(), restored.actor_revision, restored.position).is_err());
        assert!(host.advance_learned_generation(host.revision(), restored.actor_revision, restored.position).is_err());
        assert_eq!(root.files(), bytes);
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        let bytes = root.files();
        assert!(host.advance_learned_generation(host.revision(), restored.actor_revision, restored.position).is_err());
        assert_eq!(root.files(), bytes);
        host.resume_learned_generation(host.revision(), restored.actor_revision, restored.position).unwrap();
        let bytes = root.files();
        assert!(host.propose_learned_text_stream_message(host.revision(), 2,
            ElapsedTick(100), Vec::new(), snapshot()).is_err());
        assert_eq!(root.files(), bytes);
        generate(&mut host);
        assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"A");
        let action = message(&mut host, 2);
        let (inputs, automatic, offer) = prepared(&mut host, 2, &action);
        let bytes = root.files(); let revision = host.revision();
        assert!(old_reviewer.approve(&mut host, revision, &offer).is_err());
        assert!(host.publish_checked(host.revision(), 2, Some(&inputs), snapshot(), ElapsedTick(2)).is_err());
        assert_eq!(root.files(), bytes);
        let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &offer).unwrap();
        let bytes = root.files();
        assert!(host.dispatch(host.revision(), &old_automatic, &human, &action, &inputs, snapshot()).is_err());
        assert!(host.dispatch(host.revision(), &automatic, &old_human, &action, &inputs, snapshot()).is_err());
        assert_eq!(root.files(), bytes); assert_eq!(host.inspect().executions, 0);
        host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
        publish(&mut host, 2, &inputs, 3); host.reconcile(host.revision(), 2).unwrap();
        assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), ["A"]);
        assert_eq!(host.inspect().executions, 1);
        assert_eq!(host.inspect().control.ledger.charged, action.spec().units);
    }
}

#[test]
fn recovered_later_message_retains_the_confirmed_prefix_offset_and_original_request_slot() {
    for family in [Family::Core, Family::Guarded] {
        let recipe = finite_recipe(6, false, true); let root = Directory::new();
        let (mut host, reviewer) = start(family, &root, &recipe); generate(&mut host);
        let first_charge = confirmed_message(&mut host, &reviewer, 71, "A", 2);
        continue_message(&mut host, 71, 72); step(&mut host);
        let saved = capture_stream(&mut host, 1);
        let first = incident_frontier(&mut host, 800); generate(&mut host);
        let intent = acknowledge(&mut host, &saved, first);
        let expected = requirements(&host); let audience = host.stream_snapshot().unwrap(); drop(host);
        let (mut host, reviewer) = begin(family, &root, &recipe, &expected).ready()
            .finish_reset(&intent, first).unwrap();
        assert_eq!(host.stream_snapshot().unwrap().confirmed, audience.confirmed);
        assert_eq!(host.stream_snapshot().unwrap().published, audience.published);
        assert_eq!(host.inspect().target, audience.publication.target);
        assert_eq!(host.inspect().executions, 1);
        assert_eq!(host.inspect().control.ledger.charged, first_charge);
        assert!(host.learned_generation_inspection().unwrap().paused);
        resume(&mut host, 3); generate(&mut host);
        assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"B");
        let second_charge = confirmed_message(&mut host, &reviewer, 72, "B", 4);
        assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), ["A", "B"]);
        assert_eq!(host.inspect().executions, 2);
        assert_eq!(host.inspect().control.ledger.charged, first_charge + second_charge);
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    }
}

#[test]
fn every_incomplete_replay_cut_refuses_without_cleanup_and_the_complete_neighbor_recovers() {
    for family in [Family::Core, Family::Guarded] {
        let recipe = finite_recipe(6, false, true); let root = Directory::new();
        let (mut host, _) = start(family, &root, &recipe);
        let (intent, first) = pending(&mut host); let expected = requirements(&host);
        let revision = host.revision(); let bytes = root.files(); drop(host);
        let run = begin(family, &root, &recipe, &expected);
        let total = run.progress().total_events; drop(run);
        for cut in 0..total {
            let mut run = begin(family, &root, &recipe, &expected);
            if cut > 0 { run.advance(cut).unwrap(); }
            assert_eq!(run.progress().replayed_events, cut);
            assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Replaying);
            assert_eq!(run.finish_reset(&intent, first).err(), Some(Error::Incomplete.into()));
            assert_eq!(root.files(), bytes);
        }
        let (host, _) = begin(family, &root, &recipe, &expected).ready().finish_reset(&intent, first).unwrap();
        assert_eq!(host.revision(), revision + 2);
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
    }
}

#[test]
fn retained_command_frontier_and_stream_kind_must_all_match_before_recovery_can_reset() {
    for family in [Family::Core, Family::Guarded] {
        let recipe = finite_recipe(6, false, true); let root = Directory::new();
        let (mut host, _) = start(family, &root, &recipe);
        let (intent, first) = pending(&mut host); let expected = requirements(&host);
        let bytes = root.files(); drop(host);
        let mut wrong = vec![FileLearnedResetIntent::for_recovery(2,
            intent.control().clone(), intent.budget()).unwrap()];
        for field in 0..6 {
            let mut control = intent.control().clone();
            match field {
                0 => control.operation += 1,
                1 => control.expected_control_sequence += 1,
                2 => control.expected_authority_epoch += 1,
                3 => control.expected_actor_revision += 1,
                4 => control.binding.evidence_root[0] ^= 1,
                _ => control.retained_targets[0].expected_version += 1,
            }
            wrong.push(FileLearnedResetIntent::for_recovery(1, control, intent.budget()).unwrap());
        }
        let mut budget = intent.budget(); budget.cache_values -= 1;
        wrong.push(FileLearnedResetIntent::for_recovery(1, intent.control().clone(), budget).unwrap());
        for changed in wrong {
            assert_eq!(begin(family, &root, &recipe, &expected).ready().finish_reset(&changed, first).err(),
                Some(Error::Binding.into()));
            assert_eq!(root.files(), bytes);
        }
        for changed in [0, first + 1] {
            assert_eq!(begin(family, &root, &recipe, &expected).ready().finish_reset(&intent, changed).err(),
                Some(if changed == 0 { Error::InvalidInput } else { Error::Binding }.into()));
            assert_eq!(root.files(), bytes);
        }
        assert_eq!(begin(family, &root, &recipe, &expected).ready().finish_legacy(&intent).err(),
            Some(Error::Binding.into()));
        assert_eq!(root.files(), bytes);
        let (host, _) = begin(family, &root, &recipe, &expected).ready().finish_reset(&intent, first).unwrap();
        assert!(host.learned_reset_result(900).unwrap().unwrap().control.restored);
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    }
}

#[test]
fn unrecorded_fenced_stopped_and_ordinary_reopened_instructions_cannot_be_rebased() {
    for family in [Family::Core, Family::Guarded] {
        for barrier in 0..4 {
            let recipe = finite_recipe(6, false, true); let root = Directory::new();
            let (mut host, _) = start(family, &root, &recipe);
            step(&mut host); let saved = capture_stream(&mut host, 1); step(&mut host);
            let first = incident_frontier(&mut host, 800);
            let intent = if barrier == 0 {
                FileLearnedResetIntent::for_recovery(1, reset_instruction(&host, 900), restart_budget()).unwrap()
            } else { acknowledge(&mut host, &saved, first) };
            if barrier == 1 { host.fence(host.revision()).unwrap(); }
            if barrier == 2 {
                let control = host.inspect().control;
                host.request_stop(host.revision(), StopRequest { operation: 700,
                    expected_control_sequence: control.sequence,
                    expected_authority_epoch: control.ledger.epoch }).unwrap();
            }
            let mut expected = requirements(&host); drop(host);
            if barrier == 3 {
                let (host, _) = begin(family, &root, &recipe, &expected).ready().finish().unwrap();
                assert!(host.pending_learned_reset().unwrap().unwrap().interrupted);
                assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
                expected = requirements(&host); drop(host);
            }
            let bytes = root.files();
            assert_eq!(begin(family, &root, &recipe, &expected).ready().finish_reset(&intent, first).err(),
                Some(if barrier == 0 { Error::Missing } else { Error::WrongState }.into()));
            assert_eq!(root.files(), bytes);
        }
    }
}

#[test]
fn exact_already_completed_stream_recovery_adds_only_the_usual_fence() {
    for family in [Family::Core, Family::Guarded] {
        let recipe = finite_recipe(6, false, true); let root = Directory::new();
        let (mut host, _) = start(family, &root, &recipe);
        let (intent, first) = pending(&mut host); let expected = requirements(&host); drop(host);
        let (mut host, _) = begin(family, &root, &recipe, &expected).ready().finish_reset(&intent, first).unwrap();
        for _ in 0..2 {
            let expected = requirements(&host); let revision = host.revision();
            let numerical = host.learned_generation_inspection().unwrap().numerical;
            let usage = host.learned_recovery_usage().unwrap();
            let result = host.learned_reset_result(900).unwrap().unwrap().control;
            let bytes = root.files(); drop(host);
            assert_eq!(begin(family, &root, &recipe, &expected).ready().finish_reset(&intent, first + 1).err(),
                Some(Error::Binding.into()));
            assert_eq!(begin(family, &root, &recipe, &expected).ready().finish_legacy(&intent).err(),
                Some(Error::Binding.into()));
            assert_eq!(root.files(), bytes);
            (host, _) = begin(family, &root, &recipe, &expected).ready().finish_reset(&intent, first).unwrap();
            assert_eq!(host.revision(), revision + 1);
            assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
            assert_eq!(host.learned_recovery_usage().unwrap(), usage);
            assert_eq!(host.learned_reset_result(900).unwrap().unwrap().control, result);
            assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
            assert!(host.pending_learned_reset().unwrap().is_none());
            assert!(!host.clock_ready()); assert!(host.learned_generation_inspection().unwrap().paused);
            assert_eq!(host.inspect().executions, 0);
        }
    }
}

#[test]
fn original_writer_lock_and_exact_canonical_cut_survive_stream_reset_recovery() {
    for family in [Family::Core, Family::Guarded] {
        let recipe = finite_recipe(6, false, true); let root = Directory::new();
        let (mut host, _) = start(family, &root, &recipe);
        let (intent, first) = pending(&mut host); let expected = requirements(&host);
        let original = std::fs::read(root.store().join("delivery.bin")).unwrap(); drop(host);
        let run = begin(family, &root, &recipe, &expected).ready();
        assert_eq!(FileOversight::begin_open_with_learned_generation(
            root.store(), restart_profile(), &recipe).err(), Some(JournalError::Busy));
        // A noncooperating writer substitutes bytes AFTER this cursor verified
        // its original cut. The reset must not overwrite that substitution.
        let mut substituted = original.clone(); let last = substituted.len() - 1; substituted[last] ^= 1;
        std::fs::write(root.store().join("delivery.bin"), &substituted).unwrap();
        let changed = root.files();
        assert_eq!(run.finish_reset(&intent, first).err(), Some(Error::Binding.into()));
        assert_eq!(root.files(), changed);
        std::fs::write(root.store().join("delivery.bin"), original).unwrap();
        let (host, _) = begin(family, &root, &recipe, &expected).ready().finish_reset(&intent, first).unwrap();
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    }
}

#[test]
fn guarded_pending_stream_recovery_keeps_current_policy_all_floors_and_rotated_credentials() {
    let recipe = finite_recipe(6, false, true); let root = Directory::new();
    let document = include_str!("../../../fixtures/authority_topology_inventory.json")
        .replace("reference-publication", FILE_OVERSIGHT_CREDENTIAL_PROFILE);
    let inventory = LoadedPerimeterInventory::from_json_bytes(document.as_bytes()).unwrap();
    let binding = BrokerRouteBinding { family: "file-publication".into(), route: "publication".into() };
    let profile = restart_profile(); let mut selected = guards();
    selected.credential = Some(FileCredentialPolicy::from_inventory(&inventory, &binding,
        profile.delivery.scope, profile.delivery.target).unwrap());
    let registration = Some(FileCredentialRegistration { inventory: &inventory, binding: &binding });
    let (mut host, _) = FileOversight::create_guarded_with_learned_generation(
        root.store(), profile, &selected, registration, recipe.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    step(&mut host); let saved = capture_stream(&mut host, 1); step(&mut host);
    host.rotate_credential_guard(host.revision(), CredentialRotationRequest {
        operation: 700, expected_generation: 1, next_generation: 2 }).unwrap();
    let first = incident_frontier(&mut host, 800);
    let intent = acknowledge(&mut host, &saved, first);
    let expected = requirements(&host); let credential = host.credential_status().unwrap();
    assert_eq!(expected.credential_epoch, Some(FileCredentialEpoch { generation: 2, revoked: false }));
    assert_eq!(expected.effective_policy.generation(), 2);
    let anchor = host.history_anchor().unwrap(); let bytes = root.files(); drop(host);
    for field in 0..7 {
        let mut wrong = expected.clone();
        let error = match field {
            0 => { wrong.effective_policy = restart_profile().delivery.policy; Error::Binding }
            1 => { wrong.minimum.journal_revision += 1; Error::Stale }
            2 => { wrong.minimum.control_sequence += 1; Error::Stale }
            3 => { wrong.minimum.authority_epoch += 1; Error::Stale }
            4 => { wrong.credential_epoch.as_mut().unwrap().generation = 1; Error::Binding }
            5 => { wrong.credential_epoch.as_mut().unwrap().revoked = true; Error::Binding }
            _ => { wrong.guards.stream = None; Error::Binding }
        };
        let mut run = FileOversight::begin_open_guarded_anchored_with_learned_generation(
            root.store(), restart_profile(), &wrong, &recipe, &anchor).unwrap();
        assert_eq!(run.advance(0, run.progress().total_events).err(), Some(error.into()));
        assert_eq!(run.finish_pending_stream_reset(&intent, first).err(), Some(error.into()));
        assert_eq!(root.files(), bytes);
    }
    let mut run = FileOversight::begin_open_guarded_anchored_with_learned_generation(
        root.store(), restart_profile(), &expected, &recipe, &anchor).unwrap();
    run.advance(0, run.progress().total_events).unwrap();
    assert_eq!(run.credential_status().unwrap(), credential);
    let (host, roles) = run.finish_pending_stream_reset(&intent, first).unwrap();
    assert_eq!(host.credential_status().unwrap(), credential);
    assert_eq!(host.current_policy().unwrap(), &expected.effective_policy);
    assert!(host.inspect().control.sequence > expected.minimum.control_sequence);
    assert!(host.inspect().control.ledger.epoch > expected.minimum.authority_epoch);
    assert_eq!(host.revision(), expected.minimum.journal_revision + 2);
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    assert!(roles.identity_observer.is_none() && roles.policy_governor.is_none());
    assert!(!host.clock_ready()); assert!(host.learned_generation_inspection().unwrap().paused);
}
