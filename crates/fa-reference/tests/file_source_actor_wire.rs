//! Original actor bytes with source reads only at fresh durable submission.
#![cfg(unix)]
#[path = "support/file_source_intake.rs"] mod fixture;
use fixture::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::driver::FileDriverEvent;
use fa_reference::action::consequence::delivery::persistent::observed::source::FileSourceError;
use fa_reference::action::consequence::oversight::actor::{ActorError, ActorOutcome, Knowledge};
use fa_reference::action::consequence::oversight::actor_wire::{ActorChannel, ActorWire, ChannelLimits,
    ChannelState, Command, WireError, encode_command};
use fa_reference::action::consequence::oversight::policy_state::StateLimits;
use fa_reference::Error;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

fn submit(file: &FileRig, request: u64) -> Vec<u8> {
    encode_command(&Command::Submit { request, proposal: file.rig.proposal() }).unwrap()
}
fn command(request: u64, cancel: bool) -> Vec<u8> {
    encode_command(&if cancel { Command::Cancel { request } } else { Command::Poll { request } }).unwrap()
}

#[test]
fn malformed_and_observation_commands_never_acquire_a_source_snapshot() {
    let mut file = cold(10, StateLimits::default());
    let mut wire = ActorWire::new(file.rig.port.clone());
    let malformed = [b"{}".to_vec(), b"{\"version\":1,\"version\":1}".to_vec(),
        b"{\"version\":1,\"operation\":\"poll\",\"request\":\"1\",\"scope\":1}".to_vec()];
    for bytes in malformed {
        let report = file.rig.driver.exchange_actor_from_file(&mut wire, &bytes, &mut file.source,
            || panic!("invalid command sampled time")).unwrap();
        assert!(report.response.result.is_err()); assert!(report.intake.is_none());
    }
    let poll = file.rig.driver.exchange_actor_from_file(&mut wire, &command(1, false), &mut file.source,
        || panic!("poll sampled time")).unwrap();
    assert!(matches!(poll.response.result, Ok(Knowledge::Withheld { .. })));
    let cancel = file.rig.driver.exchange_actor_from_file(&mut wire, &command(1, true), &mut file.source,
        || panic!("cancel sampled time")).unwrap();
    assert_eq!(cancel.response.result, Err(WireError::Withheld));
    assert_eq!(file.source.status().read_attempts, 0);
    let bytes = submit(&file, 1);
    let accepted = file.rig.driver.exchange_actor_from_file(&mut wire, &bytes, &mut file.source, || ElapsedTick(1)).unwrap();
    assert!(matches!(accepted.response.result, Ok(Knowledge::Pending { request: 1 })));
    assert!(accepted.intake.unwrap().result.is_ok());
}

