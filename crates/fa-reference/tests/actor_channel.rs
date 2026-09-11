//! Bounded framing, transport interruption, and delivery through a real socket.
#[path = "support/actor_gateway.rs"]
mod support;

use fa_reference::action::consequence::oversight::actor::{ActorOutcome, IntakeLimits, Knowledge};
use fa_reference::action::consequence::oversight::actor_wire::{ActorChannel, ActorWire, ChannelLimits, ChannelState, CloseReason, Command, MAX_FRAME_BYTES, WireError, encode_command};
use fa_reference::action::consequence::oversight::DispatchKeys;
use fa_reference::strict_json::{self, Json, Limits};
use std::io::{self, Cursor, Write};
use support::{fixture, proposal, review, snapshot};

fn frame(command: Command) -> Vec<u8> { let mut bytes = encode_command(&command).unwrap(); bytes.push(b'\n'); bytes }
fn submit(id: u64) -> Vec<u8> { frame(Command::Submit { request: id, proposal: proposal() }) }
fn new_channel(port: fa_reference::action::consequence::oversight::actor::ActorPort) -> ActorChannel {
    ActorChannel::new(ActorWire::new(port), ChannelLimits::default()).unwrap()
}
fn drain(channel: &mut ActorChannel) -> Vec<u8> {
    let mut bytes = Vec::new();
    assert_ne!(channel.write_once(&mut bytes).unwrap(), ChannelState::ReplyReady);
    bytes
}
fn json(bytes: &[u8]) -> Json { strict_json::parse(bytes, Limits::default()).unwrap() }

#[test]
fn every_fragment_boundary_and_bytewise_input_submit_only_after_newline() {
    let request = submit(42);
    for split in 0..request.len() {
        let (port, mut supervisor, _) = fixture(IntakeLimits::default());
        let mut channel = new_channel(port);
        let first = channel.feed(&request[..split]);
        assert_eq!(first.consumed, split); assert_eq!(first.state, ChannelState::Reading);
        assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());
        let second = channel.feed(&request[split..]);
        assert_eq!(second.consumed, request.len() - split); assert_eq!(second.state, ChannelState::ReplyReady);
        assert_eq!(supervisor.accept_next(&snapshot()).unwrap().unwrap().request, 42);
        assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());
        assert_eq!(json(&drain(&mut channel)).get("request").unwrap().as_str(), Some("42"));
    }
    let (port, mut supervisor, _) = fixture(IntakeLimits::default());
    let mut channel = new_channel(port);
    for byte in &request { assert_eq!(channel.feed(&[*byte]).consumed, 1); }
    assert_eq!(supervisor.accept_next(&snapshot()).unwrap().unwrap().request, 42);
    assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());
}

#[test]
fn backpressure_retains_pipelined_input_until_previous_response_is_flushed() {
    let (port, mut supervisor, _) = fixture(IntakeLimits::default());
    let mut channel = new_channel(port);
    let first = submit(42); let second = submit(43);
    let both = [first.as_slice(), second.as_slice()].concat();
    let progress = channel.feed(&both);
    assert_eq!(progress.consumed, first.len());
    assert_eq!(channel.feed(&both[progress.consumed..]).consumed, 0);
    assert_eq!(supervisor.accept_next(&snapshot()).unwrap().unwrap().request, 42);
    assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());
    let output = channel.pending_output().to_vec();
    assert_eq!(channel.acknowledge_flushed(), Err(WireError::MalformedRequest));
    assert_eq!(channel.acknowledge_written(output.len() + 1), Err(WireError::MalformedRequest));
    assert_eq!(channel.pending_output(), output);
    channel.acknowledge_written(output.len()).unwrap();
    assert_eq!(channel.feed(&second).consumed, 0);
    assert_eq!(channel.acknowledge_flushed().unwrap(), ChannelState::Reading);
    assert_eq!(channel.feed(&second).consumed, second.len());
    assert_eq!(supervisor.accept_next(&snapshot()).unwrap().unwrap().request, 43);
}

