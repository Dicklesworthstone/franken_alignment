//! Original learned generation, original request ledger and actual journal files.
//! These synthetic controls establish no trained-detector or deployment claim.
use super::*;
use crate::action::{ElapsedTick, ResolvedTarget};
use crate::action::consequence::delivery::persistent::{JournalIo, requests::actor::{
    FileActorSupervisor, FileLearnedTextActorPort}};
use crate::action::consequence::delivery::persistent::observed::{FileOversightProfile, FileHumanReviewer,
    decoder::learned::FileLearnedConfig};
use crate::action::consequence::oversight::actor::{ActorError, ActorOutcome, Knowledge};
use crate::Snapshot;
mod fixture;
use fixture::*;

#[test]
fn source_only_request_captures_exact_generated_message_without_running_more_inference() {
    let root = Directory::new(); let config = config(); let (host, _) = owner(&root, &config);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    assert!(matches!(port.submit(71, proposal()), Err(ActorError::Unavailable)));
    observe(&mut supervisor, snapshot());
    let before = supervisor.host().unwrap().revision();
    let ticket = port.submit(71, proposal()).unwrap();
    let host = supervisor.host().unwrap();
    assert_eq!(host.request_action(71).unwrap().spec().payload, b"aa");
    assert_eq!(host.revision(), before + 1);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert!(matches!(port.poll(&ticket), Knowledge::Pending { request: 71 }));
    drop(host);
    let retry = port.submit(71, proposal()).unwrap();
    assert_eq!(port.poll(&retry), port.poll(&ticket));
    assert_eq!(supervisor.host().unwrap().revision(), before + 1);
    assert!(matches!(port.submit(72, proposal()), Err(ActorError::Unavailable)));
}

#[test]
fn retained_nonadmission_and_denial_are_not_retried_with_an_improved_snapshot() {
    for wrong_epoch in [false, true] {
        let root = Directory::new(); let (host, _) = owner(&root, &config());
        let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
        let mut supplied = snapshot(); let mut refused = proposal();
        if wrong_epoch { refused.expected_policy_epoch += 1; }
        else { supplied.values.insert(7, b"no".to_vec()); }
        observe(&mut supervisor, supplied);
        let ticket = port.submit(71, refused).unwrap();
        let expected = if wrong_epoch { ActorOutcome::NotAdmitted } else { ActorOutcome::Denied };
        assert!(matches!(port.poll(&ticket), Knowledge::Known { value, .. } if value == expected));
        observe(&mut supervisor, snapshot());
        let revision = supervisor.host().unwrap().revision();
        let retry = port.submit(71, refused).unwrap();
        assert_eq!(port.poll(&retry), port.poll(&ticket));
        assert_eq!(supervisor.host().unwrap().revision(), revision);
        // A retry did not consume the next NEW request's observation.
        let next = port.submit(72, proposal()).unwrap();
        assert!(matches!(port.poll(&next), Knowledge::Pending { request: 72 }));
        assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
    }
}

#[test]
fn changed_request_fields_conflict_without_consuming_a_new_admission_snapshot() {
    let root = Directory::new(); let (host, _) = owner(&root, &config());
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot()); port.submit(71, proposal()).unwrap();
    observe(&mut supervisor, snapshot());
    let revision = supervisor.host().unwrap().revision();
    for field in 0..4 {
        let mut changed = proposal();
        match field {
            0 => changed.target.object += 1,
            1 => changed.expected_policy_epoch += 1,
            2 => changed.deadline = ElapsedTick(99),
            _ => changed.units += 1,
        }
        assert!(matches!(port.submit(71, changed), Err(ActorError::IdempotencyConflict)));
    }
    assert_eq!(supervisor.host().unwrap().revision(), revision);
    assert!(matches!(port.poll(&port.submit(72, proposal()).unwrap()), Knowledge::Pending { .. }));
}

#[test]
fn partial_generation_is_not_a_short_message_and_admission_preserves_the_snapshot() {
    let root = Directory::new(); let config = config();
    let (mut host, _) = FileOversight::create_with_learned_text(root.store(), profile(), config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host); step(&mut host);
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot());
    let revision = supervisor.host().unwrap().revision();
    assert!(matches!(port.submit(71, proposal()), Err(ActorError::Unavailable)));
    assert_eq!(supervisor.host().unwrap().revision(), revision);
    // Derivation refused before the snapshot was consumed. Advancing via trusted
    // host_mut still invalidates it, so a fresh observation is required afterward.
    { let mut host = supervisor.host_mut().unwrap(); step(&mut host); }
    assert!(matches!(port.submit(71, proposal()), Err(ActorError::Unavailable)));
    observe(&mut supervisor, snapshot());
    assert!(matches!(port.poll(&port.submit(71, proposal()).unwrap()), Knowledge::Pending { .. }));
}

