use super::*;
use super::super::{FilePrecheckChannel, MAX_PRECHECK_FRAME_BYTES, PrecheckWireError,
    UnixPrecheckConnection, decode_precheck_response};
use crate::action::consequence::oversight::actor_transport::{DriveBudget, DriveInterest,
    DriveProgress, IoDirection, MAX_DRIVE_BYTES, MAX_DRIVE_FRAMES, MAX_DRIVE_IO_CALLS};
use crate::action::consequence::oversight::actor_wire::{ChannelState, CloseReason};
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;

fn connection(wire: FilePrecheckWire) -> (UnixPrecheckConnection, UnixStream) {
    let (server, client) = UnixStream::pair().unwrap();
    let channel = FilePrecheckChannel::new(wire, MAX_PRECHECK_FRAME_BYTES).unwrap();
    (UnixPrecheckConnection::new(server, channel).unwrap(), client)
}

fn received(client: &mut UnixStream) -> Vec<u8> {
    client.set_nonblocking(true).unwrap();
    let mut bytes = Vec::new();
    let mut buffer = [0; 2048];
    for _ in 0..16 {
        match client.read(&mut buffer) {
            Ok(0) => return bytes,
            Ok(count) => bytes.extend_from_slice(&buffer[..count]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return bytes,
            Err(error) => panic!("unexpected socket read: {error}"),
        }
    }
    panic!("bounded fixture reply failed to drain");
}

#[test]
fn invalid_drive_budgets_refuse_before_socket_or_buffered_work() {
    let (fixture, wire) = Fixture::new(2, 2);
    let (mut connection, mut client) = connection(wire);
    let bytes = frame(&request(1, b"ok"));
    client.write_all(&bytes).unwrap();
    let original = connection.status();
    for field in 0..4 {
        for too_large in [false, true] {
            let mut budget = DriveBudget::default();
            let (value, maximum) = match field {
                0 => (&mut budget.read_bytes, MAX_DRIVE_BYTES),
                1 => (&mut budget.write_bytes, MAX_DRIVE_BYTES),
                2 => (&mut budget.frames, MAX_DRIVE_FRAMES),
                _ => (&mut budget.io_calls, MAX_DRIVE_IO_CALLS),
            };
            *value = if too_large { maximum + 1 } else { 0 };
            assert_eq!(connection.drive(budget), Err(if too_large {
                PrecheckWireError::Capacity
            } else { PrecheckWireError::MalformedRequest }));
            assert_eq!(connection.status(), original);
            assert_eq!(fixture.remaining_queries(), 2);
        }
    }
    let report = connection.drive(DriveBudget::default()).unwrap();
    assert_eq!(report.progress.frames, 1);
    let bytes = received(&mut client);
    known(decode_precheck_response(&bytes[..bytes.len() - 1], 1, 0).unwrap(),
        FilePrecheckOutcome::PrecheckPassed, 1);
    assert_eq!(fixture.remaining_queries(), 1);
}

#[test]
fn buffered_pipeline_waits_for_output_and_flush_then_needs_no_readability_edge() {
    let (fixture, wire) = Fixture::new(3, 3);
    let (mut connection, mut client) = connection(wire);
    let bytes = [frame(&request(1, b"ok")), frame(&request(2, b"blocked"))].concat();
    client.write_all(&bytes).unwrap();
    let first = connection.drive(DriveBudget {
        read_bytes: bytes.len(), write_bytes: 1, frames: 1, io_calls: 2,
    }).unwrap();
    assert_eq!((first.progress.frames, first.progress.written_bytes, first.progress.io_calls), (1, 1, 2));
    assert_eq!(first.status.interest(), DriveInterest::Writable);
    assert!(first.status.buffered_input_bytes > 0);
    assert_eq!(fixture.remaining_queries(), 2);
    let written = connection.drive(DriveBudget { io_calls: 1, ..DriveBudget::default() }).unwrap();
    assert_eq!(written.progress.frames, 0);
    assert_eq!(written.status.channel, ChannelState::ReplyReady);
    assert_eq!(written.status.pending_output_bytes, 0);
    assert_eq!(written.status.interest(), DriveInterest::LocalWork);
    assert_eq!(fixture.remaining_queries(), 2);
    let flushed = connection.drive(DriveBudget { frames: 1, io_calls: 1, ..DriveBudget::default() }).unwrap();
    assert_eq!(flushed.progress.frames, 1);
    assert_eq!(flushed.progress.read_bytes, 0);
    assert_eq!(flushed.progress.io_calls, 1);
    assert_eq!(fixture.remaining_queries(), 1);
    assert_eq!(flushed.status.interest(), DriveInterest::Writable);
    connection.drive(DriveBudget::default()).unwrap();
    let bytes = received(&mut client);
    let lines: Vec<_> = bytes.split(|byte| *byte == b'\n').collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[2].is_empty());
    known(decode_precheck_response(lines[0], 1, 0).unwrap(), FilePrecheckOutcome::PrecheckPassed, 1);
    known(decode_precheck_response(lines[1], 2, 0).unwrap(), FilePrecheckOutcome::DeniedByPolicy, 2);
    assert_eq!(connection.status().interest(), DriveInterest::Readable);
}

