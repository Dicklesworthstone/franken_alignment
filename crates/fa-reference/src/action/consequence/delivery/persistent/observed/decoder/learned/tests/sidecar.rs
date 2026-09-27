//! Original numerical source, packet construction, journal and two-key endpoint.
mod refinement;
use super::*;
use super::super::sidecar::{FileLearnedSidecar, SidecarEvent};
use crate::action::consequence::oversight::{CommitteeInput, ReviewWindow,
    learned_host::sidecar::LearnedSidecarRequest,
    sidecar::{SidecarCongressBudget, SidecarIdentity}};
use crate::round::{Verdict, commitment};

fn request() -> LearnedSidecarRequest {
    LearnedSidecarRequest { identity: SidecarIdentity { object_id: 91, generation: 1, transform_id: 7 },
        priority: Vec::new(), budget: SidecarCongressBudget::default() }
}
fn proposed(host: &mut FileOversight) -> (crate::action::FrozenAction, FileLearnedSidecar) {
    step(host).unwrap();
    let action = host.propose(host.revision(), 1, action_spec(host), snapshot()).unwrap();
    let actor = host.learned_generation_inspection().unwrap().numerical.actor_revision;
    let sidecar = host.begin_learned_sidecar_plan(host.revision(), 1, actor, request()).unwrap();
    (action, sidecar)
}
fn allow(host: &mut FileOversight, inputs: &CommitteeInput) {
    host.begin_review(host.revision(), 1, 101, [9; 32],
        ReviewWindow { commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) }, snapshot()).unwrap();
    let digest = commitment(101, "reviewer", &[9; 32], Verdict::Allow, b"original salt").unwrap();
    host.commit_review(host.revision(), 101, "reviewer", digest).unwrap();
    host.open_reveals(host.revision(), 101).unwrap();
    host.reveal_review(host.revision(), 101, "reviewer", Verdict::Allow, b"original salt".to_vec()).unwrap();
    host.finish_review(host.revision(), 101, Some(inputs), snapshot()).unwrap().unwrap();
}

#[test]
fn acknowledged_coarse_input_replays_original_source_and_supports_both_keys() {
    let root = Directory::new(); let config = config(false, 1).with_required_sidecar().unwrap();
    let (mut host, reviewer) = owner(&root, &config);
    assert!(host.learned_sidecar_required());
    let (action, sidecar) = proposed(&mut host);
    let inputs = host.current_learned_sidecar(&sidecar).unwrap().clone();
    let original = host.checked_learned_sidecar(&sidecar).unwrap();
    assert!(original.round().selected_groups().is_empty());
    assert_eq!(original.round().work().rounds, 1);
    let payload = original.round().payload().to_vec();
    assert!(!payload.is_empty());
    let replayed = read_machine(&host, &config);
    assert_eq!(replayed.checked_learned_sidecar(1).unwrap().round().payload(), payload);
    assert_eq!(replayed.checked_learned_sidecar(1).unwrap().round().input(), &inputs);
    assert_eq!(host.inspect().executions, 0);
    allow(&mut host, &inputs);
    let automatic = host.authorize(host.revision(), 1, &inputs, snapshot()).unwrap();
    let human_request = host.request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(10)).unwrap();
    let revision = host.revision();
    let human = reviewer.approve(&mut host, revision, &human_request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    let result = host.publish_checked(host.revision(), 1, Some(&inputs), snapshot(), ElapsedTick(2)).unwrap();
    assert!(matches!(result.outcome, EndpointOutcome::Executed { .. }));
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().payload, action.spec().payload);
}

#[test]
fn byte_identical_caller_input_cannot_install_original_sidecar_provenance() {
    let config = config(false, 1).with_required_sidecar().unwrap();
    let first_root = Directory::new(); let (mut first, _) = owner(&first_root, &config);
    let (_, handle) = proposed(&mut first);
    let input = first.current_learned_sidecar(&handle).unwrap().clone();
    let second_root = Directory::new(); let (mut second, _) = owner(&second_root, &config);
    step(&mut second).unwrap();
    let action = second.propose(second.revision(), 1, action_spec(&second), snapshot()).unwrap();
    assert_eq!(&action, input.action());
    second.record_inputs(second.revision(), 1, 0, input.clone()).unwrap();
    assert_eq!(second.machine.broker.current_inputs(1).unwrap(), Some(&input));
    let before = second.revision();
    assert_eq!(second.begin_review(before, 1, 101, [9; 32],
        ReviewWindow { commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) }, snapshot()).err(),
        Some(JournalError::Contract(Error::Incomplete)));
    assert_eq!(second.revision(), before);
    assert_eq!(second.authorize(before, 1, &input, snapshot()).err(), Some(JournalError::Contract(Error::Incomplete)));
    assert_eq!(second.inspect().executions, 0);
    // The otherwise identical original source-recording control completes review.
    allow(&mut first, &input);
    assert!(first.authorize(first.revision(), 1, &input, snapshot()).is_ok());
}