#[test]
fn clean_eof_truncation_and_one_over_limit_never_parse_a_prefix_as_a_command() {
    let request = submit(42);
    let (port, mut supervisor, _) = fixture(IntakeLimits::default());
    let mut partial = new_channel(port.clone());
    partial.feed(&request[..request.len() - 1]);
    assert_eq!(partial.finish_input(), ChannelState::Closed(CloseReason::TruncatedFrame));
    assert_eq!(partial.feed(b"\n").consumed, 0);
    assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());
    let mut exact = ActorChannel::new(partial.into_wire(), ChannelLimits { frame_bytes: request.len() - 1, exchanges: 2 }).unwrap();
    assert_eq!(exact.feed(&request).state, ChannelState::ReplyReady);
    drain(&mut exact);
    let mut too_small = ActorChannel::new(exact.into_wire(), ChannelLimits { frame_bytes: request.len() - 2, exchanges: 2 }).unwrap();
    assert_eq!(too_small.feed(&submit(43)).state, ChannelState::Closed(CloseReason::FrameTooLarge));
    assert_eq!(supervisor.accept_next(&snapshot()).unwrap().unwrap().request, 42);
    assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());
    let mut empty = new_channel(port);
    assert_eq!(empty.finish_input(), ChannelState::Closed(CloseReason::PeerClosed));
}

#[test]
fn invalid_frames_spend_exchange_budget_without_actor_admission() {
    let (port, mut supervisor, _) = fixture(IntakeLimits::default());
    let mut channel = ActorChannel::new(ActorWire::new(port), ChannelLimits { frame_bytes: MAX_FRAME_BYTES, exchanges: 2 }).unwrap();
    let bytes = [b"{}\n{}\n".as_slice(), submit(42).as_slice()].concat();
    assert_eq!(channel.feed(&bytes).consumed, 3);
    assert_eq!(json(&drain(&mut channel)).get("status").unwrap().as_str(), Some("error"));
    assert_eq!(channel.feed(&bytes[3..]).consumed, 3);
    drain(&mut channel);
    assert_eq!(channel.state(), ChannelState::Closed(CloseReason::ExchangeLimit));
    assert_eq!(channel.feed(&submit(42)).consumed, 0);
    assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());
}

struct ShortWriter { bytes: Vec<u8>, cap: usize, block_write: bool, block_flush: bool }
impl Write for ShortWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.block_write { self.block_write = false; return Err(io::ErrorKind::WouldBlock.into()); }
        let size = bytes.len().min(self.cap); self.bytes.extend_from_slice(&bytes[..size]); Ok(size)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.block_flush { self.block_flush = false; return Err(io::ErrorKind::WouldBlock.into()); }
        Ok(())
    }
}

#[test]
fn partial_writes_and_blocked_flush_resume_without_replaying_the_request_or_response() {
    let (port, mut supervisor, _) = fixture(IntakeLimits::default());
    let mut channel = new_channel(port);
    channel.feed(&submit(42));
    let expected = channel.pending_output().to_vec();
    let mut writer = ShortWriter { bytes: Vec::new(), cap: 3, block_write: true, block_flush: true };
    assert_eq!(channel.write_once(&mut writer).unwrap_err().kind(), io::ErrorKind::WouldBlock);
    assert_eq!(channel.pending_output(), expected);
    for _ in 0..expected.len() {
        if let Err(error) = channel.write_once(&mut writer) { assert_eq!(error.kind(), io::ErrorKind::WouldBlock); break; }
    }
    assert!(channel.pending_output().is_empty());
    assert_eq!(channel.state(), ChannelState::ReplyReady);
    assert_eq!(channel.feed(&submit(43)).consumed, 0);
    assert_eq!(channel.write_once(&mut writer).unwrap(), ChannelState::Reading);
    assert_eq!(writer.bytes, expected);
    assert_eq!(supervisor.accept_next(&snapshot()).unwrap().unwrap().request, 42);
    assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());
}

struct BrokenWriter;
impl Write for BrokenWriter {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

#[test]
fn broken_reply_and_reconnection_do_not_drop_or_duplicate_the_accepted_submission() {
    let (port, mut supervisor, _) = fixture(IntakeLimits::default());
    let mut channel = new_channel(port);
    channel.feed(&submit(42));
    assert_eq!(channel.write_once(&mut BrokenWriter).unwrap_err().kind(), io::ErrorKind::BrokenPipe);
    assert_eq!(channel.state(), ChannelState::Closed(CloseReason::IoFailure));
    let mut next = ActorChannel::new(channel.into_wire(), ChannelLimits::default()).unwrap();
    next.feed(&submit(42)); drain(&mut next);
    assert_eq!(supervisor.accept_next(&snapshot()).unwrap().unwrap().request, 42);
    assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());
}

