#![cfg(unix)]

#[path = "support/actor_gateway.rs"]
mod fixture;

use fa_reference::action::consequence::delivery::{EndpointOutcome, FilePublicationLimits, NonExecutionReason, PublicationEndpoint};
use fa_reference::action::consequence::oversight::{DispatchKeys, human::HumanReviewPolicy};
use fa_reference::action::consequence::oversight::actor::{ActorSupervisor, IntakeLimits};
use fa_reference::action::consequence::oversight::actor_wire::{ActorChannel, ActorWire, ChannelLimits, ChannelState, CloseReason, Command, encode_command};
use fa_reference::action::consequence::oversight::actor_transport::{DriveBudget, UnixActorConnection};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::Error;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, Ordering};

fn connect(wire: ActorWire, exchanges: u64) -> (UnixStream, UnixActorConnection) {
    let (client, server) = UnixStream::pair().unwrap(); client.set_nonblocking(true).unwrap();
    let channel = ActorChannel::new(wire, ChannelLimits { exchanges, ..ChannelLimits::default() }).unwrap();
    (client, UnixActorConnection::new(server, channel).unwrap())
}
fn setup() -> (UnixStream, UnixActorConnection, ActorSupervisor, PublicationEndpoint) {
    let (port, supervisor, endpoint) = fixture::fixture(IntakeLimits::default());
    let (client, connection) = connect(ActorWire::new(port), 32);
    (client, connection, supervisor, endpoint)
}
fn exchange(client: &mut UnixStream, connection: &mut UnixActorConnection, command: Command) -> String {
    let mut request = encode_command(&command).unwrap(); request.push(b'\n');
    client.write_all(&request).unwrap();
    let mut reply = Vec::new(); let mut buffer = [0; 1024];
    for _ in 0..256 {
        let report = connection.drive(DriveBudget::default()).unwrap();
        assert!(report.status.failure.is_none());
        match client.read(&mut buffer) {
            Ok(0) => panic!("peer closed without the expected response"),
            Ok(n) => reply.extend_from_slice(&buffer[..n]),
            Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) => {}
            Err(error) => panic!("actor reply read failed: {error}"),
        }
        if reply.last() == Some(&b'\n') {
            assert_eq!(reply.iter().filter(|byte| **byte == b'\n').count(), 1);
            return String::from_utf8(reply).unwrap();
        }
    }
    panic!("bounded fixture did not receive its response")
}
fn submit(request: u64) -> Command { Command::Submit { request, proposal: fixture::proposal() } }
fn conserved(supervisor: &ActorSupervisor) {
    let ledger = supervisor.broker().inspect().ledger;
    assert_eq!(ledger.available + ledger.reserved + ledger.charged, 100);
}
fn private_data_absent(reply: &str) {
    for private in ["secret-helper", "secret-model", "secret-detector", "secret-cohort", "secret-salt",
        "attempt", "control_sequence", "policy_epoch", "permit", "reviewer"]
    { assert!(!reply.contains(private), "private field leaked: {private}"); }
}

#[test]
fn socket_submission_reaches_publication_only_through_original_review_and_permit() {
    let (mut client, mut connection, mut supervisor, mut endpoint) = setup();
    let pending = exchange(&mut client, &mut connection, submit(37));
    assert!(pending.contains("\"state\":\"pending\"")); private_data_absent(&pending);
    assert_eq!(endpoint.execution_count(), 0);
    let admitted = supervisor.accept_next(&fixture::snapshot()).unwrap().unwrap();
    assert_eq!(admitted.request, 37); assert_ne!(admitted.attempt, Some(37));
    let inputs = fixture::review(&mut supervisor, 37, 1);
    let permit = supervisor.authorize_request(37, Some(&inputs), &fixture::snapshot()).unwrap();
    supervisor.deliver_request(37, DispatchKeys::single(&permit), Some(&inputs), &fixture::snapshot(), &mut endpoint).unwrap();
    let observed = exchange(&mut client, &mut connection, Command::Poll { request: 37 });
    assert!(observed.contains("\"value\":\"executed\"")); private_data_absent(&observed);
    assert_eq!(endpoint.payload(), fixture::proposal().payload);
    assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(exchange(&mut client, &mut connection, submit(37)), observed);
    assert!(supervisor.accept_next(&fixture::snapshot()).unwrap().is_none());
    assert_eq!(endpoint.execution_count(), 1); conserved(&supervisor);
}

