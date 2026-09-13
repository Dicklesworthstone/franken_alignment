//! Full-input and human approval remain outside the durable actor wire.
#![cfg(unix)]
#[path = "support/file_oversight_actor.rs"]
mod support;
use support::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::oversight::actor::{ActorError, ActorOutcome, Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, Command, WireError, encode_command};
use fa_reference::Error;

#[test]
fn actor_wire_sees_pending_until_original_full_review_and_both_keys_execute() {
    let root = Directory::new(); let (port, mut supervisor, reviewer, proposal) = create(&root);
    observe(&mut supervisor);
    let mut wire = ActorWire::new(port);
    let bytes = submit_bytes(70, &proposal);
    assert!(matches!(wire.exchange(&bytes).result, Ok(Knowledge::Pending { request: 70 })));
    let keys = ready(&mut supervisor, &reviewer, 70);
    let poll = encode_command(&Command::Poll { request: 70 }).unwrap();
    assert!(matches!(wire.exchange(&poll).result, Ok(Knowledge::Pending { .. })));
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &keys);
        let revision = host.revision(); host.publish(revision, keys.automatic.attempt()).unwrap();
    }
    assert!(matches!(wire.exchange(&poll).result, Ok(Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown })));
    drop(reviewer);
    {
        let mut host = supervisor.host_mut().unwrap(); let revision = host.revision();
        assert!(matches!(host.reconcile(revision, keys.automatic.attempt()).unwrap(), Reconciliation::Resolved(_)));
    }
    let response = wire.exchange(&poll);
    assert!(matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    let output = String::from_utf8(response.encode()).unwrap();
    for private in ["PRIVATE-SOURCE-BYTES", "original-alpha", "reviewer", "charged", "attempt", "permit"] {
        assert!(!output.contains(private));
    }
    let revision = supervisor.host().unwrap().revision();
    assert!(matches!(wire.exchange(&bytes).result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert_eq!(supervisor.host().unwrap().revision(), revision);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 1);
}

#[test]
fn supervisor_loss_releases_lock_and_exact_retry_recovers_only_the_observation() {
    let root = Directory::new(); let (port, mut supervisor, reviewer, proposal) = create(&root);
    observe(&mut supervisor); let ticket = port.submit(1, &proposal).unwrap();
    let retained_port = port.clone(); let retained_ticket = ticket.clone();
    let keys = ready(&mut supervisor, &reviewer, 1);
    drop(supervisor);
    assert!(matches!(port.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }));
    let (host, fresh_reviewer) = FileOversight::open(root.store(), profile()).unwrap();
    let (fresh_port, mut supervisor) = host.into_actor_gateway();
    assert!(matches!(fresh_port.poll(&ticket), Knowledge::Withheld { .. }));
    assert!(matches!(retained_port.poll(&retained_ticket), Knowledge::Unknown { .. }));
    let mut wire = ActorWire::new(fresh_port);
    assert!(matches!(wire.exchange(&encode_command(&Command::Poll { request: 1 }).unwrap()).result, Ok(Knowledge::Withheld { .. })));
    let revision = supervisor.host().unwrap().revision();
    assert!(matches!(wire.exchange(&submit_bytes(1, &proposal)).result,
        Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    assert_eq!(supervisor.host().unwrap().revision(), revision);
    {
        let mut host = supervisor.host_mut().unwrap();
        assert!(reviewer.approve(&mut host, revision, &keys.request).is_err());
        assert!(host.dispatch(revision, &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).is_err());
        assert_eq!(host.inspect().control.ledger.available, 100);
    }
    drop(fresh_reviewer);
}

#[test]
fn snapshots_are_one_use_and_borrow_or_invalid_replacement_does_not_leak_authority() {
    let root = Directory::new(); let (port, mut supervisor, _, proposal) = create(&root);
    assert_eq!(port.submit(1, &proposal).unwrap_err(), ActorError::Unavailable);
    observe(&mut supervisor); let first = port.submit(1, &proposal).unwrap();
    assert_eq!(port.submit(2, &proposal).unwrap_err(), ActorError::Unavailable);
    let revision = supervisor.host().unwrap().revision();
    observe(&mut supervisor);
    assert_eq!(supervisor.set_snapshot(revision + 1, Some(snapshot())), Err(JournalError::Contract(Error::Stale)));
    assert_eq!(port.submit(2, &proposal).unwrap_err(), ActorError::Unavailable);
    observe(&mut supervisor);
    {
        let _owner = supervisor.host_mut().unwrap();
        assert_eq!(port.submit(2, &proposal).unwrap_err(), ActorError::Unavailable);
        assert!(matches!(port.poll(&first), Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }));
    }
    assert_eq!(port.submit(2, &proposal).unwrap_err(), ActorError::Unavailable);
    let exact = port.submit(1, &proposal).unwrap();
    assert_eq!(exact.request(), 1); assert_eq!(supervisor.host().unwrap().revision(), revision);
    observe(&mut supervisor); assert!(port.submit(2, &proposal).is_ok());
    let mut wire = ActorWire::new(port);
    let injected = b"{\"version\":1,\"operation\":\"approve\",\"request\":\"1\"}";
    assert_eq!(wire.exchange(injected).result, Err(WireError::MalformedRequest));
}

#[test]
fn ambiguous_storage_cannot_return_a_ticket_and_recovery_keeps_no_speculative_admission() {
    let root = Directory::new(); let (port, mut supervisor, _, proposal) = create(&root);
    observe(&mut supervisor);
    std::fs::write(root.store().join("delivery.pending"), b"occupied stage").unwrap();
    assert_eq!(port.submit(1, &proposal).unwrap_err(), ActorError::Unavailable);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 0);
    assert!(supervisor.host().unwrap().storage_failure().is_some());
    drop(supervisor);
    let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(host.retained_requests(), 0);
    let revision = host.revision(); host.observe_time(revision, ElapsedTick(2)).unwrap();
    let current = support::proposal(&oversight::spec(&host, b"published"));
    let (port, mut supervisor) = host.into_actor_gateway(); observe(&mut supervisor);
    let ticket = port.submit(2, &current).unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Pending { .. }));
    assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
}

