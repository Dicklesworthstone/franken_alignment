//! Source-only actor intake, using the original numerical owner and real journals.
use super::*;
use crate::action::consequence::delivery::persistent::JournalIo;
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileActorSupervisor,
};
use crate::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
};
use crate::action::consequence::oversight::actor::{ActorError, ActorOutcome, Knowledge};

fn native(root: &Directory, p: FileOversightProfile, c: &FileDecoderConfig)
    -> (FileOversight, FileHumanReviewer)
{
    let (mut host, human) = FileOversight::create_generated_text_stream(root.store(), p,
        stream(), c.clone(), tokenizer(false)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, human)
}
fn ready(root: &Directory) -> (FileOversight, FileHumanReviewer) {
    let (mut host, human) = native(root, profile(), &fixtures::stopped_config(65));
    generate(&mut host, 7, request(b"ab", 2)); (host, human)
}
fn observed(supervisor: &mut FileActorSupervisor<FileOversight>) {
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
}
fn requirements() -> FileRecoveryRequirements {
    FileRecoveryRequirements { guards: FileGuardSet { stream: Some(stream()),
        decoder: Some(fixtures::stopped_config(65)), decoder_stop: None, source: None,
        identity: None, campaigns: None, credential: None },
        effective_policy: profile().delivery.policy, credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: 0, control_sequence: 0, authority_epoch: 0 } }
}
fn resume(host: &mut FileOversight) {
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    host.resume_decoder(host.revision(), n.actor_revision, n.position).unwrap();
}

#[test]
fn generated_actor_admission_matches_the_original_source_linked_reducer() {
    let root = Directory::new(); let control_root = Directory::new();
    let (host, _) = ready(&root); let (mut control, _) = ready(&control_root);
    let input = submission(&host, 91, 7);
    let numerical = host.decoder_inspection().unwrap().numerical;
    let revision = host.revision();
    let (port, mut supervisor) = host.into_generated_text_actor_gateway().unwrap();
    assert_eq!(port.profile(), stream());
    assert_eq!(port.submit(&input).unwrap_err(), ActorError::Unavailable);
    observed(&mut supervisor);
    let ticket = port.submit(&input).unwrap();
    let expected = control.submit_decoder_text_message(control.revision(), input.clone(), snapshot()).unwrap();
    let host = supervisor.host().unwrap();
    assert_eq!(host.request_status(91).unwrap(), expected);
    assert_eq!(host.request_action(91).unwrap(), control.request_action(91).unwrap());
    assert_eq!(host.decoder_text_message_request(91).unwrap(), &input);
    assert_eq!(host.inspect().control, control.inspect().control);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert_eq!(host.revision(), revision + 1);
    assert_eq!(host.stream_snapshot().unwrap().published.message_count(), 0);
    assert_eq!(port.poll(&ticket), Knowledge::Pending { request: 91 });
    assert_eq!(port.clone().poll(&ticket), port.poll(&ticket));
}

