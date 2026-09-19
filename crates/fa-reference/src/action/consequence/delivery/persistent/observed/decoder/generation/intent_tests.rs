//! Interrupt the original Store, never a replacement mock journal or sampler.
use super::*;
use crate::action::{ActionSpec, VERSION};
use crate::action::consequence::delivery::persistent::{JournalIo, observed::containment::FileResetRequest};
use crate::action::consequence::delivery::persistent::observed::decoder::generation::{
    inspection::FileGenerationState, MAX_FILE_GENERATION_STEPS,
};
use crate::action::consequence::delivery::StopRequest;
use crate::action::consequence::gate::ReviewBinding;
use crate::Snapshot;

const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
    JournalIo::Rename, JournalIo::DirectorySync];

fn forced_prefix(host: &mut FileOversight) {
    let n = host.decoder_inspection().unwrap().numerical;
    assert!(matches!(host.advance_decoder_forced(host.revision(), n.actor_revision, n.position,
        0, budget()).unwrap().unwrap(), MonitoredStep::Released(_)));
}
fn fault(error: JournalError, stage: JournalIo) {
    let JournalError::Io(failure) = error else { panic!("expected original Store failure"); };
    assert_eq!(failure.operation, stage);
    assert_eq!(failure.replacement_may_be_visible, matches!(stage, JournalIo::Rename | JournalIo::DirectorySync));
}
fn canonical(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join(storage::CANONICAL)).unwrap()
}

#[test]
fn intent_is_durable_without_computation_and_cannot_be_replaced_or_bypassed() {
    let root = Directory::new(); let mut host = owner(&root, &config(3.0));
    forced_prefix(&mut host);
    let n = host.decoder_inspection().unwrap().numerical;
    let c = command(&host, 7, request(&[], 1));
    let revision = host.revision();
    host.begin_decoder_generation(revision, c.clone()).unwrap();
    assert_eq!(host.revision(), revision + 1);
    assert_eq!(host.decoder_inspection().unwrap().numerical, n);
    assert_eq!(host.pending_decoder_generation().unwrap().as_ref(), Some(&c));
    assert!(matches!(host.decoder_generation(7), Err(JournalError::Contract(Error::Incomplete))));
    let before = host.inspect(); let bytes = canonical(&root);
    host.begin_decoder_generation(0, c.clone()).unwrap();
    let alternate = command(&host, 8, request(&[1], 0));
    assert!(matches!(host.begin_decoder_generation(host.revision(), alternate.clone()), Err(JournalError::Contract(Error::WrongState))));
    assert!(matches!(host.generate_decoder(host.revision(), alternate), Err(JournalError::Contract(Error::WrongState))));
    let mut changed = c.request().clone(); changed.budget.sampling_entries -= 1;
    let changed = FileGenerationCommand::new(7, c.actor_revision(), c.position(), changed).unwrap();
    assert!(matches!(host.begin_decoder_generation(host.revision(), changed), Err(JournalError::Contract(Error::Binding))));
    assert!(matches!(host.advance_decoder_forced(host.revision(), n.actor_revision, n.position, 1, budget()),
        Err(JournalError::Contract(Error::Incomplete))));
    assert!(matches!(host.advance_decoder_sampled(host.revision(), n.actor_revision, n.position,
        SampleBudget { decoder: budget(), sampling: SamplingBudget { vocabulary: 2 } }),
        Err(JournalError::Contract(Error::Incomplete))));
    let spec = ActionSpec { version: VERSION, scope: host_profile().delivery.scope,
        target: Some(host.inspect().target), payload: b"proposed".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(20), units: 16 };
    let snapshot = Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() };
    assert!(matches!(host.propose(host.revision(), 1, spec.clone(), snapshot.clone()),
        Err(JournalError::Contract(Error::Incomplete))));
    assert_eq!(host.inspect(), before); assert_eq!(canonical(&root), bytes);
    assert!(host.storage_failure().is_none());
    assert_eq!(host.decoder_inspection().unwrap().numerical, n);
    assert_eq!(host.generate_decoder(host.revision(), c).unwrap().result().unwrap().tokens().len(), 1);
    assert!(host.pending_decoder_generation().unwrap().is_none());
    assert!(host.propose(host.revision(), 1, spec, snapshot).is_ok());
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn checkpoint_reset_cannot_erase_an_intent_but_original_manual_suspension_remains_available() {
    let root = Directory::new(); let mut host = owner(&root, &config(3.0));
    forced_prefix(&mut host);
    let n = host.decoder_inspection().unwrap().numerical;
    let epoch = host.inspect().control.ledger.epoch;
    let checkpoint = host.capture_decoder_checkpoint(host.revision(), 1, n.actor_revision, epoch).unwrap();
    let c = command(&host, 7, request(&[], 1));
    host.begin_decoder_generation(host.revision(), c.clone()).unwrap();
    let before = host.inspect();
    let reset = FileResetRequest { operation: 1, expected_control_sequence: before.control.sequence,
        expected_actor_revision: n.actor_revision, expected_authority_epoch: epoch,
        binding: ReviewBinding { round: 5001, evidence_root: [17; 32], reducer_generation: 1 },
        retained_targets: vec![before.target] };
    assert!(matches!(host.reset_decoder_checkpoint(host.revision(), &checkpoint, reset, budget()),
        Err(JournalError::Contract(Error::Incomplete))));
    assert!(matches!(host.capture_decoder_checkpoint(host.revision(), 2, n.actor_revision, epoch),
        Err(JournalError::Contract(Error::Incomplete))));
    assert_eq!(host.inspect(), before); assert_eq!(host.decoder_inspection().unwrap().numerical, n);
    assert!(host.storage_failure().is_none());
    let request = StopRequest { operation: 901, expected_control_sequence: before.control.sequence,
        expected_authority_epoch: epoch };
    host.transact(host.revision(), Event::Core(BaseEvent::Stop(request))).unwrap();
    assert!(host.inspect().control.suspended);
    assert_eq!(host.pending_decoder_generation().unwrap().as_ref(), Some(&c));
    assert!(matches!(host.generate_decoder(host.revision(), c), Err(JournalError::Contract(Error::Incomplete))));
    assert_eq!(host.decoder_inspection().unwrap().numerical, n);
}