#[test]
fn cancellation_and_fenced_recovery_reacquire_only_the_historical_request() {
    let root = Directory::new(); let config = config(); let (host, _) = owner(&root, &config);
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot()); let old = port.submit(71, proposal()).unwrap();
    port.cancel(&old).unwrap();
    assert!(matches!(port.poll(&old), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    drop(supervisor);
    assert!(matches!(port.poll(&old), Knowledge::Unknown { .. }));
    let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert!(!host.clock_ready());
    let revision = host.revision();
    let (reopened, supervisor) = host.into_learned_text_actor_gateway().unwrap();
    let retry = reopened.submit(71, proposal()).unwrap();
    assert!(matches!(reopened.poll(&retry), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    assert!(matches!(reopened.poll(&old), Knowledge::Withheld { .. }));
    assert!(matches!(reopened.submit(72, proposal()), Err(ActorError::Unavailable)));
    assert_eq!(supervisor.host().unwrap().revision(), revision);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
}

#[test]
fn source_interruption_blocks_new_intake_but_not_historical_retries() {
    let root = Directory::new(); let config = config(); let (host, _) = owner(&root, &config);
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot()); let ticket = port.submit(71, proposal()).unwrap();
    { let mut host = supervisor.host_mut().unwrap(); host.source_interrupted = true; }
    observe(&mut supervisor, snapshot());
    assert!(matches!(port.submit(72, proposal()), Err(ActorError::Unavailable)));
    assert_eq!(port.poll(&port.submit(71, proposal()).unwrap()), port.poll(&ticket));
    // Same source and same numerical state, with its live latch restored, is
    // the paired positive; the failed capture did not spend the observation.
    { let mut host = supervisor.host_mut().unwrap(); host.source_interrupted = false; }
    observe(&mut supervisor, snapshot());
    assert!(matches!(port.poll(&port.submit(72, proposal()).unwrap()), Knowledge::Pending { .. }));
}

#[test]
fn every_request_storage_barrier_returns_no_ticket_and_fences_any_visible_admission() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = config(); let (host, _) = owner(&root, &config);
        let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
        observe(&mut supervisor, snapshot());
        supervisor.host().unwrap().store.fail_once(barrier);
        assert!(matches!(port.submit(71, proposal()), Err(ActorError::Unavailable)));
        assert!(supervisor.host().unwrap().storage_failure().is_some());
        assert!(matches!(port.submit(71, proposal()), Err(ActorError::Unavailable)));
        drop(supervisor);
        let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.available, 100);
        let recorded = host.request_status(71).is_ok();
        let revision = host.revision();
        let (port, supervisor) = host.into_learned_text_actor_gateway().unwrap();
        if recorded {
            let ticket = port.submit(71, proposal()).unwrap();
            assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
        } else { assert!(matches!(port.submit(71, proposal()), Err(ActorError::Unavailable))); }
        assert_eq!(supervisor.host().unwrap().revision(), revision);
    }
}

#[test]
fn invalid_key_or_insufficient_units_cannot_admit_generated_output() {
    let root = Directory::new(); let (host, _) = owner(&root, &config());
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot());
    assert!(matches!(port.submit(0, proposal()), Err(ActorError::MalformedProposal)));
    let mut small = proposal(); small.units = 1;
    assert!(matches!(port.submit(71, small), Err(ActorError::Capacity)));
    assert!(matches!(port.poll(&port.submit(71, proposal()).unwrap()), Knowledge::Pending { .. }));
}

#[test]
fn only_a_learned_text_profile_can_create_this_source_only_port() {
    let root = Directory::new(); let (host, _) = FileOversight::create(root.store(), profile()).unwrap();
    assert!(matches!(host.into_learned_text_actor_gateway(), Err(JournalError::Contract(Error::Binding))));
    let root = Directory::new(); let (host, _) = owner(&root, &config());
    let (port, mut supervisor): (FileLearnedTextActorPort, _) = host.into_learned_text_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot());
    assert!(matches!(port.poll(&port.submit(71, proposal()).unwrap()), Knowledge::Pending { .. }));
}