#[test]
fn generated_actor_snapshots_are_one_use_but_exact_retries_and_conflicts_do_not_consume_them() {
    let root = Directory::new(); let (host, _) = ready(&root);
    let input = submission(&host, 91, 7);
    let (port, mut supervisor) = host.into_generated_text_actor_gateway().unwrap();
    observed(&mut supervisor);
    for which in 0..3 {
        let mut malformed = input.clone();
        match which { 0 => malformed.request = 0, 1 => malformed.generation = 0,
            _ => malformed.generation_revision = u64::MAX }
        assert!(matches!(port.submit(&malformed), Err(ActorError::MalformedProposal | ActorError::Capacity)));
    }
    let mut missing = input.clone(); missing.generation = 999;
    let before = bytes(&supervisor.host().unwrap());
    assert_eq!(port.submit(&missing).unwrap_err(), ActorError::Unavailable);
    assert_eq!(bytes(&supervisor.host().unwrap()), before);
    assert_eq!(port.submit(&input).unwrap_err(), ActorError::Unavailable); // observation consumed
    observed(&mut supervisor);
    let ticket = port.submit(&input).unwrap();
    port.cancel(&ticket).unwrap();
    observed(&mut supervisor);
    let before = bytes(&supervisor.host().unwrap());
    let retry = port.submit(&input).unwrap();
    assert!(matches!(port.poll(&retry), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    for which in 0..5 {
        let mut changed = input.clone();
        match which { 0 => changed.generation += 1, 1 => changed.generation_revision += 1,
            2 => changed.target.object += 1, 3 => changed.policy_epoch += 1,
            _ => changed.deadline.0 += 1 }
        assert_eq!(port.submit(&changed).unwrap_err(), ActorError::IdempotencyConflict);
    }
    assert_eq!(bytes(&supervisor.host().unwrap()), before);
    let mut next = input.clone(); next.request = 92;
    let ticket = port.submit(&next).unwrap(); // still has the independently installed observation
    assert_eq!(port.poll(&ticket), Knowledge::Pending { request: 92 });
    let mut third = input; third.request = 93;
    assert_eq!(port.submit(&third).unwrap_err(), ActorError::Unavailable);
}

#[test]
fn generated_actor_does_not_upgrade_incomplete_cancelled_or_held_output() {
    for mode in 0..4 {
        let root = Directory::new();
        let c = if mode == 3 { fixtures::config(0.5, 65) } else { fixtures::stopped_config(65) };
        let (mut host, _) = native(&root, profile(), &c);
        let cmd = command(&host, 7, request(b"ab", 2));
        if mode == 3 { host.generate_decoder_text(host.revision(), cmd).unwrap(); }
        else {
            host.begin_decoder_text(host.revision(), cmd).unwrap();
            if mode != 0 { host.advance_decoder_text_batch(host.revision(), 7, 0, 3).unwrap(); }
            if mode == 2 { host.cancel_decoder_text(host.revision(), 7, 3).unwrap(); resume(&mut host); }
        }
        let source = submission(&host, 91, 7); let before = bytes(&host);
        let n = host.decoder_inspection().unwrap().numerical;
        let (port, mut supervisor) = host.into_generated_text_actor_gateway().unwrap();
        observed(&mut supervisor);
        assert_eq!(port.submit(&source).unwrap_err(), ActorError::Unavailable);
        let host = supervisor.host().unwrap();
        assert_eq!(bytes(&host), before); assert_eq!(host.decoder_inspection().unwrap().numerical, n);
        assert_eq!(host.inspect().executions, 0);
        assert!(matches!(host.request_status(91), Err(JournalError::Contract(Error::Missing))));
    }
    let root = Directory::new(); let (host, _) = ready(&root);
    let source = submission(&host, 91, 7);
    let (port, mut supervisor) = host.into_generated_text_actor_gateway().unwrap();
    observed(&mut supervisor); assert!(port.submit(&source).is_ok());
}

#[test]
fn generated_actor_returns_recorded_refusal_without_disclosing_policy_or_consuming_a_retry_snapshot() {
    let root = Directory::new(); let mut p = profile();
    p.delivery.policy = Policy::new(1, vec![Predicate::PayloadAtMost(1)]).unwrap();
    let (mut host, _) = native(&root, p, &fixtures::stopped_config(65));
    generate(&mut host, 7, request(b"ab", 2));
    let input = submission(&host, 91, 7);
    let (port, mut supervisor) = host.into_generated_text_actor_gateway().unwrap();
    observed(&mut supervisor); let ticket = port.submit(&input).unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::NotAdmitted, .. }));
    assert_eq!(supervisor.host().unwrap().decoder_text_message_request(91).unwrap(), &input);
    let before = bytes(&supervisor.host().unwrap());
    assert_eq!(port.poll(&port.submit(&input).unwrap()), port.poll(&ticket));
    assert_eq!(bytes(&supervisor.host().unwrap()), before);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
}

#[test]
fn generated_actor_tickets_rejoin_the_original_two_key_driver_and_conservative_cancellation() {
    let root = Directory::new(); let (host, human) = ready(&root);
    let source = submission(&host, 91, 7); let n = host.decoder_inspection().unwrap().numerical;
    let (port, mut driver) = host.into_generated_text_supervised_driver().unwrap();
    observed(driver.supervisor_mut()); let ticket = port.submit(&source).unwrap();
    let keys = { let mut host = driver.supervisor_mut().host_mut().unwrap(); review(&mut host, &human, 91) };
    {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        host.dispatch(revision, &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).unwrap();
    }
    let charged = driver.supervisor().host().unwrap().inspect().control.ledger.charged;
    port.cancel(&ticket).unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Unknown { .. }));
    assert_eq!(driver.supervisor().host().unwrap().inspect().control.ledger.charged, charged);
    {
        let mut host = driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
        host.publish_checked(revision, keys.automatic.attempt(), Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
        assert_eq!(host.stream_snapshot().unwrap().published.messages().collect::<Vec<_>>(), ["A"]);
        assert_eq!(host.stream_snapshot().unwrap().confirmed.message_count(), 0);
    }
    assert!(matches!(port.poll(&ticket), Knowledge::Unknown { .. }));
    port.cancel(&ticket).unwrap();
    {
        let mut host = driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
        host.reconcile(revision, keys.automatic.attempt()).unwrap();
        assert_eq!(host.inspect().executions, 1); assert_eq!(host.decoder_inspection().unwrap().numerical, n);
    }
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert_eq!(port.poll(&port.submit(&source).unwrap()), port.poll(&ticket));
}

