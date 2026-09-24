//! Original JSON codec/channel/Unix transport with the source-only actor backend.
use super::*;
use crate::action::consequence::delivery::persistent::requests::actor::FileGeneratedTextActorPort;
use crate::action::consequence::oversight::actor_transport::{DriveBudget, UnixActorConnection};
use crate::action::consequence::oversight::actor_wire::{ActorChannel, ActorWire, ChannelLimits,
    ChannelState, CloseReason, Command, WireError, WireResponse, decode_response, encode_command};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

type Connection = UnixActorConnection<FileGeneratedTextActorPort>;
fn submit_command(source: &FileTextMessageRequest) -> Command {
    Command::Submit { request: source.request, proposal: FileGeneratedTextActorPort::encode_message(source).unwrap() }
}
fn framed(command: &Command) -> Vec<u8> {
    let mut bytes = encode_command(command).unwrap(); bytes.push(b'\n'); bytes
}
fn connection(port: FileGeneratedTextActorPort) -> (Connection, UnixStream) {
    let (host, peer) = UnixStream::pair().unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    peer.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
    let channel = ActorChannel::new(ActorWire::new(port), ChannelLimits { frame_bytes: 2048, exchanges: 32 }).unwrap();
    (UnixActorConnection::new(host, channel).unwrap(), peer)
}
fn read_response(peer: &mut UnixStream) -> WireResponse {
    let mut output = Vec::new();
    for _ in 0..513 {
        let mut byte = [0]; peer.read_exact(&mut byte).unwrap();
        if byte[0] == b'\n' { return decode_response(&output).unwrap(); }
        output.push(byte[0]);
    }
    panic!("original response bound exceeded");
}
fn exchange(connection: &mut Connection, peer: &mut UnixStream, command: Command) -> WireResponse {
    peer.write_all(&framed(&command)).unwrap();
    let report = connection.drive(DriveBudget::default()).unwrap();
    assert_eq!(report.progress.frames, 1); assert!(report.progress.written_bytes > 0);
    read_response(peer)
}

