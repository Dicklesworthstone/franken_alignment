//! Fixed original multi-event publication cuts retain their single storage boundary.
use crate::action::consequence::delivery::persistent::observed::decoder::learned::{
    FileLearnedConfig, FileHumanReviewer, FileOversightProfile, GenerationEvent,
    JournalIo, bind_history, journal,
};
use crate::action::consequence::delivery::persistent::observed::{Event,
    publication::CheckedCompletion};
use crate::action::consequence::delivery::persistent::{Event as BaseEvent, Reconciliation};
use crate::action::consequence::delivery::EndpointOutcome;
use crate::action::ActionState;
// Use the original numerical and authority fixture without copying its engines.
include!("../../../tests/fixture.rs");

fn prefix(host: &FileOversight) -> Option<usize> {
    host.learned_replay.as_ref().map(FileLearnedReplayContinuation::verified_events)
}
fn bytes(host: &FileOversight) -> Vec<u8> {
    host.store.read(host.profile.delivery.limits.bytes).unwrap()
}
fn current_predecessor(host: &FileOversight) {
    assert_eq!(prefix(host), Some(host.events.len() - 1));
}
fn original(host: &FileOversight, config: &FileLearnedConfig) {
    let cold = read_machine(host, config);
    assert_eq!(cold.snapshot(host.events.len()), host.inspect());
    assert_eq!(cold.broker.hosted_learned_generation().unwrap(),
        host.learned_generation_inspection().unwrap().numerical);
    assert_eq!(cold.broker.retained_actor_state().cache(), host.machine.broker.retained_actor_state().cache());
    assert_eq!(cold.broker.retained_actor_state().sampler(), host.machine.broker.retained_actor_state().sampler());
}

#[test]
fn atomic_publication_matches_the_cold_original_four_event_cut_and_replays_its_whole_tail() {
    let root = Directory::new(); let config = config(false, 1); let (mut host, reviewer) = owner(&root, &config);
    step(&mut host).unwrap(); let (action, inputs, automatic, request) = prepared(&mut host);
    let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
    current_predecessor(&host);
    let before = host.events.len(); let input_revision = host.input_revision(1).unwrap();
    let events = [Event::Core(BaseEvent::Time(ElapsedTick(2))),
        Event::Dispatch(1, 1001, input_revision, snapshot()),
        Event::PublishChecked(1, Some(inputs.views().clone()), snapshot(), ElapsedTick(2)),
        Event::Core(BaseEvent::Reconcile(1))];
    let mut expected_history = host.events.clone(); expected_history.extend(events.iter().cloned());
    let cold = Machine::replay(&host.profile, &expected_history).unwrap();
    let published = host.complete_checked_publication(host.revision(), CheckedCompletion {
        automatic: &automatic, human: &human, action: &action, current: &inputs,
        snapshot: snapshot(), now: ElapsedTick(2),
    }).unwrap();
    assert_eq!(published.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.events.len(), before + 4);
    assert_eq!(host.inspect(), cold.snapshot(expected_history.len()));
    assert_eq!(bytes(&host), journal::encode(&host.profile, host.store.identity(), &expected_history).unwrap());
    assert_eq!(prefix(&host), Some(before), "a batch predecessor must not be mislabeled as its penultimate event");
    let n = host.learned_generation_inspection().unwrap().numerical;
    let mut task = host.cached_learned_intent(host.revision(), n.actor_revision, n.position).unwrap();
    assert_eq!(task.progress().replayed_events, before);
    assert_eq!(task.progress().total_events - task.progress().replayed_events, 4);
    task.advance(&host, before, 1).unwrap();
    let disk = bytes(&host);
    assert_eq!(task.finish(&mut host).err(), Some(Error::Incomplete.into()));
    assert_eq!(bytes(&host), disk); assert_eq!(host.inspect().executions, 1);
    host.observe_time(host.revision(), ElapsedTick(3)).unwrap();
    assert_eq!(prefix(&host), None, "discarding a preparation keeps the existing cold-owner behavior");
    step(&mut host).unwrap(); current_predecessor(&host);
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().control.ledger.charged, 16);
    original(&host, &config);
}

