//! Actual Unix sockets: batching cannot bypass the original server authority.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod ordinary;
#[path = "support/file_stream_wire.rs"] mod stream;
use ordinary::{Directory, snapshot};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::{Reconciliation, MAX_JOURNAL_BYTES};
use fa_reference::action::consequence::delivery::persistent::observed::stream::actor_wire::client::{
    FileActorClient, FileActorConnection, FileClientError, FileClientProfile,
};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::actor_transport::{UnixActorConnection, DriveBudget};
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, ActorChannel, ActorRequestPort, ChannelLimits, Command, WireResponse};
use fa_reference::action::consequence::oversight::actor_wire::client::{ClientSessionLimits, ClientError, ClientProgress};
use fa_reference::action::consequence::oversight::actor_wire::client::drive::ClientDriveBudget;
use std::os::unix::net::UnixStream;

fn profile(binding: &[u8]) -> FileClientProfile {
    FileClientProfile::new(binding.to_vec(), ClientSessionLimits::default(), MAX_JOURNAL_BYTES).unwrap()
}
fn sockets<P: ActorRequestPort>(state: FileActorClient, port: P) -> (FileActorConnection<UnixStream>, UnixActorConnection<P>) {
    let (client, server) = UnixStream::pair().unwrap(); client.set_nonblocking(true).unwrap();
    (state.connect(client), UnixActorConnection::new(server,
        ActorChannel::new(ActorWire::new(port), ChannelLimits::default()).unwrap()).unwrap())
}
fn exchange<P: ActorRequestPort>(client: &mut FileActorConnection<UnixStream>, server: &mut UnixActorConnection<P>,
    budget: ClientDriveBudget) -> WireResponse {
    for _ in 0..256 {
        let revision = client.revision(); let report = client.drive(revision, budget).unwrap();
        assert!(report.steps <= budget.steps());
        if report.steps != 0 { assert_eq!(client.revision(), revision + 2); }
        server.drive(DriveBudget::default()).unwrap();
        if let ClientProgress::Response(response) = report.progress { return response; }
    }
    panic!("bounded local exchange never completed")
}

#[test]
fn batched_large_request_keeps_two_key_gate_and_exact_retry_after_lost_reply() {
    for steps in [1, 16, 64] {
        let budget = ClientDriveBudget::new(steps).unwrap();
        let root = Directory::new(); let (mut host, human) = ordinary::create(&root);
        host.enable_publication_guard(host.revision()).unwrap();
        let p = ActorProposal { target: host.inspect().target, payload: vec![b'x'; 9000],
            expected_policy_epoch: 0, deadline: ElapsedTick(100), units: 16 };
        let (port, mut supervisor) = host.into_actor_gateway();
        let revision = supervisor.host().unwrap().revision(); supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
        let contract = profile(b"independent-server/raw-v1"); let directory = root.0.join("client");
        let state = FileActorClient::create(&directory, contract.clone()).unwrap();
        let (mut client, mut server) = sockets(state, port.clone());
        client.submit(0, 9000, &p).unwrap();
        // Move the complete request, but deliberately never consume its reply.
        for _ in 0..256 {
            let revision = client.revision(); client.drive(revision, budget).unwrap();
            server.drive(DriveBudget::default()).unwrap();
            if supervisor.host().unwrap().request_status(9000).is_ok() { break; }
        }
        let revision = client.revision(); drop(client); drop(server);
        let saved = FileActorClient::read(&directory, &contract, revision).unwrap();
        assert!(saved.last_response(9000).is_none()); assert_eq!(saved.original_proposal(9000), Some(&p));
        {
            let mut host = supervisor.host_mut().unwrap(); let r = host.revision();
            assert!(host.publish(r, 1).is_err()); assert_eq!(host.inspect().executions, 0);
            let action = host.request_action(9000).unwrap().clone();
            let inputs = ordinary::inputs(&action, b"whole-input large payload review");
            ordinary::review_existing(&mut host, 1, 101, &inputs);
            let r = host.revision(); let automatic = host.authorize(r, 1, &inputs, snapshot()).unwrap();
            let r = host.revision(); let request = host.request_human_approval(r, 1001, 1, &inputs, ElapsedTick(30)).unwrap();
            let r = host.revision(); let key = human.approve(&mut host, r, &request).unwrap();
            let r = host.revision(); host.dispatch(r, &automatic, &key, &action, &inputs, snapshot()).unwrap();
            let r = host.revision(); let outcome = host.publish_checked(r, 1, Some(&inputs), snapshot(), ElapsedTick(1)).unwrap();
            let r = host.revision(); assert_eq!(host.reconcile(r, 1).unwrap(), Reconciliation::Resolved(outcome.outcome));
        }
        let before = supervisor.host().unwrap().inspect();
        let state = FileActorClient::open(&directory, contract, revision).unwrap();
        let (mut client, mut server) = sockets(state, port);
        assert_eq!(client.poll(revision, 9000), Err(FileClientError::Client(ClientError::TicketUnavailable)));
        let mut changed = p.clone(); changed.deadline.0 += 1;
        assert_eq!(client.submit(revision, 9000, &changed), Err(FileClientError::Client(ClientError::Command(
            fa_reference::action::consequence::oversight::actor_wire::WireError::IdempotencyConflict))));
        client.retry_submission(revision, 9000).unwrap();
        assert!(matches!(exchange(&mut client, &mut server, budget).result,
            Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
        assert_eq!(supervisor.host().unwrap().inspect(), before);
        assert_eq!(before.executions, 1); assert_eq!(before.control.ledger.charged, 16);
        assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
    }
}

#[test]
fn batched_stream_messages_wait_for_reconciliation_and_finish_is_separate() {
    let root = stream::Directory::new(); let (port, mut supervisor, human) = stream::create(&root);
    let directory = root.0.join("client"); let contract = profile(b"server/stream7/gen1/4-1024-4096");
    let state = FileActorClient::create(&directory, contract).unwrap();
    let (mut client, mut server) = sockets(state, port.clone());
    let budget = ClientDriveBudget::default();
    for (request, attempt, message) in [(501, 1, Some("Hello 🌎")), (502, 2, Some("Second message")), (503, 3, None)] {
        let Command::Submit { proposal, .. } = stream::command(&supervisor.host().unwrap(), request, message) else { unreachable!() };
        stream::admit(&mut supervisor); let r = client.revision(); client.submit(r, request, &proposal).unwrap();
        assert!(matches!(exchange(&mut client, &mut server, budget).result, Ok(Knowledge::Pending { .. })));
        let keys = { let mut h = supervisor.host_mut().unwrap(); stream::approve(&mut h, &human, request, attempt) };
        {
            let mut h = supervisor.host_mut().unwrap(); stream::ordinary::dispatch(&mut h, &keys);
            let r = h.revision(); h.publish_checked(r, attempt, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
        }
        let r = client.revision(); client.poll(r, request).unwrap();
        assert_eq!(exchange(&mut client, &mut server, budget).result,
            Ok(Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }));
        { let mut h = supervisor.host_mut().unwrap(); let r = h.revision(); h.reconcile(r, attempt).unwrap(); }
        let r = client.revision(); client.poll(r, request).unwrap();
        assert!(matches!(exchange(&mut client, &mut server, budget).result,
            Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
        assert_eq!(supervisor.host().unwrap().stream_snapshot().unwrap().confirmed.finished(), message.is_none());
    }
    assert_eq!(supervisor.host().unwrap().stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(),
        vec!["Hello 🌎", "Second message"]);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 3);
}