#[test]
fn reconnect_and_cancellation_cannot_refund_a_disclosed_unknown_effect() {
    let (mut client, mut connection, mut supervisor, mut endpoint) = setup();
    exchange(&mut client, &mut connection, submit(91));
    supervisor.accept_next(&fixture::snapshot()).unwrap().unwrap();
    let inputs = fixture::review(&mut supervisor, 91, 1);
    let permit = supervisor.authorize_request(91, Some(&inputs), &fixture::snapshot()).unwrap();
    let envelope = supervisor.dispatch_request(91, DispatchKeys::single(&permit), Some(&inputs), &fixture::snapshot()).unwrap();
    let receipt = endpoint.deliver(&envelope).unwrap(); supervisor.acknowledgment_lost(91).unwrap();
    drop(client);
    let (mut client, mut connection) = connect(connection.into_session(), 32);
    let cancelled = exchange(&mut client, &mut connection, Command::Cancel { request: 91 });
    assert!(cancelled.contains("outcome_unknown"));
    supervisor.synchronize().unwrap();
    assert_eq!(supervisor.broker().inspect().ledger.charged, 16);
    assert!(exchange(&mut client, &mut connection, submit(91)).contains("outcome_unknown"));
    assert!(supervisor.accept_next(&fixture::snapshot()).unwrap().is_none());
    supervisor.accept_receipt(receipt.clone()).unwrap();
    assert!(!supervisor.accept_receipt(receipt).unwrap());
    let observed = exchange(&mut client, &mut connection, Command::Poll { request: 91 });
    assert!(observed.contains("\"value\":\"executed\"")); private_data_absent(&observed);
    assert_eq!(endpoint.execution_count(), 1); assert_eq!(supervisor.broker().inspect().ledger.charged, 16);
    conserved(&supervisor);
}

#[test]
fn an_unseen_dispatch_is_refunded_only_after_the_endpoint_seals_its_key() {
    let (mut client, mut connection, mut supervisor, mut endpoint) = setup();
    exchange(&mut client, &mut connection, submit(92));
    supervisor.accept_next(&fixture::snapshot()).unwrap().unwrap();
    let inputs = fixture::review(&mut supervisor, 92, 1);
    let permit = supervisor.authorize_request(92, Some(&inputs), &fixture::snapshot()).unwrap();
    let message = supervisor.dispatch_request(92, DispatchKeys::single(&permit), Some(&inputs), &fixture::snapshot()).unwrap();
    supervisor.acknowledgment_lost(92).unwrap();
    exchange(&mut client, &mut connection, Command::Cancel { request: 92 }); supervisor.synchronize().unwrap();
    assert_eq!(supervisor.broker().inspect().ledger.charged, 16);
    let query = supervisor.broker().status_query(supervisor.attempt(92).unwrap()).unwrap();
    let receipt = endpoint.seal_unexecuted(&query).unwrap(); supervisor.accept_receipt(receipt).unwrap();
    assert_eq!(endpoint.deliver(&message).unwrap().outcome(), EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed });
    let observed = exchange(&mut client, &mut connection, Command::Poll { request: 92 });
    assert!(observed.contains("confirmed_not_executed"));
    assert_eq!(endpoint.execution_count(), 0); assert_eq!(supervisor.broker().inspect().ledger.available, 100);
    conserved(&supervisor);
}

#[test]
fn socket_ingress_cannot_override_the_human_key_requirement() {
    let (mut client, mut connection, mut supervisor, mut endpoint) = setup();
    let reviewer = supervisor.broker_mut().enable_human_review(HumanReviewPolicy {
        reviewer_id: 41, max_validity_ticks: 80, max_requests: 4,
    }).unwrap();
    exchange(&mut client, &mut connection, submit(93));
    supervisor.accept_next(&fixture::snapshot()).unwrap().unwrap();
    let inputs = fixture::review(&mut supervisor, 93, 1);
    let permit = supervisor.authorize_request(93, Some(&inputs), &fixture::snapshot()).unwrap();
    assert_eq!(supervisor.deliver_request(93, DispatchKeys::single(&permit), Some(&inputs), &fixture::snapshot(), &mut endpoint), Err(Error::Incomplete));
    assert_eq!(endpoint.execution_count(), 0); assert_eq!(supervisor.broker().inspect().ledger.reserved, 16);
    let attempt = supervisor.attempt(93).unwrap();
    let request = supervisor.broker_mut().request_human_approval(1, attempt, Some(&inputs), ElapsedTick(50)).unwrap();
    let key = reviewer.approve(&request, ElapsedTick(1)).unwrap();
    supervisor.deliver_request(93, DispatchKeys::two(&permit, &key), Some(&inputs), &fixture::snapshot(), &mut endpoint).unwrap();
    let observed = exchange(&mut client, &mut connection, Command::Poll { request: 93 });
    assert!(observed.contains("\"value\":\"executed\"")); private_data_absent(&observed);
    conserved(&supervisor);
}

