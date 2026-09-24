//! Public integration paths: real source files and original numerical execution.
use super::*;
use crate::Error;
use crate::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use crate::action::consequence::oversight::actor_wire::{
    ChannelLimits, ChannelState, Command, WireError, decode_response, encode_command,
};
use crate::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use std::cell::Cell;
mod fixture;
use fixture::*;

fn no_clock() -> ElapsedTick { panic!("this operation must not acquire evidence or sample time") }
fn pending(response: &crate::action::consequence::oversight::actor_wire::WireResponse) {
    assert_eq!(response.result, Ok(Knowledge::Pending { request: 91 }));
}

#[test]
fn generated_source_intake_reaches_the_original_two_key_publication_path() {
    use crate::action::consequence::oversight::ReviewWindow;
    use crate::action::consequence::delivery::EndpointOutcome;
    use crate::action::consequence::delivery::stream::ReleaseFrame;
    use crate::round::{Verdict, commitment};
    let mut s = setup(); let mut wire = ActorWire::new(s.port.clone()); let input = document(&s.input);
    let before = disk(&s); let numerical = s.supervisor.host().unwrap().decoder_inspection().unwrap().numerical;
    assert_eq!(wire.exchange(&input).result, Err(WireError::Unavailable));
    assert_eq!(disk(&s), before); // no separately installed gateway snapshot
    let reads = s.source.status().read_attempts; let calls = Cell::new(0);
    let report = s.supervisor.exchange_generated_actor_from_file(&mut wire, &input, &mut s.source,
        || { calls.set(calls.get() + 1); ElapsedTick(2) }).unwrap();
    pending(&report.response);
    let intake = report.intake.unwrap();
    assert_eq!(intake.result, Ok(capture(1, true, b"allow").identity()));
    assert_eq!(intake.observations.len(), 1); assert_eq!(intake.source_updates.len(), 1);
    assert_eq!(calls.get(), 2); assert_eq!(s.source.status().read_attempts, reads + 1);
    let mut borrowed = s.supervisor.host_mut().unwrap(); let host = &mut *borrowed;
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(91).unwrap().disposition else { panic!("native admission"); };
    let action = host.request_action(91).unwrap().clone();
    assert_eq!(ReleaseFrame::decode(&action.spec().payload).unwrap().message(), Some("A"));
    assert_eq!(action.spec().units, action.spec().payload.len() as u64);
    assert!(action.spec().units > FileGeneratedTextActorPort::INTENT_BYTES as u64);
    assert_eq!(host.inspect().executions, 0);
    let observed = capture(1, true, b"allow");
    let snapshot = observed.snapshot().clone();
    let inputs = observed.inputs_for(&action, &profile().committee).unwrap();
    host.record_inputs(host.revision(), attempt, 0, inputs.clone()).unwrap();
    host.begin_review(host.revision(), attempt, 101, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8),
    }, snapshot.clone()).unwrap();
    host.commit_review(host.revision(), 101, "reviewer", commitment(101, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap()).unwrap();
    host.open_reveals(host.revision(), 101).unwrap();
    host.reveal_review(host.revision(), 101, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), 101, Some(&inputs), snapshot.clone()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), attempt, &inputs, snapshot.clone()).unwrap();
    assert!(host.publish_checked(host.revision(), attempt, Some(&inputs), snapshot.clone(), ElapsedTick(2)).is_err());
    assert_eq!(host.inspect().executions, 0); // neither intake nor congress is the human key
    let request = host.request_human_approval(host.revision(), 1001, attempt, &inputs, ElapsedTick(10)).unwrap();
    let revision = host.revision(); let human = s.reviewer.approve(host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot.clone()).unwrap();
    assert!(matches!(host.publish_checked(host.revision(), attempt, Some(&inputs), snapshot, ElapsedTick(2)).unwrap().outcome,
        EndpointOutcome::Executed { .. }));
    drop(borrowed);
    let poll = encode_command(&Command::Poll { request: 91 }).unwrap();
    let report = s.supervisor.exchange_generated_actor_from_file(&mut wire, &poll, &mut s.source, no_clock).unwrap();
    assert!(matches!(report.response.result, Ok(Knowledge::Unknown { .. })));
    assert!(report.intake.is_none());
    let mut borrowed = s.supervisor.host_mut().unwrap(); let host = &mut *borrowed;
    host.reconcile(host.revision(), attempt).unwrap();
    assert_eq!(host.stream_snapshot().unwrap().confirmed.visible(), b"A");
    assert_eq!(host.inspect().control.ledger.charged, action.spec().units);
    drop(borrowed);
    assert!(matches!(wire.exchange(&poll).result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
}

#[test]
fn generated_source_malformed_references_do_not_read_or_consume_a_waiting_snapshot() {
    let mut s = setup(); let mut wire = ActorWire::new(s.port.clone());
    let revision = s.supervisor.host().unwrap().revision();
    s.supervisor.set_snapshot(revision, Some(capture(1, true, b"allow").snapshot().clone())).unwrap();
    let before = disk(&s); let reads = s.source.status().read_attempts;
    for mode in 0..5 {
        let mut p = FileGeneratedTextActorPort::encode_message(&s.input).unwrap();
        match mode {
            0 => p.payload[0] ^= 1,
            1 => p.payload[8] ^= 1,
            2 => p.payload.truncate(31),
            3 => { p.payload.push(0); p.units = 33; }
            _ => p.units = 64,
        }
        let document = encode_command(&Command::Submit { request: 91, proposal: p }).unwrap();
        let report = s.supervisor.exchange_generated_actor_from_file(&mut wire, &document, &mut s.source, no_clock).unwrap();
        assert_eq!(report.response.result, Err(WireError::MalformedRequest)); assert!(report.intake.is_none());
    }
    let malformed = br#"{"version":1,"version":1,"operation":"poll","request":"91"}"#;
    let report = s.supervisor.exchange_generated_actor_from_file(&mut wire, malformed, &mut s.source, no_clock).unwrap();
    assert_eq!(report.response.result, Err(WireError::MalformedRequest)); assert!(report.intake.is_none());
    assert_eq!(disk(&s), before); assert_eq!(s.source.status().read_attempts, reads);
    let ticket = s.port.submit(&s.input).unwrap(); // the explicitly installed slot was not stolen
    assert_eq!(s.port.poll(&ticket), Knowledge::Pending { request: 91 });
}

#[test]
fn generated_source_recorded_retries_conflicts_poll_and_cancel_are_read_free() {
    let mut s = setup(); let mut wire = ActorWire::new(s.port.clone()); let doc = document(&s.input);
    pending(&s.supervisor.exchange_generated_actor_from_file(&mut wire, &doc, &mut s.source, || ElapsedTick(2)).unwrap().response);
    std::fs::remove_file(s.root.source()).unwrap();
    let reads = s.source.status().read_attempts; let before = disk(&s);
    let mut conflicting = s.input.clone(); conflicting.generation = 999;
    for (doc, conflict) in [(doc.clone(), false), (document(&conflicting), true),
        (encode_command(&Command::Poll { request: 91 }).unwrap(), false)] {
        let report = s.supervisor.exchange_generated_actor_from_file(&mut wire, &doc, &mut s.source, no_clock).unwrap();
        assert!(report.intake.is_none());
        if conflict { assert_eq!(report.response.result, Err(WireError::IdempotencyConflict)); }
        else { pending(&report.response); }
    }
    assert_eq!(disk(&s), before);
    let cancel = encode_command(&Command::Cancel { request: 91 }).unwrap();
    let report = s.supervisor.exchange_generated_actor_from_file(&mut wire, &cancel, &mut s.source, no_clock).unwrap();
    assert!(report.intake.is_none());
    assert!(matches!(report.response.result, Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    let before = disk(&s);
    let report = s.supervisor.exchange_generated_actor_from_file(&mut wire, &doc, &mut s.source, no_clock).unwrap();
    assert!(matches!(report.response.result, Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    assert_eq!(disk(&s), before); assert_eq!(s.source.status().read_attempts, reads);
}

#[test]
fn generated_source_missing_and_incomplete_evidence_withdraws_before_repair() {
    for incomplete in [false, true] {
        let mut s = setup(); let mut wire = ActorWire::new(s.port.clone()); let doc = document(&s.input);
        let revision = s.supervisor.host().unwrap().revision();
        s.supervisor.set_snapshot(revision, Some(capture(1, true, b"allow").snapshot().clone())).unwrap();
        if incomplete { s.root.replace(&capture(2, false, b"allow")); }
        else { std::fs::remove_file(s.root.source()).unwrap(); }
        let report = s.supervisor.exchange_generated_actor_from_file(&mut wire, &doc, &mut s.source, || ElapsedTick(2)).unwrap();
        assert_eq!(report.response.result, Err(WireError::Unavailable));
        let intake = report.intake.unwrap(); assert!(intake.result.is_err());
        assert_eq!(intake.source_updates.len(), 1); assert_eq!(intake.observations.len(), 1);
        assert!(s.port.submit(&s.input).is_err()); // the older snapshot is gone
        let host = s.supervisor.host().unwrap();
        assert!(matches!(host.request_status(91), Err(JournalError::Contract(Error::Missing))));
        assert!(host.file_source_status().unwrap().capture.closed.is_none());
        assert_eq!(host.inspect().executions, 0); drop(host);
        s.root.replace(&capture(3, true, b"allow"));
        let report = s.supervisor.exchange_generated_actor_from_file(&mut wire, &doc, &mut s.source, || ElapsedTick(3)).unwrap();
        pending(&report.response); assert!(report.intake.unwrap().result.is_ok());
    }
}

#[test]
fn generated_source_read_latency_uses_the_original_half_open_lease() {
    let mut s = setup(); let mut wire = ActorWire::new(s.port.clone()); let doc = document(&s.input);
    let calls = Cell::new(0);
    let report = s.supervisor.exchange_generated_actor_from_file(&mut wire, &doc, &mut s.source, || {
        calls.set(calls.get() + 1); ElapsedTick(if calls.get() == 1 { 2 } else { 12 })
    }).unwrap();
    assert_eq!(report.response.result, Err(WireError::Unavailable));
    let intake = report.intake.unwrap();
    assert!(intake.observations[0].is_ok()); assert!(intake.source_updates[0].is_ok());
    assert_eq!(intake.result, Err(JournalError::Contract(Error::Stale)));
    assert!(s.port.submit(&s.input).is_err());
    assert!(matches!(s.supervisor.host().unwrap().request_status(91), Err(JournalError::Contract(Error::Missing))));
    let calls = Cell::new(0);
    let report = s.supervisor.exchange_generated_actor_from_file(&mut wire, &doc, &mut s.source, || {
        calls.set(calls.get() + 1); ElapsedTick(if calls.get() == 1 { 13 } else { 14 })
    }).unwrap();
    pending(&report.response); assert!(report.intake.unwrap().result.is_ok());
}

#[test]
fn generated_source_foreign_gateways_and_reentrant_actor_calls_cannot_use_intake() {
    let mut a = setup(); let b = setup(); let mut wire = ActorWire::new(b.port.clone());
    let before_a = disk(&a); let before_b = disk(&b); let doc = document(&b.input);
    assert!(matches!(a.supervisor.exchange_generated_actor_from_file(&mut wire, &doc, &mut a.source, no_clock),
        Err(JournalError::Contract(Error::Binding))));
    assert_eq!(disk(&a), before_a); assert_eq!(disk(&b), before_b);
    let mut wire = ActorWire::new(a.port.clone()); let port = a.port.clone(); let input = a.input.clone();
    let calls = Cell::new(0);
    let report = a.supervisor.exchange_generated_actor_from_file(&mut wire, &document(&input), &mut a.source, || {
        calls.set(calls.get() + 1);
        assert!(matches!(port.submit(&input), Err(ActorError::Unavailable)));
        ElapsedTick(2)
    }).unwrap();
    pending(&report.response); assert_eq!(calls.get(), 2);
}

#[test]
fn generated_source_failed_storage_exposes_no_ticket_or_old_eligible_owner() {
    let mut s = setup(); let mut wire = ActorWire::new(s.port.clone()); let doc = document(&s.input);
    let before = disk(&s);
    // A real create_new staging conflict, not a mock successful storage callback.
    std::fs::write(s.root.store().join("delivery.pending"), b"unacknowledged staging bytes").unwrap();
    let report = s.supervisor.exchange_generated_actor_from_file(&mut wire, &doc, &mut s.source, || ElapsedTick(2)).unwrap();
    assert_eq!(report.response.result, Err(WireError::Unavailable));
    assert!(matches!(report.intake.unwrap().result, Err(JournalError::Io(_))));
    assert_eq!(disk(&s), before); assert!(s.supervisor.host().unwrap().storage_failure().is_some());
    assert!(matches!(s.supervisor.host().unwrap().request_status(91), Err(JournalError::Unavailable)));
    let poll = encode_command(&Command::Poll { request: 91 }).unwrap();
    assert!(matches!(wire.exchange(&poll).result, Ok(Knowledge::Withheld { .. })));
    let image = FileOversight::read_decoder_text_progress(s.root.store(), &profile(), &s.configuration, &s.tokenizer, 7).unwrap();
    assert_eq!(image.text.bytes().unwrap(), b"A"); assert_eq!(image.publication.executions, 0);
}

#[test]
fn generated_source_channel_reads_only_after_newline_and_prior_reply_flush() {
    let mut s = setup(); let mut channel = ActorChannel::new(ActorWire::new(s.port.clone()), ChannelLimits::default()).unwrap();
    let doc = document(&s.input); let reads = s.source.status().read_attempts;
    let fed = s.supervisor.feed_generated_actor_from_file(&mut channel, &doc, &mut s.source, no_clock).unwrap();
    assert!(fed.intake.is_none()); assert_eq!(fed.feed.consumed, doc.len());
    assert_eq!(fed.feed.state, ChannelState::Reading);
    let fed = s.supervisor.feed_generated_actor_from_file(&mut channel, b"\n", &mut s.source, || ElapsedTick(2)).unwrap();
    assert!(fed.intake.unwrap().result.is_ok()); assert_eq!(s.source.status().read_attempts, reads + 1);
    let response = channel.pending_output(); pending(&decode_response(&response[..response.len() - 1]).unwrap());
    let mut next = s.input.clone(); next.request = 92; let mut line = document(&next); line.push(b'\n');
    assert_eq!(s.supervisor.feed_generated_actor_from_file(&mut channel, &line, &mut s.source, no_clock).unwrap().feed.consumed, 0);
    channel.acknowledge_written(channel.pending_output().len()).unwrap();
    assert_eq!(s.supervisor.feed_generated_actor_from_file(&mut channel, &line, &mut s.source, no_clock).unwrap().feed.consumed, 0);
    channel.acknowledge_flushed().unwrap();
    let fed = s.supervisor.feed_generated_actor_from_file(&mut channel, &line, &mut s.source, || ElapsedTick(3)).unwrap();
    assert!(fed.intake.unwrap().result.is_ok()); assert_eq!(fed.feed.consumed, line.len());
    assert_eq!(s.source.status().read_attempts, reads + 2);
}
