//! Registered source reads occur at completed stream-intent frames, not packets.
#![cfg(unix)]
#[path = "support/file_stream_source.rs"] mod support;
use support::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, ActorChannel, ChannelLimits, Command, WireError, encode_command, decode_response};
use fa_reference::Error;
use std::cell::Cell;

#[test]
fn source_prepared_stream_requests_reach_original_review_human_publication_and_explicit_finish() {
    let root = Directory::new(); let (port, mut driver, human, mut source, evidence) = create(&root);
    let mut wire = ActorWire::new(port); let mut commands = Vec::new();
    for (index, message) in [Some("source-backed α"), Some("source-backed β"), None].into_iter().enumerate() {
        let attempt = index as u64 + 1; let request = 8000 + attempt;
        let command = stream::command(&driver.supervisor().host().unwrap(), request, message);
        let bytes = encode_command(&command).unwrap(); let reads = source.status().read_attempts;
        let result = driver.exchange_stream_actor_from_file(&mut wire, &bytes, &mut source, || ElapsedTick(1)).unwrap();
        assert_eq!(result.response.result, Ok(Knowledge::Pending { request }));
        assert_eq!(result.intake.unwrap().result.unwrap(), evidence.identity());
        assert_eq!(source.status().read_attempts, reads + 1);
        let keys = review_dispatch(&mut driver, &human, &evidence, request, attempt);
        let mut h = driver.supervisor_mut().host_mut().unwrap();
        let revision = h.revision();
        let outcome = h.publish_checked(revision, attempt, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap().outcome;
        assert_eq!(outcome, EndpointOutcome::Executed { resulting_version: attempt + 1 });
        drop(h);
        let poll = driver.exchange_stream_actor_from_file(&mut wire, &encode_command(&Command::Poll { request }).unwrap(),
            &mut source, || panic!("poll must not sample time")).unwrap();
        assert!(poll.intake.is_none());
        assert_eq!(poll.response.result, Ok(Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }));
        let cancel = driver.exchange_stream_actor_from_file(&mut wire, &encode_command(&Command::Cancel { request }).unwrap(),
            &mut source, || panic!("post-dispatch cancellation must not sample time")).unwrap();
        assert!(cancel.intake.is_none());
        assert_eq!(cancel.response.result, Ok(Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }));
        let mut h = driver.supervisor_mut().host_mut().unwrap();
        let revision = h.revision();
        assert_eq!(h.reconcile(revision, attempt).unwrap(), Reconciliation::Resolved(outcome));
        drop(h); commands.push(bytes);
    }
    std::fs::remove_file(root.0.join("private-evidence.json")).unwrap();
    let reads = source.status().read_attempts;
    for original in &commands {
        let retry = driver.exchange_stream_actor_from_file(&mut wire, original, &mut source,
            || panic!("retry must not sample time")).unwrap();
        assert!(retry.intake.is_none());
        assert!(matches!(retry.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    }
    assert_eq!(source.status().read_attempts, reads);
    let stream = driver.supervisor().host().unwrap().stream_snapshot().unwrap();
    assert!(stream.confirmed.finished());
    assert_eq!(stream.confirmed.messages().collect::<Vec<_>>(), vec!["source-backed α", "source-backed β"]);
}

#[test]
fn invalid_intents_foreign_gateways_fragments_and_write_flush_backpressure_never_pre_read() {
    let root = Directory::new(); let (port, mut driver, _, mut source, _) = create(&root);
    let original = stream::command(&driver.supervisor().host().unwrap(), 9000, Some("first"));
    let mut wire = ActorWire::new(port.clone()); let ticks = Cell::new(0);
    for case in 0..4 {
        let mut invalid = original.clone();
        if let Command::Submit { proposal, .. } = &mut invalid {
            match case {
                0 => proposal.payload[36] = 2, 1 => proposal.units += 1, 2 => proposal.payload[23] ^= 1,
                _ => { proposal.payload.resize(41 + stream::stream().max_message_bytes() + 1, 0);
                    proposal.units = proposal.payload.len() as u64; }
            }
        }
        let result = driver.exchange_stream_actor_from_file(&mut wire, &encode_command(&invalid).unwrap(),
            &mut source, || { ticks.set(ticks.get() + 1); ElapsedTick(1) }).unwrap();
        assert_eq!(result.response.result, Err(if case == 3 { WireError::Capacity } else { WireError::MalformedRequest }));
        assert!(result.intake.is_none());
    }
    assert_eq!(source.status().read_attempts, 0); assert_eq!(ticks.get(), 0);
    let foreign_root = Directory::new(); let (foreign, _other, _, _, _) = create(&foreign_root);
    let mut foreign_wire = ActorWire::new(foreign);
    assert!(matches!(driver.exchange_stream_actor_from_file(&mut foreign_wire, &encode_command(&original).unwrap(),
        &mut source, || panic!("foreign gateway must not sample time")), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(source.status().read_attempts, 0);
    let mut channel = ActorChannel::new(ActorWire::new(port), ChannelLimits::default()).unwrap();
    let bytes = stream::frame(&original); let end = bytes.len() - 1;
    let partial = driver.feed_stream_actor_from_file(&mut channel, &bytes[..end], &mut source,
        || panic!("fragment must not sample time")).unwrap();
    assert_eq!(partial.feed.consumed, end); assert!(partial.intake.is_none());
    assert_eq!(source.status().read_attempts, 0);
    let complete = driver.feed_stream_actor_from_file(&mut channel, &bytes[end..], &mut source, || ElapsedTick(1)).unwrap();
    assert!(complete.intake.unwrap().result.is_ok()); assert_eq!(source.status().read_attempts, 1);
    let poll = stream::frame(&Command::Poll { request: 9000 });
    let blocked = driver.feed_stream_actor_from_file(&mut channel, &poll, &mut source,
        || panic!("blocked response must not read")).unwrap();
    assert_eq!(blocked.feed.consumed, 0); assert!(blocked.intake.is_none());
    channel.acknowledge_written(channel.pending_output().len()).unwrap();
    assert_eq!(driver.feed_stream_actor_from_file(&mut channel, &poll, &mut source,
        || panic!("unflushed response must not read")).unwrap().feed.consumed, 0);
    channel.acknowledge_flushed().unwrap();
    let polled = driver.feed_stream_actor_from_file(&mut channel, &poll, &mut source,
        || panic!("poll must not read")).unwrap();
    assert!(polled.intake.is_none()); assert_eq!(source.status().read_attempts, 1);
    channel.acknowledge_written(channel.pending_output().len()).unwrap(); channel.acknowledge_flushed().unwrap();
    let cancelled = driver.feed_stream_actor_from_file(&mut channel, &stream::frame(&Command::Cancel { request: 9000 }),
        &mut source, || panic!("cancel must not read")).unwrap();
    assert!(cancelled.intake.is_none()); assert_eq!(source.status().read_attempts, 1);
    assert!(!driver.supervisor().host().unwrap().stream_snapshot().unwrap().published.finished());
    let response = channel.pending_output();
    assert!(matches!(decode_response(&response[..response.len() - 1]).unwrap().result,
        Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
}

#[test]
fn source_loss_during_next_wire_intake_withdraws_prior_dispatch_without_refunding_unknown_effects() {
    for missing in [false, true] {
        let root = Directory::new(); let (port, mut driver, human, mut source, evidence) = create(&root);
        let original = stream::command(&driver.supervisor().host().unwrap(), 7000, Some("pending disclosure"));
        let mut wire = ActorWire::new(port);
        let accepted = driver.exchange_stream_actor_from_file(&mut wire, &encode_command(&original).unwrap(),
            &mut source, || ElapsedTick(1)).unwrap();
        assert_eq!(accepted.response.result, Ok(Knowledge::Pending { request: 7000 }));
        let keys = review_dispatch(&mut driver, &human, &evidence, 7000, 1);
        let charge = driver.supervisor().host().unwrap().inspect().control.ledger.charged; assert!(charge > 0);
        let next = stream::command(&driver.supervisor().host().unwrap(), 7001, None);
        if missing { std::fs::remove_file(root.0.join("private-evidence.json")).unwrap(); }
        let result = driver.exchange_stream_actor_from_file(&mut wire, &encode_command(&next).unwrap(),
            &mut source, || ElapsedTick(1)).unwrap();
        assert_eq!(result.intake.unwrap().result.is_err(), missing);
        // The stream has an unresolved first dispatch even in the healthy-source
        // control. Neither case can turn this second intent into a finish effect.
        assert!(result.response.result.is_err());
        assert!(!std::str::from_utf8(&result.response.encode()).unwrap().contains("private-evidence"));
        assert!(matches!(driver.supervisor().host().unwrap().request_status(7001), Err(JournalError::Contract(Error::Missing))));
        let mut h = driver.supervisor_mut().host_mut().unwrap();
        assert_eq!(h.inspect().control.ledger.charged, charge);
        let revision = h.revision();
        let outcome = h.publish_checked(revision, 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap().outcome;
        assert_eq!(matches!(outcome, EndpointOutcome::Executed { .. }), !missing);
        assert_eq!(h.inspect().control.ledger.charged, charge);
        let revision = h.revision();
        assert_eq!(h.reconcile(revision, 1).unwrap(), Reconciliation::Resolved(outcome));
        assert_eq!(h.inspect().control.ledger.charged, if missing { 0 } else { charge });
        assert_eq!(h.inspect().executions, u64::from(!missing));
        assert!(!h.stream_snapshot().unwrap().published.finished());
    }
}

#[cfg(target_os = "linux")]
#[test]
fn original_kernel_peer_admission_and_reconnect_keep_stream_intake_and_retry_read_accounting() {
    use fa_reference::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy, PeerSession, PeerRefusal};
    use fa_reference::action::consequence::oversight::actor_transport::DriveBudget;
    use fa_reference::action::consequence::delivery::persistent::observed::stream::actor_wire::FileStreamPeerDriveError;
    use std::os::unix::net::UnixStream;
    use std::io::Write;
    let root = Directory::new(); let (port, mut driver, _, mut source, _) = create(&root);
    let command = stream::command(&driver.supervisor().host().unwrap(), 6000, Some("peer message"));
    let bytes = stream::frame(&command);
    let (peer, server) = UnixStream::pair().unwrap();
    let credentials = PeerCredentials::observe(&server).unwrap();
    let wrong_pid = if credentials.pid() == 1 { 2 } else { 1 };
    let wrong_policy = PeerPolicy::new(credentials.uid(), credentials.gid(), Some(wrong_pid)).unwrap();
    let mut denied = PeerSession::new(wrong_policy, ActorWire::new(port.clone()), ChannelLimits::default(), 2).unwrap();
    assert_eq!(denied.attach(server), Err(PeerRefusal::CredentialsRejected)); drop(peer);
    assert!(matches!(driver.drive_stream_peer_from_file(&mut denied, &mut source,
        || panic!("unattached peer must not read"), DriveBudget::default()), Err(FileStreamPeerDriveError::Wire(WireError::Unavailable))));
    assert_eq!(source.status().read_attempts, 0);
    let policy = PeerPolicy::new(credentials.uid(), credentials.gid(), Some(credentials.pid())).unwrap();
    let mut session = PeerSession::new(policy, ActorWire::new(port), ChannelLimits::default(), 2).unwrap();
    let (mut peer, server) = UnixStream::pair().unwrap(); peer.set_nonblocking(true).unwrap(); session.attach(server).unwrap();
    peer.write_all(&bytes[..bytes.len() - 1]).unwrap();
    let partial = driver.drive_stream_peer_from_file(&mut session, &mut source,
        || panic!("partial frame must not read"), DriveBudget::default()).unwrap();
    assert_eq!(partial.drive.progress.frames, 0); assert!(partial.intakes.is_empty());
    peer.write_all(b"\n").unwrap();
    let accepted = driver.drive_stream_peer_from_file(&mut session, &mut source, || ElapsedTick(1),
        DriveBudget { write_bytes: 0, ..DriveBudget::default() }).unwrap();
    assert_eq!(accepted.drive.progress.frames, 1); assert_eq!(accepted.intakes.len(), 1);
    assert!(accepted.intakes[0].result.is_ok()); assert_eq!(source.status().read_attempts, 1);
    assert!(session.disconnect()); drop(peer); std::fs::remove_file(root.0.join("private-evidence.json")).unwrap();
    let (mut peer, server) = UnixStream::pair().unwrap(); peer.set_nonblocking(true).unwrap(); session.attach(server).unwrap();
    peer.write_all(&bytes).unwrap();
    let retry = driver.drive_stream_peer_from_file(&mut session, &mut source,
        || panic!("recorded retry must not read"), DriveBudget::default()).unwrap();
    assert_eq!(retry.drive.progress.frames, 1); assert!(retry.intakes.is_empty());
    assert_eq!(source.status().read_attempts, 1); assert_eq!(driver.supervisor().host().unwrap().retained_requests(), 1);
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
}