#[test]
fn intent_survives_a_crash_and_resume_never_unfreezes_its_command() {
    let root = Directory::new(); let configuration = config(3.0); let mut host = owner(&root, &configuration);
    let c = command(&host, 7, request(&[0], 1));
    host.begin_decoder_generation(host.revision(), c.clone()).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    drop(host);
    let (mut host, _) = FileOversight::open_with_decoder(root.store(), host_profile(), &configuration).unwrap();
    assert!(host.decoder_inspection().unwrap().paused); assert!(!host.clock_ready());
    assert_eq!(host.pending_decoder_generation().unwrap().as_ref(), Some(&c));
    assert!(matches!(host.decoder_generation(7), Err(JournalError::Contract(Error::Incomplete))));
    assert!(matches!(host.generate_decoder(host.revision(), c.clone()), Err(JournalError::Contract(Error::Incomplete))));
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    host.resume_decoder(host.revision(), n.actor_revision, n.position).unwrap();
    let before = host.inspect();
    let alternate = command(&host, 8, request(&[1], 1));
    assert!(matches!(host.generate_decoder(host.revision(), alternate), Err(JournalError::Contract(Error::WrongState))));
    assert!(matches!(host.advance_decoder_forced(host.revision(), n.actor_revision, n.position, 1, budget()),
        Err(JournalError::Contract(Error::Incomplete))));
    assert_eq!(host.inspect(), before);
    let report = host.generate_decoder(host.revision(), c).unwrap();
    assert_eq!(report.result().unwrap().finish(), GenerationFinish::TokenLimit);
    assert_eq!(host.revision(), before.revision + 1); // The intent is not written twice.
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 1);
    assert!(host.pending_decoder_generation().unwrap().is_none());
}