#[test]
fn retries_and_conflicts_reuse_the_original_durable_identity_without_reading_or_renewal() {
    let mut file = cold(10, StateLimits::default()); let bytes = submit(&file, 1);
    let mut wire = ActorWire::new(file.rig.port.clone());
    let accepted = file.rig.driver.exchange_actor_from_file(&mut wire, &bytes, &mut file.source, || ElapsedTick(1)).unwrap();
    assert!(accepted.response.result.is_ok());
    assert_eq!(file.rig.port.submit(2, &file.rig.proposal()).unwrap_err(), ActorError::Unavailable);
    fs::remove_file(&file.path).unwrap();
    let revision = file.rig.driver.supervisor().host().unwrap().revision();
    let retry = file.rig.driver.exchange_actor_from_file(&mut wire, &bytes, &mut file.source,
        || panic!("retry sampled time")).unwrap();
    assert_eq!(retry.response, accepted.response); assert!(retry.intake.is_none());
    let mut proposal = file.rig.proposal(); proposal.payload = b"other".to_vec();
    let different = encode_command(&Command::Submit { request: 1, proposal }).unwrap();
    let conflict = file.rig.driver.exchange_actor_from_file(&mut wire, &different, &mut file.source,
        || panic!("conflict read a source")).unwrap();
    assert_eq!(conflict.response.result, Err(WireError::IdempotencyConflict)); assert!(conflict.intake.is_none());
    assert_eq!(file.rig.driver.supervisor().host().unwrap().revision(), revision);
    let cancelled = file.rig.driver.exchange_actor_from_file(&mut wire, &command(1, true), &mut file.source,
        || panic!("cancellation sampled time")).unwrap();
    assert!(matches!(cancelled.response.result, Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    assert!(cancelled.intake.is_none()); assert_eq!(file.source.status().read_attempts, 1);
}

#[test]
fn newline_and_actual_write_flush_barriers_precede_every_new_source_read() {
    struct BlockedFlush { bytes: Vec<u8>, blocked: bool }
    impl Write for BlockedFlush {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { self.bytes.extend_from_slice(bytes); Ok(bytes.len()) }
        fn flush(&mut self) -> io::Result<()> {
            if self.blocked { self.blocked = false; Err(io::ErrorKind::WouldBlock.into()) } else { Ok(()) }
        }
    }
    let mut file = cold(10, StateLimits::default()); let first = submit(&file, 1); let second = submit(&file, 2);
    let mut channel = ActorChannel::new(ActorWire::new(file.rig.port.clone()), ChannelLimits::default()).unwrap();
    let fragment = file.rig.driver.feed_actor_from_file(&mut channel, &first, &mut file.source,
        || panic!("unterminated frame read a source")).unwrap();
    assert_eq!(fragment.feed.consumed, first.len()); assert_eq!(fragment.feed.state, ChannelState::Reading);
    assert!(fragment.intake.is_none()); assert_eq!(file.source.status().read_attempts, 0);
    let mut pipelined = vec![b'\n']; pipelined.extend_from_slice(&second); pipelined.push(b'\n');
    let ready = file.rig.driver.feed_actor_from_file(&mut channel, &pipelined, &mut file.source, || ElapsedTick(1)).unwrap();
    assert_eq!(ready.feed.consumed, 1); assert!(ready.intake.unwrap().result.is_ok());
    let mut writer = BlockedFlush { bytes: Vec::new(), blocked: true };
    assert_eq!(channel.write_once(&mut writer).unwrap_err().kind(), io::ErrorKind::WouldBlock);
    let withheld = file.rig.driver.feed_actor_from_file(&mut channel, &pipelined[1..], &mut file.source,
        || panic!("pending flush read a source")).unwrap();
    assert_eq!(withheld.feed.consumed, 0); assert!(withheld.intake.is_none());
    assert_eq!(file.source.status().read_attempts, 1);
    channel.write_once(&mut writer).unwrap();
    let next = file.rig.driver.feed_actor_from_file(&mut channel, &pipelined[1..], &mut file.source, || ElapsedTick(2)).unwrap();
    assert_eq!(next.feed.consumed, pipelined.len() - 1); assert!(next.intake.unwrap().result.is_ok());
    assert_eq!(file.source.status().read_attempts, 2);
    assert!(!writer.bytes.windows(evidence::PRIVATE.len()).any(|window| window == evidence::PRIVATE));
}

#[test]
fn partial_socket_input_and_lost_response_recover_without_a_second_submission_or_capture() {
    let mut file = cold(10, StateLimits::default()); let document = submit(&file, 1);
    let (mut server, mut client) = UnixStream::pair().unwrap();
    server.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let mut channel = ActorChannel::new(ActorWire::new(file.rig.port.clone()), ChannelLimits::default()).unwrap();
    client.write_all(&document).unwrap(); client.write_all(b"\n").unwrap();
    let mut received = 0;
    while channel.state() == ChannelState::Reading {
        let mut bytes = [0; 7]; let count = server.read(&mut bytes).unwrap(); assert!(count > 0);
        let report = file.rig.driver.feed_actor_from_file(&mut channel, &bytes[..count], &mut file.source, || ElapsedTick(1)).unwrap();
        assert_eq!(report.feed.consumed, count); received += count;
    }
    assert_eq!(received, document.len() + 1); assert_eq!(file.source.status().read_attempts, 1);
    // Drop the transport before any response is sent. Its durable request lives.
    drop(client); drop(server); drop(channel);
    fs::remove_file(&file.path).unwrap();
    let revision = file.rig.driver.supervisor().host().unwrap().revision();
    let mut wire = ActorWire::new(file.rig.port.clone());
    let hidden = file.rig.driver.exchange_actor_from_file(&mut wire, &command(1, false), &mut file.source,
        || panic!("unseen ticket read a source")).unwrap();
    assert!(matches!(hidden.response.result, Ok(Knowledge::Withheld { .. })));
    let retry = file.rig.driver.exchange_actor_from_file(&mut wire, &document, &mut file.source,
        || panic!("reconnect retry read a source")).unwrap();
    assert!(retry.intake.is_none()); assert!(matches!(retry.response.result, Ok(Knowledge::Pending { .. })));
    assert_eq!(file.rig.driver.supervisor().host().unwrap().revision(), revision);
    assert_eq!(file.source.status().read_attempts, 1);
}

#[test]
fn a_foreign_gateway_cannot_prime_a_slot_or_cancel_a_request_in_another_owner() {
    let mut first = cold(10, StateLimits::default()); let second = cold(10, StateLimits::default());
    let mut wire = ActorWire::new(second.rig.port.clone()); let bytes = submit(&first, 1);
    for document in [bytes, command(1, false), command(1, true)] {
        let result = first.rig.driver.exchange_actor_from_file(&mut wire, &document, &mut first.source,
            || panic!("foreign gateway sampled time"));
        assert!(matches!(result, Err(JournalError::Contract(Error::Binding))));
    }
    assert_eq!(first.source.status().read_attempts, 0);
    assert!(first.rig.driver.supervisor().host().unwrap().inspect().control.ledger.stages.is_empty());
    assert!(second.rig.driver.supervisor().host().unwrap().inspect().control.ledger.stages.is_empty());
}

#[test]
fn failed_preparation_is_redacted_and_cannot_use_a_previously_primed_snapshot() {
    let mut file = cold(10, StateLimits::default()); let bytes = submit(&file, 1);
    file.rig.driver.prepare_file_intake(&mut file.source, || ElapsedTick(1)).result.unwrap();
    fs::remove_file(&file.path).unwrap(); let mut wire = ActorWire::new(file.rig.port.clone());
    let refused = file.rig.driver.exchange_actor_from_file(&mut wire, &bytes, &mut file.source, || ElapsedTick(2)).unwrap();
    assert_eq!(refused.response.result, Err(WireError::Unavailable));
    assert!(refused.intake.unwrap().result.is_err());
    assert_eq!(file.rig.port.submit(1, &file.rig.proposal()).unwrap_err(), ActorError::Unavailable);
    assert!(file.rig.driver.supervisor().host().unwrap().inspect().control.ledger.stages.is_empty());
    assert_eq!(refused.response.encode(), b"{\"version\":1,\"request\":\"1\",\"status\":\"error\",\"reason\":\"unavailable\"}");
    replace(&file.path, &document(2));
    let accepted = file.rig.driver.exchange_actor_from_file(&mut wire, &bytes, &mut file.source, || ElapsedTick(3)).unwrap();
    assert!(accepted.intake.unwrap().result.is_ok()); assert!(matches!(accepted.response.result, Ok(Knowledge::Pending { .. })));
}

#[test]
fn recorded_admission_refusal_does_not_reread_better_evidence_on_retry() {
    let mut file = cold(10, StateLimits::default()); let mut proposal = file.rig.proposal();
    proposal.expected_policy_epoch += 1;
    let bytes = encode_command(&Command::Submit { request: 1, proposal }).unwrap();
    let mut wire = ActorWire::new(file.rig.port.clone());
    let refused = file.rig.driver.exchange_actor_from_file(&mut wire, &bytes, &mut file.source, || ElapsedTick(1)).unwrap();
    assert!(refused.intake.unwrap().result.is_ok());
    assert!(matches!(refused.response.result, Ok(Knowledge::Known { value: ActorOutcome::NotAdmitted, .. })));
    fs::remove_file(&file.path).unwrap(); let revision = file.rig.driver.supervisor().host().unwrap().revision();
    let retry = file.rig.driver.exchange_actor_from_file(&mut wire, &bytes, &mut file.source,
        || panic!("recorded refusal read a source")).unwrap();
    assert!(retry.intake.is_none()); assert_eq!(retry.response, refused.response);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().revision(), revision);
}

#[test]
fn successful_capture_is_not_misreported_as_successful_durable_submission() {
    let mut file = cold(10, StateLimits::default()); let bytes = submit(&file, 1);
    let mut wire = ActorWire::new(file.rig.port.clone()); let path = file.rig.root.store().join("delivery.pending");
    let mut calls = 0;
    let report = file.rig.driver.exchange_actor_from_file(&mut wire, &bytes, &mut file.source, || {
        calls += 1;
        // Native source has committed. Fail the DISTINCT ensuing submit write.
        if calls == 2 { fs::write(&path, b"occupied before submit").unwrap(); }
        ElapsedTick(1)
    }).unwrap();
    assert_eq!(calls, 2); assert!(report.intake.unwrap().result.is_ok());
    assert_eq!(report.response.result, Err(WireError::Unavailable));
    let host = file.rig.driver.supervisor().host().unwrap();
    assert!(host.storage_failure().is_some()); assert!(host.inspect().control.ledger.stages.is_empty());
    assert_eq!(file.source.status().read_attempts, 1);
}

#[test]
fn source_quota_is_not_renewed_by_another_well_formed_submit() {
    let mut file = cold(10, StateLimits { events: 1, ..StateLimits::default() });
    let first = submit(&file, 1); let second = submit(&file, 2); let mut wire = ActorWire::new(file.rig.port.clone());
    file.rig.driver.exchange_actor_from_file(&mut wire, &first, &mut file.source, || ElapsedTick(1)).unwrap();
    let exhausted = file.rig.driver.exchange_actor_from_file(&mut wire, &second, &mut file.source, || ElapsedTick(1)).unwrap();
    assert_eq!(exhausted.response.result, Err(WireError::Capacity));
    assert_eq!(exhausted.intake.unwrap().source_updates, vec![Err(FileSourceError::Refused(Error::Limit))]);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().control.ledger.stages.len(), 1);
    let retry = file.rig.driver.exchange_actor_from_file(&mut wire, &first, &mut file.source,
        || panic!("existing key tried to repair source quota")).unwrap();
    assert!(retry.intake.is_none()); assert!(retry.response.result.is_ok());
}

