//! Real durable requests and the original wire/Unix transport, not a fake backend.
#![cfg(unix)]
#[path = "support/file_stream_wire.rs"] mod support;
use support::*;
use fa_reference::action::{ElapsedTick, ActionState};
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use fa_reference::action::consequence::delivery::stream::ReleaseFrame;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, ActorChannel, ChannelLimits, Command, WireError, encode_command, decode_response};
use fa_reference::action::consequence::oversight::actor_transport::{UnixActorConnection, DriveBudget};
use fa_reference::Error;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

fn pending(value: fa_reference::action::consequence::oversight::actor_wire::WireResponse, request: u64) {
    assert_eq!(value.result, Ok(Knowledge::Pending { request }));
}

#[test]
fn two_messages_and_explicit_finish_review_entire_prefix_and_charge_native_frames() {
    let root = Directory::new(); let (port, mut supervisor, human) = create(&root);
    let mut wire = ActorWire::new(port); let mut commands = Vec::new(); let mut charge = 0;
    let mut prior = Vec::<String>::new();
    for (offset, message) in [Some("first é\n"), Some("second 🦀"), None].into_iter().enumerate() {
        let attempt = offset as u64 + 1; let request = 9000 + attempt;
        let command = command(&supervisor.host().unwrap(), request, message);
        let encoded = encode_command(&command).unwrap(); admit(&mut supervisor);
        pending(wire.exchange(&encoded), request);
        {
            let mut h = supervisor.host_mut().unwrap();
            let action = h.request_action(request).unwrap().clone();
            let frame = ReleaseFrame::decode(&action.spec().payload).unwrap();
            assert_eq!(frame.message(), message);
            assert_eq!(frame.prior_messages(), prior.iter().map(String::as_str).collect::<Vec<_>>().as_slice());
            assert_eq!(action.spec().units, action.spec().payload.len() as u64);
            if let Command::Submit { proposal, .. } = &command {
                assert!(action.spec().units > proposal.units);
                assert_ne!(action.spec().payload, proposal.payload);
            }
            charge += action.spec().units;
            let keys = approve(&mut h, &human, request, attempt);
            ordinary::dispatch(&mut h, &keys);
            let revision = h.revision();
            let result = h.publish_checked(revision, attempt, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
            assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: attempt + 1 });
            // Native receipt confirmation is deliberately a later transition.
            assert_eq!(h.inspect().control.ledger.stages[&attempt], ActionState::Dispatching);
        }
        assert_eq!(wire.exchange(&encode_command(&Command::Poll { request }).unwrap()).result,
            Ok(Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }));
        {
            let mut h = supervisor.host_mut().unwrap();
            let revision = h.revision();
            assert_eq!(h.reconcile(revision, attempt).unwrap(),
                Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: attempt + 1 }));
            assert_eq!(h.inspect().control.ledger.charged, charge);
        }
        assert!(matches!(wire.exchange(&encoded).result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
        commands.push(encoded); if let Some(message) = message { prior.push(message.to_owned()); }
    }
    let state = supervisor.host().unwrap().stream_snapshot().unwrap();
    assert!(state.confirmed.finished() && state.published.finished());
    assert_eq!(state.confirmed.messages().collect::<Vec<_>>(), vec!["first é\n", "second 🦀"]);
    let before = supervisor.host().unwrap().inspect();
    for original in commands {
        assert!(matches!(wire.exchange(&original).result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    }
    assert_eq!(supervisor.host().unwrap().inspect(), before);
}

#[test]
fn retained_refusal_and_recovery_retry_do_not_rebuild_against_new_epoch_or_require_snapshot() {
    let root = Directory::new(); let mut p = profile(); p.delivery.total = 1;
    let (mut h, _) = FileOversight::create_stream(root.store(), p.clone(), stream()).unwrap();
    h.observe_time(h.revision(), ElapsedTick(1)).unwrap();
    let original = command(&h, 9000, Some("cannot afford")); let encoded = encode_command(&original).unwrap();
    let (port, mut supervisor) = h.into_stream_actor_gateway().unwrap();
    let mut wire = ActorWire::new(port); admit(&mut supervisor);
    assert!(matches!(wire.exchange(&encoded).result, Ok(Knowledge::Known { value: ActorOutcome::NotAdmitted, .. })));
    assert!(matches!(supervisor.host().unwrap().request_status(9000).unwrap().disposition, FileRequestDisposition::NotAdmitted(_)));
    drop(supervisor);
    assert!(matches!(wire.exchange(&encoded).result, Err(WireError::Unavailable)));
    let (h, _) = FileOversight::open_stream(root.store(), p, stream()).unwrap();
    let (port, supervisor) = h.into_stream_actor_gateway().unwrap(); let mut fresh = ActorWire::new(port);
    assert!(!supervisor.host().unwrap().clock_ready()); let before = supervisor.host().unwrap().inspect();
    assert!(matches!(fresh.exchange(&encode_command(&Command::Poll { request: 9000 }).unwrap()).result,
        Ok(Knowledge::Withheld { .. })));
    assert!(matches!(fresh.exchange(&encoded).result, Ok(Knowledge::Known { value: ActorOutcome::NotAdmitted, .. })));
    let mut conflict = original;
    if let Command::Submit { proposal, .. } = &mut conflict { *proposal.payload.last_mut().unwrap() ^= 1; }
    assert_eq!(fresh.exchange(&encode_command(&conflict).unwrap()).result, Err(WireError::IdempotencyConflict));
    assert_eq!(supervisor.host().unwrap().inspect(), before);
}

#[test]
fn raw_frames_bad_units_and_foreign_profiles_refuse_without_consuming_admission_snapshot() {
    let root = Directory::new(); let (port, mut supervisor, _) = create(&root);
    let mut wire = ActorWire::new(port);
    let valid = command(&supervisor.host().unwrap(), 9000, Some("valid"));
    admit(&mut supervisor); let before = supervisor.host().unwrap().inspect();
    for case in 0..4 {
        let mut malformed = valid.clone();
        if let Command::Submit { proposal, .. } = &mut malformed {
            match case {
                0 => { proposal.payload = b"raw message".to_vec(); proposal.units = proposal.payload.len() as u64; }
                1 => proposal.units += 1,
                2 => proposal.payload[23] ^= 1,
                _ => { proposal.payload = supervisor.host().unwrap().stream_message_spec("raw frame", ElapsedTick(100)).unwrap().payload;
                    proposal.units = proposal.payload.len() as u64; }
            }
        }
        assert_eq!(wire.exchange(&encode_command(&malformed).unwrap()).result, Err(WireError::MalformedRequest));
        assert_eq!(supervisor.host().unwrap().inspect(), before);
    }
    pending(wire.exchange(&encode_command(&valid).unwrap()), 9000);
    assert!(supervisor.host().unwrap().request_action(9000).is_ok());
    let ordinary_root = Directory::new(); let (plain, _) = ordinary::create(&ordinary_root);
    assert!(plain.into_stream_actor_gateway().is_err());
}

#[test]
fn original_nonblocking_socket_fragments_reconnect_and_eof_do_not_publish_or_finish() {
    let root = Directory::new(); let (port, mut supervisor, human) = create(&root);
    let command = command(&supervisor.host().unwrap(), 7000, Some("socket message")); let bytes = frame(&command);
    let (mut peer, server) = UnixStream::pair().unwrap(); peer.set_nonblocking(true).unwrap();
    let mut connection = UnixActorConnection::new(server,
        ActorChannel::new(ActorWire::new(port), ChannelLimits::default()).unwrap()).unwrap();
    admit(&mut supervisor);
    peer.write_all(&bytes[..bytes.len() - 1]).unwrap();
    assert_eq!(connection.drive(DriveBudget::default()).unwrap().progress.frames, 0);
    assert!(matches!(supervisor.host().unwrap().request_status(7000), Err(JournalError::Contract(Error::Missing))));
    peer.write_all(b"\n").unwrap();
    let paused = connection.drive(DriveBudget { write_bytes: 0, ..DriveBudget::default() }).unwrap();
    assert_eq!(paused.progress.frames, 1); assert!(paused.status.pending_output_bytes > 0);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
    let mut wire = connection.into_session(); drop(peer);
    let before = supervisor.host().unwrap().inspect();
    pending(wire.exchange(&encode_command(&command).unwrap()), 7000);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    let keys = approve(&mut supervisor.host_mut().unwrap(), &human, 7000, 1);
    {
        let mut h = supervisor.host_mut().unwrap(); ordinary::dispatch(&mut h, &keys);
        let revision = h.revision();
        h.publish_checked(revision, 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
        let revision = h.revision();
        h.reconcile(revision, 1).unwrap();
    }
    let (mut peer, server) = UnixStream::pair().unwrap(); peer.set_nonblocking(true).unwrap();
    let mut connection = UnixActorConnection::new(server, ActorChannel::new(wire, ChannelLimits::default()).unwrap()).unwrap();
    peer.write_all(&frame(&Command::Poll { request: 7000 })).unwrap();
    let mut reply = Vec::new();
    for _ in 0..16 {
        connection.drive(DriveBudget::default()).unwrap();
        let mut part = [0_u8; 513];
        match peer.read(&mut part) {
            Ok(count) if count > 0 => reply.extend_from_slice(&part[..count]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            other => panic!("unexpected socket reply: {other:?}"),
        }
        if reply.last() == Some(&b'\n') { break; }
    }
    assert_eq!(reply.last(), Some(&b'\n'));
    assert!(matches!(decode_response(&reply[..reply.len() - 1]).unwrap().result,
        Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    drop(peer); connection.drive(DriveBudget::default()).unwrap();
    assert!(!supervisor.host().unwrap().stream_snapshot().unwrap().published.finished());
    assert_eq!(supervisor.host().unwrap().stream_snapshot().unwrap().published.messages().collect::<Vec<_>>(), vec!["socket message"]);
}