#[test]
fn generated_actor_wire_sessions_reacquire_only_exact_source_tickets() {
    let root = Directory::new(); let (host, _) = ready(&root); let source = submission(&host, 91, 7);
    let (port, mut supervisor) = host.into_generated_text_actor_gateway().unwrap();
    let mut original = ActorWire::new(port.clone()); let mut newcomer = ActorWire::new(port);
    let poll = encode_command(&Command::Poll { request: 91 }).unwrap();
    assert!(matches!(newcomer.exchange(&poll).result, Ok(Knowledge::Withheld { .. })));
    observed(&mut supervisor);
    let document = encode_command(&submit_command(&source)).unwrap();
    assert_eq!(original.exchange(&document).result, Ok(Knowledge::Pending { request: 91 }));
    let before = bytes(&supervisor.host().unwrap());
    let mut conflicting = source.clone(); conflicting.generation += 1;
    assert_eq!(newcomer.exchange(&encode_command(&submit_command(&conflicting)).unwrap()).result,
        Err(WireError::IdempotencyConflict));
    assert!(matches!(newcomer.exchange(&poll).result, Ok(Knowledge::Withheld { .. })));
    assert_eq!(newcomer.exchange(&document).result, Ok(Knowledge::Pending { request: 91 }));
    assert_eq!(bytes(&supervisor.host().unwrap()), before);
    let response = newcomer.exchange(&encode_command(&Command::Cancel { request: 91 }).unwrap());
    assert!(matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    assert_eq!(original.exchange(&poll).result, response.result);
    assert_eq!(supervisor.host().unwrap().decoder_text_generation(7).unwrap().result().unwrap().generation().finish(), GenerationFinish::StopToken);
}

#[test]
fn generated_actor_wire_malformed_intents_leave_the_observation_for_the_valid_source() {
    let root = Directory::new(); let (host, _) = ready(&root); let source = submission(&host, 91, 7);
    let (port, mut supervisor) = host.into_generated_text_actor_gateway().unwrap();
    observed(&mut supervisor); let mut wire = ActorWire::new(port);
    let before = bytes(&supervisor.host().unwrap());
    for mode in 0..7 {
        let mut proposal = FileGeneratedTextActorPort::encode_message(&source).unwrap();
        match mode {
            0 => proposal.payload[0] ^= 1,
            1 => { proposal.payload.pop(); }
            2 => { proposal.payload.push(0); proposal.units += 1; }
            3 => proposal.payload[8] ^= 1,
            4 => proposal.units += 1,
            5 => { proposal.payload[16..24].fill(0); proposal.payload[31] = 1; }
            _ => { proposal.payload = b"replacement".to_vec(); }
        }
        let command = Command::Submit { request: 91, proposal };
        let response = wire.exchange(&encode_command(&command).unwrap());
        assert_eq!(response.result, Err(WireError::MalformedRequest));
        assert_eq!(bytes(&supervisor.host().unwrap()), before);
    }
    let mut hidden_field = encode_command(&submit_command(&source)).unwrap();
    hidden_field.pop(); hidden_field.extend_from_slice(b",\"approve\":true}");
    assert_eq!(wire.exchange(&hidden_field).result, Err(WireError::MalformedRequest));
    let mut wrong_version = encode_command(&submit_command(&source)).unwrap();
    let version = b"\"version\":1";
    let at = wrong_version.windows(version.len()).position(|bytes| bytes == version).unwrap();
    wrong_version[at + version.len() - 1] = b'2';
    assert_eq!(wire.exchange(&wrong_version).result, Err(WireError::UnsupportedVersion));
    assert_eq!(wire.exchange(&encode_command(&submit_command(&source)).unwrap()).result,
        Ok(Knowledge::Pending { request: 91 }));
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
}

#[test]
fn generated_actor_wire_preserves_channel_backpressure_eof_and_exact_frame_bounds() {
    for under in [true, false] {
        let root = Directory::new(); let (host, _) = ready(&root); let source = submission(&host, 91, 7);
        let (port, mut supervisor) = host.into_generated_text_actor_gateway().unwrap();
        observed(&mut supervisor);
        let request = framed(&submit_command(&source)); let before = bytes(&supervisor.host().unwrap());
        let mut channel = ActorChannel::new(ActorWire::new(port.clone()), ChannelLimits {
            frame_bytes: request.len() - 1 - usize::from(under), exchanges: 8,
        }).unwrap();
        if under {
            assert_eq!(channel.feed(&request).state, ChannelState::Closed(CloseReason::FrameTooLarge));
            assert_eq!(bytes(&supervisor.host().unwrap()), before);
            // A complete JSON document still needs its newline. EOF cannot admit
            // its source reference or consume the observation held by the owner.
            let mut incomplete = ActorChannel::new(ActorWire::new(port.clone()), ChannelLimits::default()).unwrap();
            assert_eq!(incomplete.feed(&request[..request.len() - 1]).state, ChannelState::Reading);
            assert_eq!(incomplete.finish_input(), ChannelState::Closed(CloseReason::TruncatedFrame));
            assert_eq!(bytes(&supervisor.host().unwrap()), before);
            channel = ActorChannel::new(ActorWire::new(port), ChannelLimits::default()).unwrap();
        }
        let cancel = framed(&Command::Cancel { request: 91 });
        let mut pipeline = request.clone(); pipeline.extend_from_slice(&cancel);
        let first = channel.feed(&pipeline);
        assert_eq!(first.consumed, request.len()); assert_eq!(first.state, ChannelState::ReplyReady);
        let admitted = bytes(&supervisor.host().unwrap());
        assert_eq!(channel.feed(&cancel).consumed, 0);
        let remaining = channel.pending_output().len(); channel.acknowledge_written(remaining).unwrap();
        assert_eq!(channel.feed(&cancel).consumed, 0); // no early acceptance before flush
        assert_eq!(bytes(&supervisor.host().unwrap()), admitted);
        channel.acknowledge_flushed().unwrap();
        assert_eq!(channel.feed(&cancel).consumed, cancel.len());
        assert!(matches!(decode_response(channel.pending_output().strip_suffix(b"\n").unwrap()).unwrap().result,
            Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
        assert_eq!(supervisor.host().unwrap().decoder_inspection().unwrap().numerical.sampled_draws, 2);
    }
}

#[test]
fn generated_actor_wire_unix_lost_reply_rejoins_the_original_two_key_publication() {
    let root = Directory::new(); let (host, human) = ready(&root); let source = submission(&host, 91, 7);
    let numerical = host.decoder_inspection().unwrap().numerical;
    let (port, mut supervisor) = host.into_generated_text_actor_gateway().unwrap();
    observed(&mut supervisor);
    let (mut transport, mut peer) = connection(port.clone());
    let input = framed(&submit_command(&source)); let before = bytes(&supervisor.host().unwrap());
    peer.write_all(&input[..input.len() - 1]).unwrap();
    let read_only = DriveBudget { read_bytes: 2048, write_bytes: 0, frames: 1, io_calls: 8 };
    assert_eq!(transport.drive(read_only).unwrap().progress.frames, 0);
    assert_eq!(bytes(&supervisor.host().unwrap()), before);
    peer.write_all(b"\n").unwrap();
    let admitted = transport.drive(read_only).unwrap();
    assert_eq!(admitted.progress.frames, 1); assert_eq!(admitted.progress.written_bytes, 0);
    assert!(admitted.status.pending_output_bytes > 0);
    let before_retry = bytes(&supervisor.host().unwrap());
    drop(transport); drop(peer); // the original command was admitted; its reply was not sent
    let (mut transport, mut peer) = connection(port);
    assert!(matches!(exchange(&mut transport, &mut peer, Command::Poll { request: 91 }).result,
        Ok(Knowledge::Withheld { .. })));
    assert_eq!(exchange(&mut transport, &mut peer, submit_command(&source)).result,
        Ok(Knowledge::Pending { request: 91 }));
    assert_eq!(bytes(&supervisor.host().unwrap()), before_retry);
    let keys = { let mut host = supervisor.host_mut().unwrap(); review(&mut host, &human, 91) };
    {
        let mut host = supervisor.host_mut().unwrap(); let revision = host.revision();
        host.dispatch(revision, &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).unwrap();
    }
    assert!(matches!(exchange(&mut transport, &mut peer, Command::Poll { request: 91 }).result,
        Ok(Knowledge::Unknown { .. })));
    {
        let mut host = supervisor.host_mut().unwrap(); let revision = host.revision();
        host.publish_checked(revision, keys.automatic.attempt(), Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
        assert_eq!(host.stream_snapshot().unwrap().published.messages().collect::<Vec<_>>(), ["A"]);
        assert_eq!(host.stream_snapshot().unwrap().confirmed.message_count(), 0);
    }
    assert!(matches!(exchange(&mut transport, &mut peer, Command::Cancel { request: 91 }).result,
        Ok(Knowledge::Unknown { .. }))); // cancellation cannot undo the observed publication
    {
        let mut host = supervisor.host_mut().unwrap(); let revision = host.revision();
        host.reconcile(revision, keys.automatic.attempt()).unwrap();
        assert_eq!(host.inspect().control.ledger.charged, keys.action.spec().units);
        assert!(keys.action.spec().units > FileGeneratedTextActorPort::INTENT_BYTES as u64);
        assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    }
    let response = exchange(&mut transport, &mut peer, Command::Poll { request: 91 });
    assert!(matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert_eq!(exchange(&mut transport, &mut peer, submit_command(&source)).result, response.result);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 1);
}

#[test]
fn generated_actor_wire_finish_is_explicit_and_still_needs_the_original_two_keys() {
    let root = Directory::new(); let (host, human) = ready(&root); let source = submission(&host, 91, 7);
    let expected = host.stream_finish_spec(source.deadline).unwrap();
    let (port, mut supervisor) = host.into_generated_text_actor_gateway().unwrap();
    observed(&mut supervisor);
    let (mut transport, mut peer) = connection(port);
    let proposal = FileGeneratedTextActorPort::encode_finish(91, source.target,
        source.policy_epoch, source.deadline).unwrap();
    let command = Command::Submit { request: 91, proposal };
    assert_eq!(exchange(&mut transport, &mut peer, command.clone()).result, Ok(Knowledge::Pending { request: 91 }));
    assert_eq!(supervisor.host().unwrap().request_action(91).unwrap().spec(), &expected);
    assert!(!supervisor.host().unwrap().stream_snapshot().unwrap().published.finished());
    let keys = { let mut host = supervisor.host_mut().unwrap(); review(&mut host, &human, 91) };
    {
        let mut host = supervisor.host_mut().unwrap(); let revision = host.revision();
        host.dispatch(revision, &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).unwrap();
        let revision = host.revision();
        host.publish_checked(revision, keys.automatic.attempt(), Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
        let revision = host.revision(); host.reconcile(revision, keys.automatic.attempt()).unwrap();
        let cut = host.stream_snapshot().unwrap(); assert!(cut.published.finished());
        assert_eq!(cut.published, cut.confirmed); assert!(cut.published.visible().is_empty());
    }
    let done = exchange(&mut transport, &mut peer, command);
    assert!(matches!(done.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert_eq!(exchange(&mut transport, &mut peer, submit_command(&source)).result, Err(WireError::IdempotencyConflict));
    assert_eq!(supervisor.host().unwrap().decoder_text_generation(7).unwrap().result().unwrap().bytes().unwrap(), b"A");
}