#[test]
fn atomic_publication_faults_recover_old_or_complete_history_without_partial_keys_or_resends() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = config(false, 1); let (mut host, reviewer) = owner(&root, &config);
        step(&mut host).unwrap(); let (action, inputs, automatic, request) = prepared(&mut host);
        let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
        let before = host.inspect(); host.store.fail_once(barrier);
        assert!(matches!(host.complete_checked_publication(host.revision(), CheckedCompletion {
            automatic: &automatic, human: &human, action: &action, current: &inputs,
            snapshot: snapshot(), now: ElapsedTick(2),
        }), Err(JournalError::Io(failure)) if failure.operation == barrier));
        assert_eq!(host.inspect(), before); assert_eq!(prefix(&host), None);
        assert!(host.storage_failure().is_some()); drop(host);
        let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        assert_eq!(prefix(&host), None); assert!(host.learned_generation_inspection().unwrap().paused);
        let recovered = host.inspect();
        match recovered.executions {
            0 => {
                assert_eq!(recovered.payload, b"initial");
                assert_eq!(recovered.control.ledger.available, 100);
                assert_eq!(recovered.control.ledger.stages[&1], ActionState::Cancelled);
            }
            1 => {
                assert_eq!(recovered.payload, b"visible");
                assert_eq!(recovered.control.ledger.charged, 16);
                assert_eq!(recovered.control.ledger.stages[&1], ActionState::Confirmed);
            }
            _ => panic!("single original batch cannot execute twice"),
        }
        assert!(host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).is_err());
        assert_eq!(host.inspect(), recovered);
    }
}

#[test]
fn query_only_batch_preserves_interrupted_source_and_unknown_effect_charge() {
    let root = Directory::new(); let config = config(false, 1); let (mut host, reviewer) = owner(&root, &config);
    step(&mut host).unwrap(); let (action, inputs, automatic, request) = prepared(&mut host);
    let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    host.cancel(host.revision(), 1).unwrap(); current_predecessor(&host);
    host.source_interrupted = true;
    let before = host.events.len();
    host.reconcile_publications_at(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.events.len(), before + 2); assert_eq!(prefix(&host), Some(before));
    assert!(host.source_interrupted); assert_eq!(host.inspect().control.ledger.available, 84);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
    assert!(host.source_interrupted); current_predecessor(&host);
    let n = host.learned_generation_inspection().unwrap().numerical;
    assert!(host.begin_learned_step(host.revision(), n.actor_revision, n.position).is_err());
    assert!(host.source_interrupted); assert_eq!(host.inspect().control.ledger.available, 84);
}

#[test]
fn foreign_human_key_is_rejected_before_consuming_the_original_replay_cache() {
    let root = Directory::new(); let other = Directory::new(); let config = config(false, 1);
    let (mut host, _) = owner(&root, &config); let (mut foreign, reviewer) = owner(&other, &config);
    step(&mut host).unwrap(); step(&mut foreign).unwrap();
    let (action, inputs, automatic, _) = prepared(&mut host);
    let (_, _, _, request) = prepared(&mut foreign);
    let revision = foreign.revision(); let human = reviewer.approve(&mut foreign, revision, &request).unwrap();
    let before = host.inspect(); let disk = bytes(&host); let cache = prefix(&host);
    assert_eq!(host.complete_checked_publication(host.revision(), CheckedCompletion {
        automatic: &automatic, human: &human, action: &action, current: &inputs,
        snapshot: snapshot(), now: ElapsedTick(2),
    }).err(), Some(Error::Binding.into()));
    assert_eq!(host.inspect(), before); assert_eq!(bytes(&host), disk); assert_eq!(prefix(&host), cache);
    assert!(host.storage_failure().is_none());
}