#[test]
fn source_progress_and_withdrawal_make_handles_stale_without_reopening_plans() {
    let config = config(false, 1).with_required_sidecar().unwrap();
    let root = Directory::new(); let (mut host, _) = owner(&root, &config);
    let (_, handle) = proposed(&mut host);
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
    assert_eq!(host.current_learned_sidecar(&handle).err(), Some(JournalError::Contract(Error::Incomplete)));
    host.complete_learned_step(host.revision(), n.actor_revision, n.position).unwrap().unwrap();
    assert_eq!(host.current_learned_sidecar(&handle).err(), Some(JournalError::Contract(Error::Stale)));
    assert_eq!(host.inspect().executions, 0);

    let root = Directory::new(); let (mut host, _) = owner(&root, &config);
    let (_, handle) = proposed(&mut host);
    let actor = handle.actor_revision();
    host.inputs_unavailable(host.revision(), 1, handle.input_revision()).unwrap();
    assert!(host.current_learned_sidecar(&handle).is_err());
    let before = host.revision();
    assert_eq!(host.begin_learned_sidecar_plan(before, 1, actor, request()).err(), Some(JournalError::Contract(Error::Duplicate)));
    assert_eq!(host.revision(), before);
}

#[test]
fn recovery_requires_exact_sidecar_recipe_and_never_accepts_the_old_handle() {
    let plain = config(false, 1);
    let legacy_bytes = plain.bytes().clone();
    let required = plain.clone().with_required_sidecar().unwrap();
    assert_eq!(plain.bytes().as_ref(), legacy_bytes.as_ref());
    assert!(!plain.requires_sidecar() && required.requires_sidecar());
    assert_ne!(plain, required);
    assert_eq!(required.clone().with_required_sidecar().err(), Some(Error::Duplicate));
    let root = Directory::new(); let (mut host, _) = owner(&root, &required);
    let (_, handle) = proposed(&mut host);
    let retained = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    drop(host);
    assert!(FileOversight::open_with_learned_generation(root.store(), profile(), &plain).is_err());
    assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), retained);
    let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &required).unwrap();
    assert!(host.learned_sidecar_required());
    assert_eq!(host.current_learned_sidecar(&handle).err(), Some(JournalError::Contract(Error::Binding)));
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
    assert_eq!(host.current_learned_sidecar(&handle).err(), Some(JournalError::Contract(Error::Binding)));
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn changed_packet_witness_refuses_replay_and_insufficient_budget_spends_no_input() {
    let config = config(false, 1).with_required_sidecar().unwrap();
    let root = Directory::new(); let (mut host, _) = owner(&root, &config);
    step(&mut host).unwrap();
    host.propose(host.revision(), 1, action_spec(&host), snapshot()).unwrap();
    let actor = host.learned_generation_inspection().unwrap().numerical.actor_revision;
    let mut invalid = request(); invalid.budget.committee_bytes = 1;
    let before = host.revision();
    assert_eq!(host.begin_learned_sidecar_plan(before, 1, actor, invalid).err(), Some(JournalError::Contract(Error::Limit)));
    assert_eq!(host.revision(), before);
    assert_eq!(host.input_revision(1).unwrap(), 0);
    let handle = host.begin_learned_sidecar_plan(before, 1, actor, request()).unwrap();
    assert!(host.current_learned_sidecar(&handle).is_ok());
    let mut events = host.events.clone();
    let Event::Decoder(DecoderEvent::Learned(LearnedEvent::Sidecar(SidecarEvent::Begin { expected_payload, .. })))
        = events.last_mut().unwrap() else { panic!("original sidecar event"); };
    let mut bytes = expected_payload.to_vec(); let end = bytes.len() - 1; bytes[end] ^= 1;
    *expected_payload = bytes.into();
    assert_eq!(Machine::replay(&host.profile, &events).err(), Some(Error::Binding));
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn original_storage_barriers_return_no_unacknowledged_sidecar_handle() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = config(false, 1).with_required_sidecar().unwrap();
        let (mut host, _) = owner(&root, &config);
        step(&mut host).unwrap();
        host.propose(host.revision(), 1, action_spec(&host), snapshot()).unwrap();
        let actor = host.learned_generation_inspection().unwrap().numerical.actor_revision;
        let before = host.revision();
        host.store.fail_once(barrier);
        let error = host.begin_learned_sidecar_plan(before, 1, actor, request()).unwrap_err();
        let JournalError::Io(failure) = error else { panic!("original storage barrier"); };
        assert_eq!(failure.operation, barrier);
        assert_eq!(host.revision(), before);
        assert_eq!(host.input_revision(1).unwrap(), 0);
        assert_eq!(host.begin_learned_sidecar_plan(before, 1, actor, request()).err(), Some(JournalError::Unavailable));
        drop(host);
        let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        assert_eq!(host.input_revision(1).unwrap(), if barrier == JournalIo::DirectorySync { 1 } else { 0 });
        assert!(host.learned_sidecar_required());
        assert_eq!(host.inspect().executions, 0);
    }
}