#[test]
fn result_write_faults_leave_only_the_frozen_intent_or_the_complete_canonical_result() {
    for threshold in [3.0, 1.5] {
        for stage in BARRIERS {
            let root = Directory::new(); let configuration = config(threshold); let mut host = owner(&root, &configuration);
            host.enable_decoder_stop(host.revision(), HostedStopPolicy::new(7, 1, 900).unwrap()).unwrap();
            let c = command(&host, 7, request(&[0], 1));
            host.begin_decoder_generation(host.revision(), c.clone()).unwrap();
            let before = host.inspect(); let numerical = host.decoder_inspection().unwrap().numerical;
            host.store.fail_once(stage);
            fault(host.generate_decoder(host.revision(), c.clone()).unwrap_err(), stage);
            assert_eq!(host.inspect(), before);
            assert!(matches!(host.decoder_generation(7), Err(JournalError::Unavailable)));
            assert!(matches!(host.pending_decoder_generation(), Err(JournalError::Unavailable)));
            assert!(matches!(host.generate_decoder(host.revision(), c.clone()), Err(JournalError::Unavailable)));
            let bytes = canonical(&root);
            let disk = FileOversight::read_decoder_generation(root.store(), &host_profile(), &configuration, 7).unwrap();
            let completed = stage == JournalIo::DirectorySync;
            let finish = if threshold == 1.5 { GenerationFinish::Held } else { GenerationFinish::TokenLimit };
            match disk.generation {
                FileGenerationState::Recorded(receipt) => {
                    assert!(completed); assert_eq!(receipt.result().unwrap().finish(), finish);
                    assert_eq!(disk.numerical.numerical.sampled_draws, 1);
                    if finish == GenerationFinish::Held { assert!(receipt.result().unwrap().tokens().is_empty()); }
                }
                FileGenerationState::Pending(command) => {
                    assert!(!completed); assert_eq!(command, c);
                    assert_eq!(disk.numerical.numerical, numerical);
                }
            }
            assert_eq!(disk.publication.executions, 0); assert_eq!(disk.publication.payload, b"initial");
            assert_eq!(canonical(&root), bytes); // Inspection never repairs or cleans up.
            drop(host);
            let (mut host, _) = FileOversight::open_with_decoder(root.store(), host_profile(), &configuration).unwrap();
            let before = host.inspect();
            if completed {
                assert_eq!(host.generate_decoder(0, c.clone()).unwrap().result().unwrap().finish(), finish);
                assert_eq!(host.inspect(), before);
            } else {
                assert_eq!(host.pending_decoder_generation().unwrap().as_ref(), Some(&c));
                assert!(matches!(host.decoder_generation(7), Err(JournalError::Contract(Error::Incomplete))));
                host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
                host.resume_decoder(host.revision(), numerical.actor_revision, numerical.position).unwrap();
                let result = host.generate_decoder(host.revision(), c.clone()).unwrap();
                assert_eq!(result.result().unwrap().finish(), finish);
            }
            assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 1);
            if finish == GenerationFinish::Held {
                assert!(host.inspect().control.suspended);
                assert_eq!(host.decoder_stop_incident().unwrap().unwrap().cause(),
                    HostedStopCause::Monitoring(MonitorOutcome::Alarm));
            }
            assert!(host.pending_decoder_generation().unwrap().is_none());
        }
    }
}

#[test]
fn failed_intent_publication_never_enters_the_numerical_engine() {
    for stage in BARRIERS {
        let root = Directory::new(); let configuration = config(3.0); let mut host = owner(&root, &configuration);
        let c = command(&host, 7, request(&[0], 1));
        let before = host.inspect(); let numerical = host.decoder_inspection().unwrap().numerical;
        host.store.fail_once(stage);
        fault(host.begin_decoder_generation(host.revision(), c.clone()).unwrap_err(), stage);
        assert_eq!(host.inspect(), before);
        assert_eq!(host.machine.broker.hosted_decoder().unwrap(), numerical);
        let disk = FileOversight::read_decoder_generation(root.store(), &host_profile(), &configuration, 7);
        if stage == JournalIo::DirectorySync {
            let disk = disk.unwrap();
            assert!(matches!(disk.generation, FileGenerationState::Pending(ref original) if original == &c));
            assert_eq!(disk.numerical.numerical, numerical);
        } else { assert!(matches!(disk, Err(JournalError::Contract(Error::Missing)))); }
        assert!(matches!(host.generate_decoder(host.revision(), c), Err(JournalError::Unavailable)));
    }
}

#[test]
fn read_only_recovery_pins_configuration_and_rejects_a_bad_tail_instead_of_returning_a_prefix() {
    let root = Directory::new(); let configuration = config(3.0); let mut host = owner(&root, &configuration);
    let c = command(&host, 7, request(&[0], 1));
    host.generate_decoder(host.revision(), c).unwrap();
    let bytes = canonical(&root);
    let old = FileOversight::read_decoder_generation(root.store(), &host_profile(), &configuration, 7).unwrap();
    assert!(matches!(old.generation, FileGenerationState::Recorded(_)));
    assert!(matches!(FileOversight::read_decoder_generation(root.store(), &host_profile(), &config(1.5), 7),
        Err(JournalError::Contract(Error::Binding))));
    assert_eq!(canonical(&root), bytes);
    let mut events = host.events.clone(); events.push(host.events.last().unwrap().clone());
    let changed = journal::encode(&host_profile(), host.store.identity(), &events).unwrap();
    // This fixture deliberately publishes a structurally valid, semantically
    // duplicated tail. The reader must replay past the requested older receipt.
    host.store.replace(&changed).unwrap();
    assert!(matches!(FileOversight::read_decoder_generation(root.store(), &host_profile(), &configuration, 7),
        Err(JournalError::Contract(Error::Duplicate))));
    assert_eq!(canonical(&root), changed);
}