#[test]
fn existing_unix_transport_handles_partial_io_then_two_key_publication_and_exact_reconnect() {
    use fa_reference::action::consequence::oversight::actor_transport::{DriveBudget, UnixActorConnection};
    use fa_reference::action::consequence::oversight::actor_wire::{ActorChannel, ChannelLimits};
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::time::{Duration, Instant};
    let root = Directory::new(); let (port, mut supervisor, reviewer, proposal) = create(&root);
    observe(&mut supervisor);
    let (socket, mut peer) = UnixStream::pair().unwrap(); peer.set_nonblocking(true).unwrap();
    let mut connection = UnixActorConnection::new(socket,
        ActorChannel::new(ActorWire::new(port), ChannelLimits::default()).unwrap()).unwrap();
    let mut bytes = submit_bytes(17, &proposal); bytes.push(b'\n');
    peer.write_all(&bytes).unwrap();
    let budget = DriveBudget { read_bytes: 7, write_bytes: 3, frames: 1, io_calls: 4 };
    let until = Instant::now() + Duration::from_secs(10); let mut response = Vec::new();
    while !response.ends_with(b"\n") {
        let report = connection.drive(budget).unwrap();
        assert!(report.progress.read_bytes <= 7 && report.progress.written_bytes <= 3 && report.progress.io_calls <= 4);
        let mut part = [0; 32];
        match peer.read(&mut part) {
            Ok(n) => response.extend_from_slice(&part[..n]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {},
            Err(error) => panic!("peer read: {error}"),
        }
        assert!(Instant::now() < until, "transport did not complete bounded response");
    }
    assert!(String::from_utf8(response).unwrap().contains("pending"));
    let keys = ready(&mut supervisor, &reviewer, 17);
    {
        let mut host = supervisor.host_mut().unwrap(); oversight::dispatch(&mut host, &keys);
        let revision = host.revision(); host.publish(revision, keys.automatic.attempt()).unwrap();
        let revision = host.revision(); host.reconcile(revision, keys.automatic.attempt()).unwrap();
    }
    drop(peer);
    let mut wire = connection.into_session(); let revision = supervisor.host().unwrap().revision();
    let response = wire.exchange(&submit_bytes(17, &proposal));
    assert!(matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert_eq!(supervisor.host().unwrap().revision(), revision);
    assert_eq!(supervisor.host().unwrap().inspect().control.ledger.stages[&keys.automatic.attempt()], ActionState::Confirmed);
}
