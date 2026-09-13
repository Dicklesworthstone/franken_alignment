//! Same actor codec/channel, durable original authority, private supervisor role.
#![cfg(unix)]
#[path = "support/file_delivery.rs"]
mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::persistent::*;
use fa_reference::action::consequence::delivery::persistent::requests::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::*;
use fa_reference::action::consequence::oversight::actor::{ActorError, ActorOutcome, ActorProposal, Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::actor_wire::*;
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

fn proposal() -> ActorProposal {
    ActorProposal { target: profile().target, payload: b"payload".to_vec(), units: 16,
        deadline: ElapsedTick(100), expected_policy_epoch: 0 }
}
fn submit(key: u64) -> Vec<u8> {
    encode_command(&Command::Submit { request: key, proposal: proposal() }).unwrap()
}
fn provide(supervisor: &mut FileActorSupervisor) {
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
}
fn admitted(supervisor: &FileActorSupervisor, key: u64) -> u64 {
    match supervisor.host().unwrap().request_status(key).unwrap().disposition {
        FileRequestDisposition::Admitted { attempt, .. } => attempt,
        other => panic!("refused: {other:?}"),
    }
}
fn send(supervisor: &mut FileActorSupervisor, key: u64, publish: bool) {
    let id = admitted(supervisor, key);
    let mut guard = supervisor.host_mut().unwrap(); let host = &mut *guard;
    host.review(host.revision(), review(id, id + 100, Verdict::Allow)).unwrap();
    let permit = host.authorize(host.revision(), id, snapshot()).unwrap();
    let action = host.request_action(key).unwrap().clone();
    host.dispatch(host.revision(), &permit, &action, snapshot()).unwrap();
    if publish { host.publish(host.revision(), id).unwrap(); }
}

#[test]
fn actor_requests_need_one_fresh_supervisor_snapshot_but_retries_and_poll_do_not() {
    let root = Directory::new(); let (port, mut supervisor) = create(&root).into_actor_gateway();
    assert_eq!(port.submit(1, &proposal()).unwrap_err(), ActorError::Unavailable);
    provide(&mut supervisor);
    let ticket = port.submit(1, &proposal()).unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Pending { .. }));
    assert_eq!(port.submit(2, &proposal()).unwrap_err(), ActorError::Unavailable);
    let revision = supervisor.host().unwrap().revision();
    assert_eq!(port.submit(1, &proposal()).unwrap().request(), 1);
    assert_eq!(supervisor.host().unwrap().revision(), revision);
    provide(&mut supervisor);
    assert_eq!(supervisor.set_snapshot(revision + 1, Some(snapshot())), Err(JournalError::Contract(Error::Stale)));
    assert_eq!(port.submit(2, &proposal()).unwrap_err(), ActorError::Unavailable);
    provide(&mut supervisor);
    { let _host = supervisor.host_mut().unwrap(); }
    assert_eq!(port.submit(2, &proposal()).unwrap_err(), ActorError::Unavailable);
    provide(&mut supervisor); assert!(port.submit(2, &proposal()).is_ok());
    provide(&mut supervisor);
    port.cancel(&ticket).unwrap();
    assert_eq!(port.submit(3, &proposal()).unwrap_err(), ActorError::Unavailable);
    provide(&mut supervisor); assert!(port.submit(3, &proposal()).is_ok());
}