#[test]
fn complete_witness_binds_even_nonoperative_budget_and_request_id_changes() {
    let root = Directory::new(); let mut host = owner(&root, &config(3.0));
    let c = command(&host, 7, request(&[0], 1));
    host.generate_decoder(host.revision(), c.clone()).unwrap();
    let last = host.events.len() - 1;
    let Event::Decoder(DecoderEvent::Generate(_, witness)) = &host.events[last] else { panic!("complete generation"); };
    for change_id in [false, true] {
        let mut input = c.request().clone();
        if !change_id { input.budget.scalar_products -= 1; } // Still ample for exactly the same tokens.
        let changed = Rc::new(FileGenerationCommand::new(if change_id { 8 } else { 7 },
            c.actor_revision(), c.position(), input).unwrap());
        let mut events = host.events.clone();
        events[last - 1] = Event::Decoder(DecoderEvent::BeginGeneration(Rc::clone(&changed)));
        events[last] = Event::Decoder(DecoderEvent::Generate(changed, Rc::clone(witness)));
        assert!(matches!(Machine::replay(&host_profile(), &events), Err(Error::Binding)));
    }
}

#[test]
fn requested_step_capacity_is_reserved_once_and_is_not_refunded_by_native_refusal() {
    let root = Directory::new(); let mut host = owner(&root, &config(3.0));
    let n = host.decoder_inspection().unwrap().numerical;
    let horizon = 4096;
    let requests = MAX_FILE_GENERATION_STEPS / horizon;
    for id in 1..=requests as u64 {
        let c = command(&host, id, request(&[0], horizon - 1));
        host.begin_decoder_generation(host.revision(), c.clone()).unwrap();
        host.begin_decoder_generation(0, c.clone()).unwrap();
        // The tiny model's context refuses before inference. Intent+result still
        // consumes one declared horizon, not two, and not zero.
        let receipt = host.generate_decoder(host.revision(), c).unwrap();
        assert_eq!(receipt.result().unwrap_err(), Error::Limit);
    }
    let before = host.inspect();
    let c = command(&host, requests as u64 + 1, request(&[0], 0));
    assert!(matches!(host.begin_decoder_generation(host.revision(), c), Err(JournalError::Contract(Error::Limit))));
    assert_eq!(host.inspect(), before); assert_eq!(host.decoder_inspection().unwrap().numerical, n);
    assert!(host.pending_decoder_generation().unwrap().is_none());
}

#[test]
fn legacy_completed_generation_still_replays_but_live_runs_always_write_intent_first() {
    let root = Directory::new(); let mut host = owner(&root, &config(3.0));
    let c = command(&host, 7, request(&[0], 1));
    host.generate_decoder(host.revision(), c.clone()).unwrap();
    let events: Vec<_> = host.events.iter().filter(|event|
        !matches!(event, Event::Decoder(DecoderEvent::BeginGeneration(_)))).cloned().collect();
    let legacy = Machine::replay(&host_profile(), &events).unwrap();
    assert_eq!(legacy.recorded_decoder_generation(7).unwrap().command(), &c);
    assert_eq!(legacy.broker.hosted_replay_bytes().unwrap(), host.machine.broker.hosted_replay_bytes().unwrap());
    assert!(host.events.iter().any(|event| matches!(event, Event::Decoder(DecoderEvent::BeginGeneration(_)))));
}

#[test]
fn intent_admission_requires_room_for_a_result_and_refuses_stale_actor_predecessors() {
    let root = Directory::new(); let mut profile = host_profile(); profile.delivery.limits.events = 3;
    let (mut host, _) = FileOversight::create(root.store(), profile).unwrap();
    host.enable_decoder(host.revision(), config(3.0)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let before = host.inspect(); let n = host.decoder_inspection().unwrap().numerical;
    let c = command(&host, 7, request(&[0], 1));
    assert!(matches!(host.begin_decoder_generation(host.revision(), c), Err(JournalError::Contract(Error::Limit))));
    assert_eq!(host.inspect(), before); assert!(host.pending_decoder_generation().unwrap().is_none());
    assert!(host.storage_failure().is_none());
    let stale = FileGenerationCommand::new(7, n.actor_revision + 1, n.position, request(&[0], 1)).unwrap();
    assert!(matches!(host.begin_decoder_generation(host.revision(), stale), Err(JournalError::Contract(Error::Stale))));
    assert_eq!(host.inspect(), before); assert_eq!(host.decoder_inspection().unwrap().numerical, n);
}