#[test]
fn generated_actor_finish_keeps_the_original_frame_and_cannot_be_relabelled_as_native_text() {
    let root = Directory::new(); let (host, _) = ready(&root);
    let source = submission(&host, 91, 7);
    let expected = host.stream_finish_spec(source.deadline).unwrap();
    let (port, mut supervisor) = host.into_generated_text_actor_gateway().unwrap();
    observed(&mut supervisor);
    let ticket = port.finish(91, source.target, source.policy_epoch, source.deadline).unwrap();
    assert_eq!(supervisor.host().unwrap().request_action(91).unwrap().spec(), &expected);
    assert!(ReleaseFrame::decode(&expected.payload).unwrap().is_finish());
    assert!(!supervisor.host().unwrap().stream_snapshot().unwrap().published.finished());
    observed(&mut supervisor);
    assert_eq!(port.submit(&source).unwrap_err(), ActorError::IdempotencyConflict);
    assert_eq!(port.poll(&port.finish(91, source.target, source.policy_epoch, source.deadline).unwrap()), port.poll(&ticket));
    // The finish request is not native-message provenance; no generation is erased.
    assert!(matches!(supervisor.host().unwrap().decoder_text_message_request(91), Err(JournalError::Contract(Error::Missing))));
    port.cancel(&ticket).unwrap();
    observed(&mut supervisor);
    let mut next = source.clone(); next.request = 92;
    let ticket = port.submit(&next).unwrap();
    assert_eq!(port.finish(92, source.target, source.policy_epoch, source.deadline).unwrap_err(), ActorError::IdempotencyConflict);
    assert_eq!(port.poll(&ticket), Knowledge::Pending { request: 92 });
}

#[test]
fn generated_actor_recovery_reacquires_exact_tickets_without_restoring_clock_or_old_gateway() {
    let root = Directory::new(); let (host, _) = ready(&root);
    let source = submission(&host, 91, 7);
    let (port, mut supervisor) = host.into_generated_text_actor_gateway().unwrap();
    observed(&mut supervisor); let old_ticket = port.submit(&source).unwrap();
    let anchor = supervisor.host().unwrap().history_anchor().unwrap();
    drop(supervisor);
    assert!(matches!(port.poll(&old_ticket), Knowledge::Unknown { .. }));
    assert_eq!(port.submit(&source).unwrap_err(), ActorError::Unavailable);
    let (host, _) = FileOversight::open_generated_text_stream_anchored(root.store(), profile(),
        &requirements(), &tokenizer(false), &anchor).unwrap();
    assert!(!host.clock_ready()); assert!(host.decoder_inspection().unwrap().paused);
    let before = bytes(&host);
    let (fresh, supervisor) = host.into_generated_text_actor_gateway().unwrap();
    assert!(matches!(fresh.poll(&old_ticket), Knowledge::Withheld { .. }));
    assert_eq!(fresh.cancel(&old_ticket), Err(ActorError::Withheld));
    let ticket = fresh.submit(&source).unwrap();
    assert!(matches!(fresh.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    assert_eq!(bytes(&supervisor.host().unwrap()), before);
    assert!(!supervisor.host().unwrap().clock_ready());
    assert!(supervisor.host().unwrap().decoder_inspection().unwrap().paused);
    // No conversion of a valid legacy owner into the stronger source-only port.
    let other = Directory::new(); let (legacy, _) = start(&other, profile(), &fixtures::stopped_config(65));
    assert!(matches!(legacy.into_generated_text_actor_gateway(), Err(JournalError::Contract(Error::Binding))));
}

#[test]
fn generated_actor_storage_failures_expose_no_ticket_and_recover_only_the_canonical_request() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
        JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let (host, _) = ready(&root);
        let source = submission(&host, 91, 7); let anchor = host.history_anchor().unwrap();
        let before = host.inspect(); let n = host.decoder_inspection().unwrap().numerical;
        host.store.fail_once(barrier);
        let (port, mut supervisor) = host.into_generated_text_actor_gateway().unwrap();
        observed(&mut supervisor);
        assert_eq!(port.submit(&source).unwrap_err(), ActorError::Unavailable);
        assert_eq!(supervisor.host().unwrap().inspect(), before);
        assert_eq!(supervisor.host().unwrap().storage_failure().unwrap().operation, barrier);
        assert_eq!(port.submit(&source).unwrap_err(), ActorError::Unavailable);
        drop(supervisor);
        let (mut recovered, _) = FileOversight::open_generated_text_stream_anchored(root.store(), profile(),
            &requirements(), &tokenizer(false), &anchor).unwrap();
        assert_eq!(recovered.decoder_inspection().unwrap().numerical, n);
        if barrier == JournalIo::DirectorySync {
            let (fresh, owner) = recovered.into_generated_text_actor_gateway().unwrap();
            let ticket = fresh.submit(&source).unwrap();
            assert!(matches!(fresh.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
            assert_eq!(owner.host().unwrap().inspect().executions, 0);
        } else {
            assert!(matches!(recovered.request_status(91), Err(JournalError::Contract(Error::Missing))));
            resume(&mut recovered);
            let mut renewed = source.clone(); renewed.policy_epoch = recovered.inspect().control.ledger.epoch;
            let (fresh, mut owner) = recovered.into_generated_text_actor_gateway().unwrap();
            observed(&mut owner);
            assert_eq!(fresh.poll(&fresh.submit(&renewed).unwrap()), Knowledge::Pending { request: 91 });
            assert_eq!(owner.host().unwrap().decoder_inspection().unwrap().numerical, n);
            assert_eq!(owner.host().unwrap().inspect().executions, 0);
        }
    }
}

mod wire;
