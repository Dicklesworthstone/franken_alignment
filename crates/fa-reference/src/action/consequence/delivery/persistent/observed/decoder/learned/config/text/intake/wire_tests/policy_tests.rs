//! Real operator-file observations at the original source-only wire boundary.
//! Source files have no helper authority and cannot replace generated output.
use super::*;
use crate::action::consequence::oversight::evidence_source::{EvidenceError,
    EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use std::path::{Path, PathBuf};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn capture(generation: u64) -> EvidenceSnapshot {
    EvidenceSnapshot::new(EvidenceIdentity { source: 42, generation,
        scope: profile().delivery.scope }, snapshot(),
        ["alpha", "beta"].into_iter().map(|name| (name.to_owned(), Vec::new())).collect()).unwrap()
}
fn path(root: &Directory) -> PathBuf { root.store().with_extension("policy") }
fn write(path: &Path, capture: &EvidenceSnapshot) {
    let stage = path.with_extension("pending");
    std::fs::write(&stage, capture.encode()).unwrap();
    std::fs::rename(&stage, path).unwrap();
}
fn source(root: &Directory) -> FileEvidenceSource {
    FileEvidenceSource::new(path(root), 42, profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap()
}
fn no_clock() -> ElapsedTick { panic!("this request must not acquire a policy observation") }

#[test]
fn valid_new_wire_intent_reads_policy_once_without_exposing_source_or_generated_bytes() {
    let root = Directory::new(); let (host, _) = owner(&root, &config());
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    let mut wire = ActorWire::new(port);
    write(&path(&root), &capture(1)); let mut source = source(&root);
    let mut clocks = 0;
    let report = supervisor.exchange_learned_text_actor_from_policy_file(&mut wire,
        &raw_document(71), &mut source, || { clocks += 1; ElapsedTick(2) }).unwrap();
    assert_eq!(clocks, 2); assert_eq!(source.status().read_attempts, 1);
    assert_eq!(report.response.result, Ok(Knowledge::Pending { request: 71 }));
    let intake = report.intake.unwrap();
    assert_eq!(intake.result, Ok(capture(1).identity()));
    assert_eq!(intake.observations, vec![Ok(capture(1).identity())]);
    assert!(intake.source_updates.is_empty(), "no claimed durable producer-floor update");
    let expected = WireResponse { request: Some(71), result: Ok(Knowledge::Pending { request: 71 }) };
    assert_eq!(report.response.encode(), expected.encode());
    let host = supervisor.host().unwrap();
    let action = host.request_action(71).unwrap();
    assert_eq!(action.spec().payload, b"aa"); assert_eq!(action.spec().units, 2);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
    drop(host);
    // The file-derived observation was spent by the original request transaction.
    assert_eq!(wire.exchange(&raw_document(72)).result, Err(WireError::Unavailable));
}

#[test]
fn malformed_poll_cancel_and_exact_or_conflicting_retries_never_read_or_sample_time() {
    let root = Directory::new(); let (host, _) = owner(&root, &config());
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    let mut wire = ActorWire::new(port);
    write(&path(&root), &capture(1)); let mut source = source(&root);
    let mut bad = FileLearnedTextActorPort::encode_request(71, proposal()).unwrap();
    bad.payload[0] ^= 1;
    let malformed = supervisor.exchange_learned_text_actor_from_policy_file(&mut wire,
        &document(71, bad), &mut source, no_clock).unwrap();
    assert_eq!(malformed.response.result, Err(WireError::MalformedRequest)); assert!(malformed.intake.is_none());
    let unknown = supervisor.exchange_learned_text_actor_from_policy_file(&mut wire,
        &encode_command(&Command::Poll { request: 71 }).unwrap(), &mut source, no_clock).unwrap();
    assert!(matches!(unknown.response.result, Ok(Knowledge::Withheld { .. })));
    assert!(unknown.intake.is_none()); assert_eq!(source.status().read_attempts, 0);
    let accepted = supervisor.exchange_learned_text_actor_from_policy_file(&mut wire,
        &raw_document(71), &mut source, || ElapsedTick(2)).unwrap().response;
    assert_eq!(accepted.result, Ok(Knowledge::Pending { request: 71 }));
    // Break the current file: historical requests must not depend on it.
    let stage = path(&root).with_extension("pending");
    std::fs::write(&stage, b"not policy JSON").unwrap(); std::fs::rename(stage, path(&root)).unwrap();
    observe(&mut supervisor, snapshot());
    let revision = supervisor.host().unwrap().revision();
    let retry = supervisor.exchange_learned_text_actor_from_policy_file(&mut wire,
        &raw_document(71), &mut source, no_clock).unwrap();
    assert_eq!(retry.response, accepted); assert!(retry.intake.is_none());
    let mut changed = proposal(); changed.units = 3;
    let conflict = supervisor.exchange_learned_text_actor_from_policy_file(&mut wire,
        &document(71, FileLearnedTextActorPort::encode_request(71, changed).unwrap()), &mut source, no_clock).unwrap();
    assert_eq!(conflict.response.result, Err(WireError::IdempotencyConflict)); assert!(conflict.intake.is_none());
    assert_eq!(supervisor.host().unwrap().revision(), revision);
    // Neither retry consumed the next admission's existing one-use snapshot.
    assert_eq!(wire.exchange(&raw_document(72)).result, Ok(Knowledge::Pending { request: 72 }));
    let cancel = supervisor.exchange_learned_text_actor_from_policy_file(&mut wire,
        &encode_command(&Command::Cancel { request: 71 }).unwrap(), &mut source, no_clock).unwrap();
    assert!(matches!(cancel.response.result, Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    assert!(cancel.intake.is_none()); assert_eq!(source.status().read_attempts, 1);
}

#[test]
fn file_refusal_withdraws_old_snapshot_and_redacts_private_roster_scope_and_completeness_failures() {
    for kind in 0..4 {
        let root = Directory::new(); let (host, _) = owner(&root, &config());
        let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
        let mut wire = ActorWire::new(port);
        let good = capture(1);
        let mut identity = good.identity(); let mut state = good.snapshot().clone();
        let mut contexts = good.contexts().clone();
        match kind {
            0 => { contexts.insert("alpha".to_owned(), b"private instruction replacing the native view".to_vec()); }
            1 => { contexts.remove("beta"); }
            2 => state.complete = false,
            _ => identity.scope.principal += 1,
        }
        let bad = EvidenceSnapshot::new(identity, state, contexts).unwrap();
        write(&path(&root), &bad); let mut source = source(&root);
        observe(&mut supervisor, snapshot());
        let report = supervisor.exchange_learned_text_actor_from_policy_file(&mut wire,
            &raw_document(71), &mut source, || ElapsedTick(2)).unwrap();
        assert_eq!(report.response.result, Err(WireError::Unavailable));
        assert_eq!(report.response.encode(), WireResponse { request: Some(71), result: Err(WireError::Unavailable) }.encode());
        let intake = report.intake.unwrap();
        assert!(intake.result.is_err()); assert!(matches!(intake.observations.as_slice(), [Err(EvidenceError::Data(_))]));
        assert!(intake.source_updates.is_empty()); assert_eq!(source.status().read_attempts, 1);
        assert_eq!(wire.exchange(&raw_document(71)).result, Err(WireError::Unavailable), "old slot must be gone");
        assert_eq!(supervisor.host().unwrap().retained_requests(), 0);
        // A separately published newer good observation is a paired positive.
        write(&path(&root), &capture(2));
        let report = supervisor.exchange_learned_text_actor_from_policy_file(&mut wire,
            &raw_document(71), &mut source, || ElapsedTick(3)).unwrap();
        assert_eq!(report.response.result, Ok(Knowledge::Pending { request: 71 }));
        assert_eq!(report.intake.unwrap().result, Ok(capture(2).identity()));
        assert_eq!(source.status().read_attempts, 2);
    }
}

#[test]
fn failed_or_unwound_post_read_clock_cannot_restore_an_old_or_new_snapshot() {
    for panic_after_read in [false, true] {
        let root = Directory::new(); let (host, _) = owner(&root, &config());
        let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
        let mut wire = ActorWire::new(port.clone());
        write(&path(&root), &capture(1)); let mut source = source(&root);
        observe(&mut supervisor, snapshot());
        let mut calls = 0;
        let result = catch_unwind(AssertUnwindSafe(|| supervisor.exchange_learned_text_actor_from_policy_file(
            &mut wire, &raw_document(71), &mut source, || {
                calls += 1;
                assert!(matches!(port.submit(999, proposal()), Err(crate::action::consequence::oversight::actor::ActorError::Unavailable)),
                    "exclusive source custody must prevent reentrant use of a prior slot");
                if calls == 1 { ElapsedTick(2) }
                else if panic_after_read { panic!("controlled post-read clock unwind"); }
                else { ElapsedTick(1) }
            })));
        assert_eq!(calls, 2); assert_eq!(source.status().read_attempts, 1);
        if panic_after_read { assert!(result.is_err()); }
        else {
            let report = result.unwrap().unwrap();
            assert_eq!(report.response.result, Err(WireError::Unavailable));
            let intake = report.intake.unwrap();
            assert_eq!(intake.observations, vec![Ok(capture(1).identity())]);
            assert!(intake.result.is_err());
        }
        assert_eq!(supervisor.host().unwrap().retained_requests(), 0);
        assert_eq!(wire.exchange(&raw_document(71)).result, Err(WireError::Unavailable));
        let report = supervisor.exchange_learned_text_actor_from_policy_file(&mut wire,
            &raw_document(71), &mut source, || ElapsedTick(3)).unwrap();
        assert_eq!(report.response.result, Ok(Knowledge::Pending { request: 71 }));
    }
}

#[test]
fn incomplete_recovered_or_interrupted_original_source_refuses_before_policy_io() {
    for kind in 0..3 {
        let root = Directory::new(); let config = config();
        let host = match kind {
            0 => {
                let (mut host, _) = FileOversight::create_with_learned_text(root.store(), profile(), config.clone()).unwrap();
                host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host); host
            }
            1 => {
                let (host, _) = owner(&root, &config); drop(host);
                FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap().0
            }
            _ => { let (mut host, _) = owner(&root, &config); host.source_interrupted = true; host }
        };
        let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
        let mut wire = ActorWire::new(port);
        write(&path(&root), &capture(1)); let mut source = source(&root);
        let revision = supervisor.host().unwrap().revision();
        let report = supervisor.exchange_learned_text_actor_from_policy_file(&mut wire,
            &raw_document(71), &mut source, no_clock).unwrap();
        assert_eq!(report.response.result, Err(WireError::Unavailable)); assert!(report.intake.is_none());
        assert_eq!(source.status().read_attempts, 0); assert_eq!(supervisor.host().unwrap().revision(), revision);
        // Direct preparation also refuses, but first withdraws any older slot.
        observe(&mut supervisor, snapshot());
        let report = supervisor.prepare_learned_policy_intake(&mut source, no_clock);
        assert!(report.result.is_err()); assert!(report.observations.is_empty());
        assert_eq!(source.status().read_attempts, 0);
    }
}

#[test]
fn source_preparation_success_is_not_a_claim_that_request_storage_committed() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = config(); let (host, _) = owner(&root, &config);
        let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
        let mut wire = ActorWire::new(port);
        write(&path(&root), &capture(1)); let mut source = source(&root);
        // All clock observations remain at the acknowledged tick. The injected
        // failure therefore belongs to original request admission, not Time.
        supervisor.host().unwrap().store.fail_once(barrier);
        let report = supervisor.exchange_learned_text_actor_from_policy_file(&mut wire,
            &raw_document(71), &mut source, || ElapsedTick(1)).unwrap();
        assert_eq!(report.intake.unwrap().result, Ok(capture(1).identity()));
        assert_eq!(report.response.result, Err(WireError::Unavailable));
        assert_eq!(source.status().read_attempts, 1);
        assert!(matches!(wire.exchange(&encode_command(&Command::Poll { request: 71 }).unwrap()).result,
            Ok(Knowledge::Withheld { .. })));
        assert!(supervisor.host().unwrap().storage_failure().is_some());
        drop(supervisor);
        let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
    }
}

#[test]
fn original_channel_reads_only_at_complete_new_frames_after_response_flush() {
    let root = Directory::new(); let (host, _) = owner(&root, &config());
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    let mut channel = ActorChannel::new(ActorWire::new(port), ChannelLimits::default()).unwrap();
    write(&path(&root), &capture(1)); let mut source = source(&root);
    let first = raw_document(71); let mut second = raw_document(72); second.push(b'\n');
    let part = supervisor.feed_learned_text_actor_from_policy_file(&mut channel, &first, &mut source, no_clock).unwrap();
    assert_eq!(part.feed.state, ChannelState::Reading); assert!(part.intake.is_none());
    assert_eq!(source.status().read_attempts, 0);
    let mut batch = vec![b'\n']; batch.extend_from_slice(&second);
    let result = supervisor.feed_learned_text_actor_from_policy_file(&mut channel, &batch,
        &mut source, || ElapsedTick(2)).unwrap();
    assert_eq!(result.feed.consumed, 1); assert!(result.intake.unwrap().result.is_ok());
    channel.acknowledge_written(channel.pending_output().len()).unwrap();
    let blocked = supervisor.feed_learned_text_actor_from_policy_file(&mut channel, &second, &mut source, no_clock).unwrap();
    assert_eq!(blocked.feed.consumed, 0); assert!(blocked.intake.is_none());
    assert_eq!(source.status().read_attempts, 1);
    channel.acknowledge_flushed().unwrap();
    let result = supervisor.feed_learned_text_actor_from_policy_file(&mut channel, &second,
        &mut source, || ElapsedTick(3)).unwrap();
    assert_eq!(result.feed.consumed, second.len()); assert!(result.intake.unwrap().result.is_ok());
    assert_eq!(source.status().read_attempts, 2); assert_eq!(supervisor.host().unwrap().retained_requests(), 2);
}

#[test]
fn stream_policy_wire_preserves_release_law_before_acquisition_and_original_full_charge() {
    let root = Directory::new(); let config = stream_config(); let (host, _) = stream_owner(&root, &config);
    let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut wire = ActorWire::new(port);
    write(&path(&root), &capture(1)); let mut source = source(&root);
    let early = supervisor.exchange_learned_text_stream_actor_from_policy_file(&mut wire,
        &stream_document(71, LearnedTextRelease::Finish), &mut source, no_clock).unwrap();
    assert_eq!(early.response.result, Err(WireError::Unavailable)); assert!(early.intake.is_none());
    let accepted = supervisor.exchange_learned_text_stream_actor_from_policy_file(&mut wire,
        &stream_document(71, LearnedTextRelease::Message), &mut source, || ElapsedTick(2)).unwrap();
    assert_eq!(accepted.response.result, Ok(Knowledge::Pending { request: 71 }));
    assert_eq!(accepted.intake.unwrap().result, Ok(capture(1).identity()));
    let host = supervisor.host().unwrap();
    let spec = host.request_action(71).unwrap().spec();
    assert_eq!(ReleaseFrame::decode(&spec.payload).unwrap().message(), Some("aa"));
    assert_eq!(spec.units, spec.payload.len() as u64); assert!(spec.units > 17); drop(host);
    drop(supervisor);
    let (host, _) = FileOversight::open_with_learned_text_stream(root.store(), stream_delivery_profile(), &config).unwrap();
    let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut wire = ActorWire::new(port);
    let retry = supervisor.exchange_learned_text_stream_actor_from_policy_file(&mut wire,
        &stream_document(71, LearnedTextRelease::Message), &mut source, no_clock).unwrap();
    assert!(matches!(retry.response.result, Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    assert!(retry.intake.is_none()); assert_eq!(source.status().read_attempts, 1);
    assert!(supervisor.host().unwrap().learned_generation_inspection().unwrap().paused);
}

#[test]
fn foreign_wire_owner_refuses_before_parsing_or_consuming_either_admission_slot() {
    let a = Directory::new(); let b = Directory::new();
    let (host_a, _) = owner(&a, &config()); let (host_b, _) = owner(&b, &config());
    let (port_a, mut supervisor_a) = host_a.into_learned_text_actor_gateway().unwrap();
    let (port_b, mut supervisor_b) = host_b.into_learned_text_actor_gateway().unwrap();
    let mut wire_a = ActorWire::new(port_a); let mut wire_b = ActorWire::new(port_b);
    observe(&mut supervisor_a, snapshot()); observe(&mut supervisor_b, snapshot());
    let mut source = source(&a); // even a missing file must not be opened
    assert!(matches!(supervisor_a.exchange_learned_text_actor_from_policy_file(
        &mut wire_b, b"not JSON", &mut source, no_clock), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(source.status().read_attempts, 0);
    assert_eq!(wire_a.exchange(&raw_document(71)).result, Ok(Knowledge::Pending { request: 71 }));
    assert_eq!(wire_b.exchange(&raw_document(71)).result, Ok(Knowledge::Pending { request: 71 }));
}

#[cfg(target_os = "linux")]
mod peer;

#[cfg(target_os = "linux")]
mod pipeline;