#[test]
fn buffered_reads_consume_one_frame_and_eof_keeps_completed_work() {
    let (port, mut supervisor, _) = fixture(IntakeLimits::default());
    let mut channel = new_channel(port);
    let first = submit(42); let second = frame(Command::Cancel { request: 42 });
    let mut reader = Cursor::new([first.as_slice(), second.as_slice()].concat());
    assert_eq!(channel.read_once(&mut reader).unwrap(), ChannelState::ReplyReady);
    assert_eq!(reader.position(), first.len() as u64);
    channel.read_once(&mut reader).unwrap();
    assert_eq!(reader.position(), first.len() as u64);
    drain(&mut channel);
    channel.read_once(&mut reader).unwrap(); drain(&mut channel);
    assert_eq!(channel.read_once(&mut reader).unwrap(), ChannelState::Closed(CloseReason::PeerClosed));
    assert!(supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap().is_none());
}

#[cfg(unix)]
#[test]
fn unix_socket_requests_reach_congress_publication_and_redacted_receipt_polling() {
    use std::io::{BufRead, BufReader};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;
    let (port, mut supervisor, mut endpoint) = fixture(IntakeLimits::default());
    let mut channel = new_channel(port);
    let (mut client, mut server) = UnixStream::pair().unwrap();
    client.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    server.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let mut incoming = BufReader::new(server.try_clone().unwrap());
    let mut replies = BufReader::new(client.try_clone().unwrap());
    client.write_all(&submit(42)).unwrap();
    while channel.state() == ChannelState::Reading { channel.read_once(&mut incoming).unwrap(); }
    while channel.state() == ChannelState::ReplyReady { channel.write_once(&mut server).unwrap(); }
    let mut line = String::new(); replies.read_line(&mut line).unwrap();
    assert_eq!(json(line.as_bytes()).get("knowledge").unwrap().get("state").unwrap().as_str(), Some("pending"));
    assert!(supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap().is_some());
    let inputs = review(&mut supervisor, 42, 1);
    let permit = supervisor.authorize_request(42, Some(&inputs), &snapshot()).unwrap();
    supervisor.deliver_request(42, DispatchKeys::single(&permit), Some(&inputs), &snapshot(), &mut endpoint).unwrap();
    client.write_all(&frame(Command::Poll { request: 42 })).unwrap();
    while channel.state() == ChannelState::Reading { channel.read_once(&mut incoming).unwrap(); }
    while channel.state() == ChannelState::ReplyReady { channel.write_once(&mut server).unwrap(); }
    line.clear(); replies.read_line(&mut line).unwrap();
    assert_eq!(json(line.as_bytes()).get("knowledge").unwrap().get("value").unwrap().as_str(), Some("executed"));
    assert!(!line.contains("secret-")); assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(endpoint.payload(), b"publish");
    let mut wire = channel.into_wire();
    assert!(matches!(wire.exchange(&encode_command(&Command::Poll { request: 42 }).unwrap()).result,
        Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
}

#[test]
fn disconnect_after_publication_recovers_terminal_state_without_a_second_effect() {
    let (port, mut supervisor, mut endpoint) = fixture(IntakeLimits::default());
    let mut channel = new_channel(port);
    channel.feed(&submit(42)); drain(&mut channel);
    assert!(supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap().is_some());
    let inputs = review(&mut supervisor, 42, 1);
    let permit = supervisor.authorize_request(42, Some(&inputs), &snapshot()).unwrap();
    supervisor.deliver_request(42, DispatchKeys::single(&permit), Some(&inputs), &snapshot(), &mut endpoint).unwrap();
    channel.feed(&frame(Command::Poll { request: 42 }));
    assert!(channel.write_once(&mut BrokenWriter).is_err());
    let mut wire = channel.into_wire();
    assert!(matches!(wire.exchange(&encode_command(&Command::Submit { request: 42, proposal: proposal() }).unwrap()).result,
        Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());
    assert_eq!(endpoint.execution_count(), 1);
}