#[test]
fn exact_denial_remains_terminal_for_that_key_while_new_reviewed_work_can_succeed() {
    let (mut client, mut connection, mut supervisor, mut endpoint) = setup();
    exchange(&mut client, &mut connection, submit(94));
    let mut bad = fixture::snapshot(); bad.values.insert(7, vec![8]);
    let denied = supervisor.accept_next(&bad).unwrap().unwrap().result.unwrap().unwrap();
    assert_eq!(denied.state, ActionState::Denied);
    let observed = exchange(&mut client, &mut connection, Command::Poll { request: 94 });
    assert!(observed.contains("\"value\":\"denied\"")); private_data_absent(&observed);
    assert_eq!(exchange(&mut client, &mut connection, submit(94)), observed);
    assert!(supervisor.accept_next(&fixture::snapshot()).unwrap().is_none());
    assert_eq!(endpoint.execution_count(), 0);
    exchange(&mut client, &mut connection, submit(95));
    supervisor.accept_next(&fixture::snapshot()).unwrap().unwrap();
    let inputs = fixture::review(&mut supervisor, 95, 1);
    let permit = supervisor.authorize_request(95, Some(&inputs), &fixture::snapshot()).unwrap();
    supervisor.deliver_request(95, DispatchKeys::single(&permit), Some(&inputs), &fixture::snapshot(), &mut endpoint).unwrap();
    assert_eq!(endpoint.execution_count(), 1); conserved(&supervisor);
}

#[test]
fn channel_quota_closes_transport_not_the_accepted_action() {
    let (port, mut supervisor, mut endpoint) = fixture::fixture(IntakeLimits::default());
    let (mut client, mut connection) = connect(ActorWire::new(port), 1);
    exchange(&mut client, &mut connection, submit(96));
    assert_eq!(connection.status().channel, ChannelState::Closed(CloseReason::ExchangeLimit));
    supervisor.accept_next(&fixture::snapshot()).unwrap().unwrap();
    let inputs = fixture::review(&mut supervisor, 96, 1);
    let permit = supervisor.authorize_request(96, Some(&inputs), &fixture::snapshot()).unwrap();
    supervisor.deliver_request(96, DispatchKeys::single(&permit), Some(&inputs), &fixture::snapshot(), &mut endpoint).unwrap();
    drop(client);
    let (mut client, mut connection) = connect(connection.into_session(), 8);
    assert!(exchange(&mut client, &mut connection, Command::Poll { request: 96 }).contains("\"value\":\"executed\""));
    assert!(supervisor.accept_next(&fixture::snapshot()).unwrap().is_none()); conserved(&supervisor);
}

#[test]
fn socket_reconnect_and_file_endpoint_reopen_preserve_the_original_publication() {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let mut root = None;
    for _ in 0..100 {
        let path = std::env::temp_dir().join(format!("fa-actor-unix-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        match fs::create_dir(&path) {
            Ok(()) => { root = Some(path); break; }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => panic!("cannot provision owned test directory: {error}"),
        }
    }
    let root = root.expect("unique test directory"); let directory = root.join("publication");
    let (mut endpoint, recovery) = PublicationEndpoint::create_file_publication(
        &directory, fixture::proposal().target, b"old".to_vec(), 200, 128,
        FilePublicationLimits { mutations: 100, bytes: 1_048_576 },
    ).unwrap();
    let (port, mut supervisor) = fixture::attach(&mut endpoint, IntakeLimits::default());
    let (mut client, mut connection) = connect(ActorWire::new(port), 32);
    exchange(&mut client, &mut connection, submit(97));
    supervisor.accept_next(&fixture::snapshot()).unwrap().unwrap();
    let inputs = fixture::review(&mut supervisor, 97, 1);
    let permit = supervisor.authorize_request(97, Some(&inputs), &fixture::snapshot()).unwrap();
    let message = supervisor.dispatch_request(97, DispatchKeys::single(&permit), Some(&inputs), &fixture::snapshot()).unwrap();
    endpoint.deliver(&message).unwrap(); supervisor.acknowledgment_lost(97).unwrap();
    drop(endpoint); drop(client);
    let (mut client, mut connection) = connect(connection.into_session(), 32);
    assert!(exchange(&mut client, &mut connection, Command::Poll { request: 97 }).contains("outcome_unknown"));
    let mut endpoint = recovery.reopen().unwrap(); endpoint.observe_time(ElapsedTick(2)).unwrap();
    supervisor.broker_mut().observe_time(ElapsedTick(2)).unwrap();
    supervisor.reconcile_pending(&mut endpoint).unwrap();
    let observed = exchange(&mut client, &mut connection, Command::Poll { request: 97 });
    assert!(observed.contains("\"value\":\"executed\"")); private_data_absent(&observed);
    assert_eq!(PublicationEndpoint::read_file_publication(&directory).unwrap().payload, fixture::proposal().payload);
    assert_eq!(endpoint.execution_count(), 1); conserved(&supervisor);
    drop((endpoint, recovery, supervisor, connection, client));
    fs::remove_dir_all(root).unwrap();
}
