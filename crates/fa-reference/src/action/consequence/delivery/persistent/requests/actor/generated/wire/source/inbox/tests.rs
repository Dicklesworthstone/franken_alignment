//! Real generated source requests, journal status, socket custody and work hints.
use super::super::*;
use crate::Error;
use crate::action::ActionState;
use crate::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use crate::action::consequence::delivery::persistent::requests::actor::FileActorInbox;
use crate::action::consequence::delivery::persistent::requests::actor::source_wire::FileActorPeerDriveError;
use crate::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use crate::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy};
use crate::action::consequence::oversight::actor_transport::{DriveBudget, MAX_DRIVE_FRAMES};
use crate::action::consequence::oversight::actor_wire::{
    ChannelLimits, Command, WireError, WireResponse, decode_response, encode_command,
};
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;

#[path = "../tests/fixture.rs"]
mod fixture;
use fixture::*;

type Inbox = FileActorInbox<FileGeneratedTextActorPort>;
fn no_clock() -> ElapsedTick { panic!("this operation must not acquire source evidence") }
fn input() -> DriveBudget { DriveBudget { write_bytes: 0, frames: 1, ..DriveBudget::default() } }
fn connected(port: FileGeneratedTextActorPort) -> (Inbox, UnixStream) {
    let (server, client) = UnixStream::pair().unwrap();
    let credentials = PeerCredentials::observe(&server).unwrap();
    let policy = PeerPolicy::new(credentials.uid(), credentials.gid(), Some(credentials.pid())).unwrap();
    let mut inbox = FileActorInbox::new(policy, ActorWire::new(port), ChannelLimits::default(), 2).unwrap();
    inbox.attach(server).unwrap();
    client.set_nonblocking(true).unwrap();
    (inbox, client)
}
fn send(client: &mut UnixStream, document: &[u8]) {
    let mut line = document.to_vec();
    line.push(b'\n');
    client.write_all(&line).unwrap();
}
fn drain<S: EvidenceFile + ?Sized>(inbox: &mut Inbox, client: &mut UnixStream,
    driver: &mut FileSupervisedDriver, source: &mut S) -> WireResponse
{
    let mut bytes = Vec::new();
    for _ in 0..64 {
        let report = inbox.drive(driver, source, no_clock,
            DriveBudget { read_bytes: 0, frames: 0, ..DriveBudget::default() }).unwrap();
        assert_eq!(report.drive.progress.frames, 0);
        assert!(report.intakes.is_empty());
        let mut buffer = [0; 513];
        match client.read(&mut buffer) {
            Ok(0) => panic!("EOF before response"),
            Ok(count) => bytes.extend_from_slice(&buffer[..count]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("response read: {error}"),
        }
        if bytes.last() == Some(&b'\n') {
            return decode_response(&bytes[..bytes.len() - 1]).unwrap();
        }
    }
    panic!("response did not drain in bounded turns");
}

#[test]
fn generated_inbox_retains_unsent_admission_and_reconnect_ticket_after_ingress_revocation() {
    let mut s = setup();
    let before = disk(&s);
    let numerical = s.supervisor.host().unwrap().decoder_inspection().unwrap().numerical;
    let reads = s.source.status().read_attempts;
    let (mut inbox, mut client) = connected(s.port.clone());
    let mut driver = FileSupervisedDriver::new(s.supervisor);
    send(&mut client, &document(&s.input));
    let report = inbox.drive(&mut driver, &mut s.source, || ElapsedTick(2), input()).unwrap();
    assert_eq!(report.drive.progress.frames, 1);
    assert_eq!(report.drive.progress.written_bytes, 0);
    assert_eq!(report.intakes.len(), 1);
    assert_eq!(inbox.queued(), 1); // acknowledgment deliberately never sent
    assert_ne!(std::fs::read(s.root.store().join("delivery.bin")).unwrap(), before);
    assert_eq!(driver.supervisor().host().unwrap().decoder_inspection().unwrap().numerical, numerical);
    assert!(inbox.disconnect());
    drop(client);
    std::fs::remove_file(s.root.source()).unwrap();
    let (server, mut client) = UnixStream::pair().unwrap();
    inbox.attach(server).unwrap();
    client.set_nonblocking(true).unwrap();
    assert_eq!(inbox.status().connections_admitted, 2);
    send(&mut client, &encode_command(&Command::Poll { request: 91 }).unwrap());
    assert!(inbox.drive(&mut driver, &mut s.source, no_clock, input()).unwrap().intakes.is_empty());
    assert_eq!(drain(&mut inbox, &mut client, &mut driver, &mut s.source).result,
        Ok(Knowledge::Pending { request: 91 }));
    let reserved = driver.supervisor().host().unwrap().inspect().control.ledger.reserved;
    assert!(inbox.revoke());
    assert_eq!(inbox.queued(), 1);
    assert!(matches!(inbox.drive(&mut driver, &mut s.source, no_clock, input()),
        Err(FileActorPeerDriveError::Wire(WireError::Withheld))));
    let next = inbox.next_request(&driver).unwrap().unwrap();
    assert_eq!(next.request, 91);
    assert!(matches!(next.disposition, FileRequestDisposition::Admitted { stage: ActionState::Reviewing, .. }));
    assert_eq!(inbox.queued(), 0);
    assert_eq!(driver.supervisor().host().unwrap().inspect().control.ledger.reserved, reserved);
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
    assert_eq!(s.source.status().read_attempts, reads + 1);
}

#[test]
fn generated_inbox_deduplicates_retries_and_skips_cancellation_in_observed_fifo_order() {
    let mut s = setup();
    let reads = s.source.status().read_attempts;
    let (mut inbox, mut client) = connected(s.port.clone());
    let mut driver = FileSupervisedDriver::new(s.supervisor);
    for id in [91, 7, 42] {
        let mut request = s.input.clone();
        request.request = id;
        send(&mut client, &document(&request));
        assert_eq!(inbox.drive(&mut driver, &mut s.source, || ElapsedTick(2), input()).unwrap().intakes.len(), 1);
        assert_eq!(drain(&mut inbox, &mut client, &mut driver, &mut s.source).result,
            Ok(Knowledge::Pending { request: id }));
    }
    assert_eq!(inbox.queued(), 3);
    send(&mut client, &document(&s.input));
    assert!(inbox.drive(&mut driver, &mut s.source, no_clock, input()).unwrap().intakes.is_empty());
    assert_eq!(drain(&mut inbox, &mut client, &mut driver, &mut s.source).result,
        Ok(Knowledge::Pending { request: 91 }));
    let mut conflicting = s.input.clone();
    conflicting.generation = 999;
    send(&mut client, &document(&conflicting));
    assert!(inbox.drive(&mut driver, &mut s.source, no_clock, input()).unwrap().intakes.is_empty());
    assert_eq!(drain(&mut inbox, &mut client, &mut driver, &mut s.source).result, Err(WireError::IdempotencyConflict));
    assert_eq!(inbox.queued(), 3);
    send(&mut client, &encode_command(&Command::Cancel { request: 7 }).unwrap());
    assert!(inbox.drive(&mut driver, &mut s.source, no_clock, input()).unwrap().intakes.is_empty());
    assert!(matches!(drain(&mut inbox, &mut client, &mut driver, &mut s.source).result,
        Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    assert_eq!(inbox.next_request(&driver).unwrap().unwrap().request, 91);
    assert_eq!(inbox.next_request(&driver).unwrap().unwrap().request, 42);
    assert!(inbox.next_request(&driver).unwrap().is_none());
    assert_eq!(s.source.status().read_attempts, reads + 3);
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn generated_inbox_malformed_fragments_and_failed_source_never_queue_new_work() {
    let mut s = setup();
    let reads = s.source.status().read_attempts;
    let (mut inbox, mut client) = connected(s.port.clone());
    let mut driver = FileSupervisedDriver::new(s.supervisor);
    let mut proposal = FileGeneratedTextActorPort::encode_message(&s.input).unwrap();
    proposal.payload[0] ^= 1;
    send(&mut client, &encode_command(&Command::Submit { request: 91, proposal }).unwrap());
    assert!(inbox.drive(&mut driver, &mut s.source, no_clock, input()).unwrap().intakes.is_empty());
    assert_eq!(drain(&mut inbox, &mut client, &mut driver, &mut s.source).result, Err(WireError::MalformedRequest));
    assert_eq!(inbox.queued(), 0);
    client.write_all(&document(&s.input)).unwrap();
    let report = inbox.drive(&mut driver, &mut s.source, no_clock, input()).unwrap();
    assert_eq!(report.drive.progress.frames, 0);
    assert!(report.intakes.is_empty());
    assert_eq!(s.source.status().read_attempts, reads);
    std::fs::remove_file(s.root.source()).unwrap();
    client.write_all(b"\n").unwrap();
    let report = inbox.drive(&mut driver, &mut s.source, || ElapsedTick(2), input()).unwrap();
    assert!(report.intakes[0].result.is_err());
    assert_eq!(drain(&mut inbox, &mut client, &mut driver, &mut s.source).result, Err(WireError::Unavailable));
    assert_eq!(inbox.queued(), 0);
    assert!(inbox.next_request(&driver).unwrap().is_none());
    assert!(matches!(driver.supervisor().host().unwrap().request_status(91),
        Err(JournalError::Contract(Error::Missing))));
    s.root.replace(&capture(2, true, b"allow"));
    send(&mut client, &document(&s.input));
    let report = inbox.drive(&mut driver, &mut s.source, || ElapsedTick(3), input()).unwrap();
    assert_eq!(report.intakes[0].result, Ok(capture(2, true, b"allow").identity()));
    assert_eq!(drain(&mut inbox, &mut client, &mut driver, &mut s.source).result,
        Ok(Knowledge::Pending { request: 91 }));
    assert_eq!(inbox.next_request(&driver).unwrap().unwrap().request, 91);
}

#[test]
fn generated_inbox_foreign_owner_and_budgets_preserve_unread_bytes_and_queued_hints() {
    let mut a = setup();
    let mut b = setup();
    let before_a = disk(&a);
    let before_b = disk(&b);
    let (mut inbox, mut client) = connected(b.port.clone());
    let status = inbox.status();
    let mut driver_a = FileSupervisedDriver::new(a.supervisor);
    let mut driver_b = FileSupervisedDriver::new(b.supervisor);
    send(&mut client, &document(&b.input));
    assert!(matches!(inbox.drive(&mut driver_a, &mut a.source, no_clock, input()),
        Err(FileActorPeerDriveError::Journal(JournalError::Contract(Error::Binding)))));
    assert!(matches!(inbox.drive(&mut driver_b, &mut b.source, no_clock,
        DriveBudget { frames: MAX_DRIVE_FRAMES + 1, ..input() }),
        Err(FileActorPeerDriveError::Wire(WireError::Capacity))));
    let report = inbox.drive(&mut driver_b, &mut b.source, no_clock,
        DriveBudget { read_bytes: 0, write_bytes: 0, frames: 0, io_calls: 0 }).unwrap();
    assert_eq!(report.drive.progress.frames, 0);
    assert!(report.intakes.is_empty());
    assert_eq!(inbox.status(), status);
    assert_eq!(std::fs::read(a.root.store().join("delivery.bin")).unwrap(), before_a);
    assert_eq!(std::fs::read(b.root.store().join("delivery.bin")).unwrap(), before_b);
    inbox.drive(&mut driver_b, &mut b.source, || ElapsedTick(2), input()).unwrap();
    assert_eq!(inbox.queued(), 1);
    assert!(matches!(inbox.next_request(&driver_a), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(inbox.queued(), 1);
    assert_eq!(inbox.next_request(&driver_b).unwrap().unwrap().request, 91);
}

#[test]
fn generated_inbox_storage_fault_keeps_prior_hint_and_exposes_no_new_ticket() {
    let mut s = setup();
    let (mut inbox, mut client) = connected(s.port.clone());
    let mut driver = FileSupervisedDriver::new(s.supervisor);
    send(&mut client, &document(&s.input));
    inbox.drive(&mut driver, &mut s.source, || ElapsedTick(2), input()).unwrap();
    assert_eq!(drain(&mut inbox, &mut client, &mut driver, &mut s.source).result,
        Ok(Knowledge::Pending { request: 91 }));
    let before = std::fs::read(s.root.store().join("delivery.bin")).unwrap();
    std::fs::write(s.root.store().join("delivery.pending"), b"existing unacknowledged staging file").unwrap();
    let mut next = s.input.clone();
    next.request = 92;
    send(&mut client, &document(&next));
    assert!(matches!(inbox.drive(&mut driver, &mut s.source, || ElapsedTick(3), input()),
        Err(FileActorPeerDriveError::Journal(_))));
    assert_eq!(inbox.queued(), 1);
    assert!(driver.supervisor().host().unwrap().storage_failure().is_some());
    assert!(inbox.next_request(&driver).is_err());
    assert_eq!(inbox.queued(), 1);
    assert_eq!(std::fs::read(s.root.store().join("delivery.bin")).unwrap(), before);
    assert_eq!(drain(&mut inbox, &mut client, &mut driver, &mut s.source).result, Err(WireError::Unavailable));
    send(&mut client, &encode_command(&Command::Poll { request: 92 }).unwrap());
    inbox.drive(&mut driver, &mut s.source, no_clock, input()).unwrap();
    assert!(matches!(drain(&mut inbox, &mut client, &mut driver, &mut s.source).result,
        Ok(Knowledge::Withheld { .. })));
}

#[test]
fn generated_inbox_dispatch_is_reconciliation_only_and_terminal_retries_do_not_requeue() {
    use crate::action::consequence::delivery::EndpointOutcome;
    use crate::action::consequence::oversight::actor::ActorOutcome;
    use crate::action::consequence::oversight::ReviewWindow;
    use crate::round::{Verdict, commitment};
    let mut s = setup();
    let (mut inbox, mut client) = connected(s.port.clone());
    let mut driver = FileSupervisedDriver::new(s.supervisor);
    send(&mut client, &document(&s.input));
    inbox.drive(&mut driver, &mut s.source, || ElapsedTick(2), input()).unwrap();
    drain(&mut inbox, &mut client, &mut driver, &mut s.source);
    let mut borrowed = driver.supervisor_mut().host_mut().unwrap();
    let host = &mut *borrowed;
    let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(91).unwrap().disposition
        else { panic!("expected original admission"); };
    let action = host.request_action(91).unwrap().clone();
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
    let request = host.request_human_approval(host.revision(), 1001, attempt, &inputs, ElapsedTick(10)).unwrap();
    let revision = host.revision();
    let human = s.reviewer.approve(host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot.clone()).unwrap();
    drop(borrowed);
    let next = inbox.next_request(&driver).unwrap().unwrap();
    assert!(matches!(next.disposition, FileRequestDisposition::Admitted { stage: ActionState::Dispatching | ActionState::Unknown, .. }));
    let reads = s.source.status().read_attempts;
    send(&mut client, &encode_command(&Command::Cancel { request: 91 }).unwrap());
    assert!(inbox.drive(&mut driver, &mut s.source, no_clock, input()).unwrap().intakes.is_empty());
    assert!(matches!(drain(&mut inbox, &mut client, &mut driver, &mut s.source).result,
        Ok(Knowledge::Unknown { .. })));
    send(&mut client, &document(&s.input));
    assert!(inbox.drive(&mut driver, &mut s.source, no_clock, input()).unwrap().intakes.is_empty());
    assert!(matches!(drain(&mut inbox, &mut client, &mut driver, &mut s.source).result,
        Ok(Knowledge::Unknown { .. })));
    assert_eq!(inbox.queued(), 1); // original dispatched work, not another review
    let mut borrowed = driver.supervisor_mut().host_mut().unwrap();
    let host = &mut *borrowed;
    assert!(matches!(host.publish_checked(host.revision(), attempt, Some(&inputs), snapshot, ElapsedTick(2)).unwrap().outcome,
        EndpointOutcome::Executed { .. }));
    host.reconcile(host.revision(), attempt).unwrap();
    assert_eq!(host.stream_snapshot().unwrap().confirmed.visible(), b"A");
    assert_eq!(host.inspect().control.ledger.charged, action.spec().units);
    drop(borrowed);
    assert!(inbox.next_request(&driver).unwrap().is_none());
    send(&mut client, &document(&s.input));
    assert!(inbox.drive(&mut driver, &mut s.source, no_clock, input()).unwrap().intakes.is_empty());
    assert!(matches!(drain(&mut inbox, &mut client, &mut driver, &mut s.source).result,
        Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert_eq!(inbox.queued(), 0);
    assert_eq!(s.source.status().read_attempts, reads);
}

#[test]
fn generated_inbox_independent_clients_keep_their_own_queue_and_drive_allowance() {
    let mut s = setup();
    let (mut first, mut a) = connected(s.port.clone());
    let (mut second, mut b) = connected(s.port.clone());
    let mut driver = FileSupervisedDriver::new(s.supervisor);
    send(&mut a, &document(&s.input));
    let mut next = s.input.clone();
    next.request = 92;
    send(&mut a, &document(&next));
    next.request = 41;
    send(&mut b, &document(&next));
    let report = first.drive(&mut driver, &mut s.source, || ElapsedTick(2), input()).unwrap();
    assert_eq!(report.drive.progress.frames, 1);
    assert_eq!(report.intakes.len(), 1);
    assert_eq!(first.queued(), 1);
    // The host deliberately selects another session instead of draining the
    // first peer's burst. This demonstrates bounded composition, not a new loop.
    let report = second.drive(&mut driver, &mut s.source, || ElapsedTick(2), input()).unwrap();
    assert_eq!(report.drive.progress.frames, 1);
    assert_eq!(report.intakes.len(), 1);
    assert_eq!(second.next_request(&driver).unwrap().unwrap().request, 41);
    assert_eq!(drain(&mut first, &mut a, &mut driver, &mut s.source).result,
        Ok(Knowledge::Pending { request: 91 }));
    assert_eq!(first.queued(), 1); // flush-only drive cannot admit the buffered frame
    let report = first.drive(&mut driver, &mut s.source, || ElapsedTick(2), input()).unwrap();
    assert_eq!(report.drive.progress.frames, 1);
    assert_eq!(first.queued(), 2);
    assert_eq!(first.next_request(&driver).unwrap().unwrap().request, 91);
    assert_eq!(first.next_request(&driver).unwrap().unwrap().request, 92);
    assert_eq!(first.status().connections_admitted, 1);
    assert_eq!(second.status().connections_admitted, 1);
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
}