#[test]
fn partial_input_eof_cannot_evaluate_and_would_block_preserves_fragments() {
    let (fixture, wire) = Fixture::new(2, 2);
    let (mut connection, mut client) = connection(wire);
    let bytes = frame(&request(4, b"ok"));
    client.write_all(&bytes[..bytes.len() - 1]).unwrap();
    let report = connection.drive(DriveBudget::default()).unwrap();
    assert_eq!(report.progress.frames, 0);
    assert_eq!(report.progress.would_block, Some(IoDirection::Read));
    assert_eq!(report.status.channel, ChannelState::Reading);
    assert_eq!(fixture.remaining_queries(), 2);
    client.write_all(b"\n").unwrap();
    let report = connection.drive(DriveBudget::default()).unwrap();
    assert_eq!(report.progress.frames, 1);
    let bytes = received(&mut client);
    known(decode_precheck_response(&bytes[..bytes.len() - 1], 4, 0).unwrap(),
        FilePrecheckOutcome::PrecheckPassed, 4);
    client.write_all(b"{").unwrap();
    client.shutdown(std::net::Shutdown::Write).unwrap();
    let report = connection.drive(DriveBudget::default()).unwrap();
    assert_eq!(report.progress.frames, 0);
    assert_eq!(report.status.channel, ChannelState::Closed(CloseReason::TruncatedFrame));
    assert_eq!(fixture.remaining_queries(), 1);
    assert_eq!(connection.into_session().remaining_exchanges(), 1);
}

#[test]
fn lost_reply_latches_failure_and_keeps_parser_and_native_work_spent() {
    let (fixture, wire) = Fixture::new(2, 2);
    let files = fixture.root.files();
    let before = fixture.supervisor.host().unwrap().inspect();
    let (mut connection, mut client) = connection(wire);
    let request_bytes = frame(&request(8, b"ok"));
    client.write_all(&request_bytes).unwrap();
    // Short reads may take several turns. Each turn can perform one read only,
    // so no response byte can escape before the complete frame has entered.
    let mut staged = None;
    let mut entered = 0;
    for _ in 0..=request_bytes.len() {
        let report = connection.drive(DriveBudget { io_calls: 1, ..DriveBudget::default() }).unwrap();
        assert_eq!(report.progress.written_bytes, 0);
        entered += report.progress.frames;
        assert!(entered <= 1);
        if entered == 1 { staged = Some(report); break; }
    }
    let staged = staged.expect("complete queued request entered within its byte bound");
    assert_eq!(staged.progress.frames, 1);
    assert_eq!(staged.progress.written_bytes, 0);
    assert!(staged.status.pending_output_bytes > 0);
    assert_eq!(fixture.remaining_queries(), 1);
    drop(client);
    let failed = connection.drive(DriveBudget::default()).unwrap();
    assert!(failed.status.closed());
    assert_eq!(failed.status.failure.unwrap().direction, IoDirection::Write);
    assert_eq!(fixture.remaining_queries(), 1);
    for _ in 0..3 {
        let again = connection.drive(DriveBudget::default()).unwrap();
        assert_eq!(again.progress, DriveProgress::default());
        assert_eq!(again.status, failed.status);
    }
    assert_eq!(fixture.root.files(), files);
    assert_eq!(fixture.supervisor.host().unwrap().inspect(), before);
    let wire = connection.into_session();
    assert_eq!(wire.remaining_exchanges(), 1);
    let (mut connection, mut client) = self::connection(wire);
    client.write_all(&frame(&request(8, b"ok"))).unwrap();
    let retried = connection.drive(DriveBudget::default()).unwrap();
    assert_eq!(retried.progress.frames, 1);
    assert_eq!(fixture.remaining_queries(), 0);
    assert_eq!(retried.status.channel, ChannelState::Closed(CloseReason::ExchangeLimit));
    let bytes = received(&mut client);
    known(decode_precheck_response(&bytes[..bytes.len() - 1], 8, 0).unwrap(),
        FilePrecheckOutcome::PrecheckPassed, 8);
}