#[test]
fn existing_wire_restores_request_visibility_only_after_exact_retry_on_reopened_host() {
    let root = Directory::new(); let (port, mut supervisor) = create(&root).into_actor_gateway();
    provide(&mut supervisor); let mut wire = ActorWire::new(port.clone());
    assert!(matches!(wire.exchange(&submit(77)).result, Ok(Knowledge::Pending { .. })));
    send(&mut supervisor, 77, true);
    let original = port.submit(77, &proposal()).unwrap();
    assert!(matches!(port.poll(&original), Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }));
    drop(supervisor);
    assert!(matches!(port.poll(&original), Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }));
    let (fresh, mut supervisor) = FileDelivery::open(root.store(), profile()).unwrap().into_actor_gateway();
    assert!(matches!(fresh.poll(&original), Knowledge::Withheld { .. }));
    let mut wire = ActorWire::new(fresh);
    let poll = encode_command(&Command::Poll { request: 77 }).unwrap();
    assert!(matches!(wire.exchange(&poll).result, Ok(Knowledge::Withheld { .. })));
    let revision = supervisor.host().unwrap().revision();
    assert!(matches!(wire.exchange(&submit(77)).result, Ok(Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown })));
    assert_eq!(supervisor.host().unwrap().revision(), revision);
    let id = admitted(&supervisor, 77);
    {
        let mut guard = supervisor.host_mut().unwrap(); let host = &mut *guard;
        host.observe_time(revision, ElapsedTick(2)).unwrap();
        host.reconcile(host.revision(), id).unwrap();
        assert_eq!(host.inspect().executions, 1);
        assert_eq!(host.inspect().control.ledger.charged, 16);
    }
    assert!(matches!(wire.exchange(&poll).result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    let bytes = wire.exchange(&poll).encode(); let text = std::str::from_utf8(&bytes).unwrap();
    for hidden in ["payload", "alpha", "beta", "salt", "charged", "attempt", "snapshot"] { assert!(!text.contains(hidden)); }
}

#[test]
fn malformed_and_authority_injecting_commands_never_consume_snapshot_or_write_a_request() {
    let root = Directory::new(); let (port, mut supervisor) = create(&root).into_actor_gateway();
    provide(&mut supervisor); let mut wire = ActorWire::new(port);
    let good = String::from_utf8(submit(1)).unwrap(); let revision = supervisor.host().unwrap().revision();
    for extra in ["\"scope\":{}", "\"permit\":1", "\"snapshot\":{}", "\"ballots\":[]"] {
        let bad = format!("{{{extra},{}", &good[1..]);
        assert_eq!(wire.exchange(bad.as_bytes()).result, Err(WireError::MalformedRequest));
        assert_eq!(supervisor.host().unwrap().revision(), revision);
    }
    assert!(matches!(wire.exchange(good.as_bytes()).result, Ok(Knowledge::Pending { .. })));
}

#[test]
fn channel_backpressure_and_failed_output_do_not_replay_durable_admission() {
    let root = Directory::new(); let (port, mut supervisor) = create(&root).into_actor_gateway();
    provide(&mut supervisor);
    let mut channel = ActorChannel::new(ActorWire::new(port.clone()), ChannelLimits::default()).unwrap();
    let mut frame = submit(1); frame.push(b'\n');
    assert_eq!(channel.feed(&frame).state, ChannelState::ReplyReady);
    let revision = supervisor.host().unwrap().revision();
    assert_eq!(channel.feed(&frame).consumed, 0);
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    assert!(channel.write_once(&mut Broken).is_err());
    let mut reconnect = ActorChannel::new(channel.into_wire(), ChannelLimits::default()).unwrap();
    reconnect.feed(&frame);
    assert_eq!(supervisor.host().unwrap().revision(), revision);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
    let mut partial = ActorChannel::new(ActorWire::new(port), ChannelLimits::default()).unwrap();
    let other = submit(2); partial.feed(&other);
    assert_eq!(partial.finish_input(), ChannelState::Closed(CloseReason::TruncatedFrame));
    assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
}

#[test]
fn actor_cancellation_cannot_refund_a_published_or_unknown_effect() {
    for published in [false, true] {
        let root = Directory::new(); let (port, mut supervisor) = create(&root).into_actor_gateway();
        provide(&mut supervisor); let ticket = port.submit(1, &proposal()).unwrap();
        send(&mut supervisor, 1, published);
        let revision = supervisor.host().unwrap().revision();
        port.cancel(&ticket).unwrap();
        assert_eq!(supervisor.host().unwrap().revision(), revision);
        assert_eq!(supervisor.host().unwrap().inspect().control.ledger.charged, 16);
        assert!(matches!(port.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }));
        let id = admitted(&supervisor, 1);
        {
            let mut guard = supervisor.host_mut().unwrap(); let host = &mut *guard;
            host.seal_unexecuted(revision, id).unwrap();
        }
        let expected = if published { ActorOutcome::Executed } else { ActorOutcome::ConfirmedNotExecuted };
        assert!(matches!(port.poll(&ticket), Knowledge::Known { value, .. } if value == expected));
    }
}

#[test]
fn real_unix_stream_uses_the_original_framing_through_durable_publication() {
    let root = Directory::new(); let (port, mut supervisor) = create(&root).into_actor_gateway();
    provide(&mut supervisor);
    let (mut server, mut client) = UnixStream::pair().unwrap();
    for stream in [&server, &client] {
        stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
    }
    let mut reader = BufReader::new(server.try_clone().unwrap());
    let mut remote = BufReader::new(client.try_clone().unwrap());
    let mut channel = ActorChannel::new(ActorWire::new(port), ChannelLimits::default()).unwrap();
    let mut frame = submit(42); frame.push(b'\n'); client.write_all(&frame).unwrap();
    assert_eq!(channel.read_once(&mut reader).unwrap(), ChannelState::ReplyReady);
    while channel.state() == ChannelState::ReplyReady { channel.write_once(&mut server).unwrap(); }
    let mut response = String::new(); remote.read_line(&mut response).unwrap();
    assert!(response.contains("pending"));
    send(&mut supervisor, 42, true); let id = admitted(&supervisor, 42);
    { let mut guard = supervisor.host_mut().unwrap(); let host = &mut *guard; host.reconcile(host.revision(), id).unwrap(); }
    let mut poll = encode_command(&Command::Poll { request: 42 }).unwrap(); poll.push(b'\n');
    client.write_all(&poll).unwrap(); channel.read_once(&mut reader).unwrap();
    while channel.state() == ChannelState::ReplyReady { channel.write_once(&mut server).unwrap(); }
    response.clear(); remote.read_line(&mut response).unwrap();
    assert!(response.contains("executed")); assert!(!response.contains("payload"));
    assert_eq!(supervisor.host().unwrap().inspect().executions, 1);
}

#[test]
fn holding_the_privileged_borrow_does_not_leak_authority_or_panic_the_actor_port() {
    let root = Directory::new(); let (port, mut supervisor) = create(&root).into_actor_gateway();
    provide(&mut supervisor); let ticket = port.submit(1, &proposal()).unwrap();
    {
        let _guard = supervisor.host_mut().unwrap();
        assert_eq!(port.submit(2, &proposal()).unwrap_err(), ActorError::Unavailable);
        assert!(matches!(port.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }));
        assert_eq!(port.cancel(&ticket), Err(ActorError::Unavailable));
    }
    assert!(matches!(port.poll(&ticket), Knowledge::Pending { .. }));
    drop(supervisor);
    // Ports/tickets/wires retain only Weak owners, not the process file lock.
    let host = FileDelivery::open(root.store(), profile()).unwrap();
    assert!(matches!(host.request_status(1).unwrap().disposition,
        FileRequestDisposition::Admitted { stage: ActionState::Cancelled, .. }));
    assert_eq!(port.submit(1, &proposal()).unwrap_err(), ActorError::Unavailable);
}
