use super::*;
use super::super::{FilePrecheckChannel, MAX_PRECHECK_FRAME_BYTES, PrecheckWireError,
    decode_precheck_response};
use crate::action::consequence::oversight::actor_wire::{ChannelState, CloseReason};

fn reply(channel: &mut FilePrecheckChannel, query: u64) -> super::super::PrecheckWireResponse {
    let bytes = channel.pending_output();
    assert_eq!(bytes.last(), Some(&b'\n'));
    decode_precheck_response(&bytes[..bytes.len() - 1], query, 0).unwrap()
}

fn flush(channel: &mut FilePrecheckChannel) -> ChannelState {
    channel.acknowledge_written(channel.pending_output().len()).unwrap();
    channel.acknowledge_flushed().unwrap()
}

#[test]
fn fragments_and_flush_barrier_preserve_original_query_and_admission_state() {
    let (fixture, wire) = Fixture::new(2, 2);
    let before = fixture.supervisor.host().unwrap().inspect();
    let files = fixture.root.files();
    let reads = fixture.source.status().read_attempts;
    let mut channel = FilePrecheckChannel::new(wire, MAX_PRECHECK_FRAME_BYTES).unwrap();
    let first = frame(&request(901, b"ok"));
    let second = frame(&request(902, b"blocked"));
    let split = first.len() / 2;
    assert_eq!(channel.feed(&first[..split]).state, ChannelState::Reading);
    assert_eq!(fixture.remaining_queries(), 2);
    let suffix = [first[split..].to_vec(), second.clone()].concat();
    let fed = channel.feed(&suffix);
    assert_eq!(fed.consumed, first.len() - split);
    assert_eq!(fed.state, ChannelState::ReplyReady);
    assert_eq!(fixture.remaining_queries(), 1);
    known(reply(&mut channel, 901), FilePrecheckOutcome::PrecheckPassed, 901);
    let pending = channel.pending_output().len();
    assert_eq!(channel.acknowledge_flushed(), Err(PrecheckWireError::MalformedRequest));
    assert_eq!(channel.acknowledge_written(pending + 1), Err(PrecheckWireError::MalformedRequest));
    channel.acknowledge_written(pending).unwrap();
    assert!(channel.pending_output().is_empty());
    assert_eq!(channel.feed(&second).consumed, 0);
    assert_eq!(fixture.remaining_queries(), 1);
    assert_eq!(channel.acknowledge_flushed().unwrap(), ChannelState::Reading);
    assert_eq!(channel.feed(&second).consumed, second.len());
    known(reply(&mut channel, 902), FilePrecheckOutcome::DeniedByPolicy, 902);
    assert_eq!(flush(&mut channel), ChannelState::Closed(CloseReason::ExchangeLimit));
    assert_eq!(fixture.remaining_queries(), 0);
    assert_eq!(fixture.supervisor.host().unwrap().inspect(), before);
    assert_eq!(fixture.root.files(), files);
    assert_eq!(fixture.source.status().read_attempts, reads);
    // The read label is not a submission ID or a reserved effect. The original
    // admission snapshot also remains untouched and ordinary review is required.
    let ticket = fixture.actor.submit(901, &request(901, b"ok").proposal).unwrap();
    assert_eq!(fixture.actor.poll(&ticket), Knowledge::Pending { request: 901 });
    let state = fixture.supervisor.host().unwrap().inspect();
    assert_eq!(state.control.ledger.stages[&1], ActionState::Reviewing);
    assert_eq!((state.control.ledger.available, state.control.ledger.reserved, state.executions), (100, 0, 0));
    assert!(state.control.decisions.is_empty());
}