#[test]
fn complete_owner_reopen_recovers_exact_actor_identity_without_reacquiring_evidence() {
    let mut file = cold(10, StateLimits::default()); let bytes = submit(&file, 1);
    let mut old_wire = ActorWire::new(file.rig.port.clone());
    file.rig.driver.exchange_actor_from_file(&mut old_wire, &bytes, &mut file.source, || ElapsedTick(1)).unwrap();
    file.reviewed(); let key = file.human(); file.dispatch(&key);
    assert!(matches!(file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(1), None).result,
        Ok(FileDriverEvent::PublicationChecked { .. })));
    let reads = file.source.status().read_attempts;
    fs::remove_file(&file.path).unwrap(); drop(file.rig.driver.release());
    let (host, reviewer) = FileOversight::open(file.rig.root.store(), driver::profile()).unwrap();
    let (port, driver) = host.into_supervised_driver();
    file.rig.port = port; file.rig.driver = driver; file.rig.reviewer = reviewer;
    let old = file.rig.driver.exchange_actor_from_file(&mut old_wire, &bytes, &mut file.source,
        || panic!("old gateway sampled time"));
    assert!(matches!(old, Err(JournalError::Contract(Error::Binding))));
    let mut new_wire = ActorWire::new(file.rig.port.clone());
    let recovered = file.rig.driver.exchange_actor_from_file(&mut new_wire, &bytes, &mut file.source,
        || panic!("recovered exact retry sampled time")).unwrap();
    assert!(recovered.intake.is_none()); assert!(matches!(recovered.response.result, Ok(Knowledge::Unknown { .. })));
    file.rig.driver.resume_reconciliation(1).unwrap();
    let settled = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(2), None);
    assert!(settled.source_updates.is_empty());
    assert!(matches!(settled.result, Ok(FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(_), .. })));
    let poll = file.rig.driver.exchange_actor_from_file(&mut new_wire, &command(1, false), &mut file.source,
        || panic!("recovered poll sampled time")).unwrap();
    assert!(matches!(poll.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert_eq!(file.source.status().read_attempts, reads);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().executions, 1);
}
