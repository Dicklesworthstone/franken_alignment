#![cfg(unix)]

#[path = "support/actor_gateway.rs"]
mod fixture;

use fa_reference::action::consequence::oversight::actor::IntakeLimits;
use fa_reference::action::consequence::oversight::actor_wire::{ActorChannel, ActorWire, ChannelLimits, ChannelState, CloseReason, Command, WireError, encode_command};
use fa_reference::action::consequence::oversight::actor_transport::{DriveBudget, IoDirection, UnixActorConnection, MAX_DRIVE_IO_CALLS};
use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;

fn line(command: Command) -> Vec<u8> {
    let mut bytes = encode_command(&command).unwrap(); bytes.push(b'\n'); bytes
}
fn submit(request: u64) -> Vec<u8> { line(Command::Submit { request, proposal: fixture::proposal() }) }
fn pending(request: u64) -> Vec<u8> {
    format!("{{\"version\":1,\"request\":\"{request}\",\"status\":\"ok\",\"knowledge\":{{\"state\":\"pending\",\"request\":\"{request}\"}}}}\n").into_bytes()
}
fn connection() -> (UnixStream, UnixActorConnection, fa_reference::action::consequence::oversight::actor::ActorSupervisor) {
    let (port, supervisor, _) = fixture::fixture(IntakeLimits::default());
    let (client, server) = UnixStream::pair().unwrap(); client.set_nonblocking(true).unwrap();
    let channel = ActorChannel::new(ActorWire::new(port), ChannelLimits::default()).unwrap();
    (client, UnixActorConnection::new(server, channel).unwrap(), supervisor)
}
fn read_available(client: &mut UnixStream) -> Vec<u8> {
    let mut bytes = Vec::new(); let mut buffer = [0; 1024];
    for _ in 0..128 {
        match client.read(&mut buffer) {
            Ok(0) => return bytes,
            Ok(n) => bytes.extend_from_slice(&buffer[..n]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return bytes,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => panic!("unexpected socket read: {error}"),
        }
    }
    panic!("test reader exceeded its finite bound")
}

#[test]
fn real_socket_fragments_only_enter_intake_after_newline() {
    let (mut client, mut connection, mut supervisor) = connection();
    let bytes = submit(7);
    client.write_all(&bytes[..bytes.len() - 1]).unwrap();
    let progress = connection.drive(DriveBudget::default()).unwrap().progress;
    assert_eq!(progress.frames, 0); assert_eq!(progress.read_bytes, bytes.len() - 1);
    assert!(supervisor.accept_next(&fixture::snapshot()).unwrap().is_none());
    assert!(read_available(&mut client).is_empty());
    client.write_all(b"\n").unwrap();
    assert_eq!(connection.drive(DriveBudget::default()).unwrap().progress.frames, 1);
    assert_eq!(read_available(&mut client), pending(7));
    assert_eq!(supervisor.accept_next(&fixture::snapshot()).unwrap().unwrap().request, 7);
}

#[test]
fn partial_writes_resume_exact_suffix_and_block_the_next_admission() {
    let (mut client, mut connection, mut supervisor) = connection();
    client.write_all(&[submit(1), submit(2)].concat()).unwrap();
    let report = connection.drive(DriveBudget { write_bytes: 0, ..DriveBudget::default() }).unwrap();
    assert_eq!(report.progress.frames, 1); assert_eq!(report.progress.written_bytes, 0);
    assert!(report.status.buffered_input_bytes > 0);
    supervisor.accept_next(&fixture::snapshot()).unwrap().unwrap();
    for _ in 0..3 {
        assert_eq!(connection.drive(DriveBudget { write_bytes: 0, ..DriveBudget::default() }).unwrap().progress.frames, 0);
        assert!(supervisor.accept_next(&fixture::snapshot()).unwrap().is_none());
    }
    let mut received = Vec::new(); let first = pending(1); let expected = [first.as_slice(), pending(2).as_slice()].concat();
    let mut second_admitted = false;
    for _ in 0..expected.len() + 10 {
        let report = connection.drive(DriveBudget { write_bytes: 1, ..DriveBudget::default() }).unwrap();
        assert!(report.progress.written_bytes <= 1);
        received.extend(read_available(&mut client));
        if let Some(accepted) = supervisor.accept_next(&fixture::snapshot()).unwrap() {
            assert_eq!(accepted.request, 2); assert!(!second_admitted);
            assert!(received.starts_with(&first)); second_admitted = true;
        }
        if received.len() == expected.len() { break; }
    }
    assert!(second_admitted); assert_eq!(received, expected);
}

#[test]
fn peer_half_close_drains_the_complete_reply_but_never_admits_its_tail() {
    let (mut client, mut connection, mut supervisor) = connection();
    client.write_all(&[submit(1), submit(2)[..12].to_vec()].concat()).unwrap();
    client.shutdown(Shutdown::Write).unwrap();
    let report = connection.drive(DriveBudget::default()).unwrap();
    assert_eq!(report.progress.frames, 1);
    assert_eq!(report.status.channel, ChannelState::Closed(CloseReason::TruncatedFrame));
    assert_eq!(read_available(&mut client), pending(1));
    assert_eq!(supervisor.accept_next(&fixture::snapshot()).unwrap().unwrap().request, 1);
    assert!(supervisor.accept_next(&fixture::snapshot()).unwrap().is_none());
}

#[test]
fn idle_would_block_yields_and_invalid_or_zero_budgets_do_no_work() {
    let (_, mut connection, _) = connection();
    let before = connection.status();
    assert_eq!(connection.drive(DriveBudget { io_calls: MAX_DRIVE_IO_CALLS + 1, ..DriveBudget::default() }), Err(WireError::Capacity));
    assert_eq!(connection.status(), before);
    let zero = connection.drive(DriveBudget { read_bytes: 0, write_bytes: 0, frames: 0, io_calls: 0 }).unwrap();
    assert_eq!(zero.progress.io_calls, 0); assert_eq!(zero.status, before);
    // Keep the actual peer alive for an empty, non-EOF read.
    let (client, mut connection, supervisor) = self::connection();
    let idle = connection.drive(DriveBudget::default()).unwrap();
    assert_eq!(idle.progress.would_block, Some(IoDirection::Read));
    assert_eq!(idle.progress.io_calls, 1); assert_eq!(idle.progress.frames, 0);
    drop((client, supervisor));
}

#[test]
fn frame_and_io_budgets_preserve_coalesced_input_between_drives() {
    let (mut client, mut connection, mut supervisor) = connection();
    client.write_all(&[submit(1), submit(2), submit(3)].concat()).unwrap();
    for request in 1..=3 {
        let report = connection.drive(DriveBudget { frames: 1, io_calls: 4, ..DriveBudget::default() }).unwrap();
        assert_eq!(report.progress.frames, 1); assert!(report.progress.io_calls <= 4);
        assert_eq!(supervisor.accept_next(&fixture::snapshot()).unwrap().unwrap().request, request);
        assert!(supervisor.accept_next(&fixture::snapshot()).unwrap().is_none());
        assert_eq!(read_available(&mut client), pending(request));
    }
}

#[test]
fn write_failure_returns_completed_work_and_does_not_cancel_the_reservation() {
    let (mut client, mut connection, mut supervisor) = connection();
    client.write_all(&submit(8)).unwrap();
    let accepted = connection.drive(DriveBudget { write_bytes: 0, ..DriveBudget::default() }).unwrap();
    assert_eq!(accepted.progress.frames, 1);
    supervisor.accept_next(&fixture::snapshot()).unwrap().unwrap();
    let inputs = fixture::review(&mut supervisor, 8, 1);
    let _permit = supervisor.authorize_request(8, Some(&inputs), &fixture::snapshot()).unwrap();
    let before = supervisor.broker().inspect(); drop(client);
    let failed = connection.drive(DriveBudget::default()).unwrap();
    assert_eq!(failed.status.failure.unwrap().direction, IoDirection::Write);
    assert_eq!(failed.progress.frames, 0); assert!(failed.progress.io_calls > 0);
    assert_eq!(connection.drive(DriveBudget::default()).unwrap().progress.io_calls, 0);
    assert_eq!(supervisor.broker().inspect(), before);
    assert_eq!(before.ledger.reserved, fixture::proposal().units);
}