#[test]
fn exact_frame_bound_succeeds_but_overrun_and_unterminated_frames_never_evaluate() {
    let (fixture, wire) = Fixture::new(2, 2);
    let mut channel = FilePrecheckChannel::new(wire, MAX_PRECHECK_FRAME_BYTES).unwrap();
    let mut exact = encode_precheck_request(&request(11, b"ok")).unwrap();
    exact.resize(MAX_PRECHECK_FRAME_BYTES, b' ');
    exact.push(b'\n');
    assert_eq!(channel.feed(&exact).consumed, MAX_PRECHECK_FRAME_BYTES + 1);
    known(reply(&mut channel, 11), FilePrecheckOutcome::PrecheckPassed, 11);
    flush(&mut channel);
    assert_eq!(fixture.remaining_queries(), 1);
    let overrun = vec![b' '; MAX_PRECHECK_FRAME_BYTES + 1];
    assert_eq!(channel.feed(&overrun).state, ChannelState::Closed(CloseReason::FrameTooLarge));
    assert_eq!(fixture.remaining_queries(), 1);
    assert!(channel.pending_output().is_empty());
    let wire = channel.into_wire();
    assert_eq!(wire.remaining_exchanges(), 1);
    let mut channel = FilePrecheckChannel::new(wire, MAX_PRECHECK_FRAME_BYTES).unwrap();
    let line = frame(&request(12, b"ok"));
    channel.feed(&line[..line.len() - 1]);
    assert_eq!(channel.finish_input(), ChannelState::Closed(CloseReason::TruncatedFrame));
    assert_eq!(fixture.remaining_queries(), 1);
    assert_eq!(channel.into_wire().remaining_exchanges(), 1);
}

#[test]
fn malformed_and_cross_protocol_frames_spend_parser_allowance_not_evaluations() {
    let (fixture, wire) = Fixture::new(2, 2);
    let mut channel = FilePrecheckChannel::new(wire, MAX_PRECHECK_FRAME_BYTES).unwrap();
    for document in [b"not-json\n".as_slice(),
        b"{\"schema\":\"fa.actor/1\",\"command\":\"poll\",\"request\":\"1\"}\n".as_slice()] {
        assert_eq!(channel.feed(document).state, ChannelState::ReplyReady);
        let response = reply(&mut channel, 1);
        assert_eq!(response.query, None);
        assert!(matches!(response.result, Err(PrecheckWireError::MalformedRequest | PrecheckWireError::UnsupportedVersion)));
        flush(&mut channel);
    }
    assert_eq!(fixture.remaining_queries(), 2);
    let wire = channel.into_wire();
    assert_eq!(wire.remaining_exchanges(), 0);
    let mut renewed_channel = FilePrecheckChannel::new(wire, MAX_PRECHECK_FRAME_BYTES).unwrap();
    assert_eq!(renewed_channel.state(), ChannelState::Closed(CloseReason::ExchangeLimit));
    assert_eq!(renewed_channel.feed(&frame(&request(1, b"ok"))).consumed, 0);
    assert_eq!(fixture.remaining_queries(), 2);
}

#[test]
fn eof_can_drain_one_completed_response_but_debug_never_contains_frames_or_replies() {
    let (fixture, wire) = Fixture::new(2, 2);
    let mut channel = FilePrecheckChannel::new(wire, MAX_PRECHECK_FRAME_BYTES).unwrap();
    channel.feed(&frame(&request(55, b"SECR")));
    let debug = format!("{channel:?}");
    for secret in ["SECR", "53454352", "precheck_passed", "review_required", "private review input"] {
        assert!(!debug.contains(secret));
    }
    assert_eq!(channel.finish_input(), ChannelState::ReplyReady);
    known(reply(&mut channel, 55), FilePrecheckOutcome::PrecheckPassed, 55);
    assert_eq!(flush(&mut channel), ChannelState::Closed(CloseReason::PeerClosed));
    assert_eq!(channel.feed(&frame(&request(56, b"ok"))).consumed, 0);
    assert_eq!(fixture.remaining_queries(), 1);
}

#[test]
fn framing_configuration_cannot_widen_codec_limits_or_create_a_zero_allowance() {
    for (limit, expected) in [(0, PrecheckWireError::MalformedRequest),
        (MAX_PRECHECK_FRAME_BYTES + 1, PrecheckWireError::Capacity)] {
        let (fixture, wire) = Fixture::new(1, 1);
        assert_eq!(FilePrecheckChannel::new(wire, limit).err(), Some(expected));
        assert_eq!(fixture.remaining_queries(), 1);
    }
    let (fixture, wire) = Fixture::new(1, 1);
    let bytes = frame(&request(1, b"ok"));
    let mut channel = FilePrecheckChannel::new(wire, bytes.len() - 1).unwrap();
    assert_eq!(channel.feed(&bytes).state, ChannelState::ReplyReady);
    known(reply(&mut channel, 1), FilePrecheckOutcome::PrecheckPassed, 1);
    assert_eq!(fixture.remaining_queries(), 0);
}
