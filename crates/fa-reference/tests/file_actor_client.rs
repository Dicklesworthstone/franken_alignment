//! Actual Unix sockets and original durable effect ownership across client loss.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod ordinary;
#[path = "support/file_stream_wire.rs"] mod stream;
use ordinary::{Directory, snapshot};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::{Reconciliation, MAX_JOURNAL_BYTES};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::observed::stream::actor_wire::client::{
    FileActorClient, FileActorConnection, FileClientError, FileClientProfile,
};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::actor_transport::{UnixActorConnection, DriveBudget};
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, ActorChannel, ActorRequestPort, ChannelLimits, Command, WireResponse};
use fa_reference::action::consequence::oversight::actor_wire::client::{ClientSessionLimits, ClientError, ClientProgress};
use std::os::unix::net::UnixStream;

fn profile(binding: &[u8]) -> FileClientProfile {
    FileClientProfile::new(binding.to_vec(), ClientSessionLimits::default(), MAX_JOURNAL_BYTES).unwrap()
}
fn sockets<P: ActorRequestPort>(state: FileActorClient, port: P) -> (FileActorConnection<UnixStream>, UnixActorConnection<P>) {
    let (client, server) = UnixStream::pair().unwrap(); client.set_nonblocking(true).unwrap();
    (state.connect(client), UnixActorConnection::new(server,
        ActorChannel::new(ActorWire::new(port), ChannelLimits::default()).unwrap()).unwrap())
}
fn exchange<P: ActorRequestPort>(client: &mut FileActorConnection<UnixStream>, server: &mut UnixActorConnection<P>) -> WireResponse {
    for _ in 0..64 {
        let revision = client.revision(); let progress = client.step(revision).unwrap();
        server.drive(DriveBudget::default()).unwrap();
        if let ClientProgress::Response(response) = progress { return response; }
    }
    panic!("bounded local exchange did not complete")
}
fn publish_existing(host: &mut FileOversight, human: &FileHumanReviewer, request: u64) -> u64 {
    let action = host.request_action(request).unwrap().clone();
    let inputs = ordinary::inputs(&action, b"whole-input review after client disappeared");
    ordinary::review_existing(host, 1, 101, &inputs);
    let revision = host.revision(); let automatic = host.authorize(revision, 1, &inputs, snapshot()).unwrap();
    let revision = host.revision(); let request = host.request_human_approval(revision, 1001, 1, &inputs, ElapsedTick(30)).unwrap();
    let revision = host.revision(); let human = human.approve(host, revision, &request).unwrap();
    let revision = host.revision(); host.dispatch(revision, &automatic, &human, &action, &inputs, snapshot()).unwrap();
    let revision = host.revision(); let published = host.publish_checked(revision, 1, Some(&inputs), snapshot(), ElapsedTick(1)).unwrap();
    let revision = host.revision(); assert_eq!(host.reconcile(revision, 1).unwrap(), Reconciliation::Resolved(published.outcome));
    action.spec().units
}

#[test]
fn lost_client_reply_and_later_server_execution_recover_only_the_original_request() {
    let root = Directory::new(); let (mut host, human) = ordinary::create(&root);
    host.enable_publication_guard(host.revision()).unwrap();
    let p = ActorProposal { target: host.inspect().target, payload: b"actual request".to_vec(),
        expected_policy_epoch: 0, deadline: ElapsedTick(100), units: 16 };
    let (port, mut supervisor) = host.into_actor_gateway();
    let revision = supervisor.host().unwrap().revision(); supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
    let contract = profile(b"independent-local-server/raw-submit-v1"); let directory = root.0.join("client");
    let state = FileActorClient::create(&directory, contract.clone()).unwrap();
    let (mut client, mut server) = sockets(state, port.clone());
    client.submit(0, 9000, &p).unwrap(); assert_eq!(client.step(1).unwrap(), ClientProgress::Progress);
    assert_eq!(server.drive(DriveBudget::default()).unwrap().progress.frames, 1);
    let before = client.state().unwrap().work(); let revision = client.revision();
    drop(client); drop(server); // response was written by server but never read
    assert!(FileActorClient::read(&directory, &contract, revision).unwrap().last_response(9000).is_none());
    let units = publish_existing(&mut supervisor.host_mut().unwrap(), &human, 9000);
    let before_server = supervisor.host().unwrap().inspect(); assert_eq!(before_server.executions, 1);
    let state = FileActorClient::open(&directory, contract, revision).unwrap();
    assert_eq!(state.state().original_proposal(9000), Some(&p)); assert_eq!(state.state().work(), before);
    let (mut client, mut server) = sockets(state, port);
    assert_eq!(client.poll(client.revision(), 9000), Err(FileClientError::Client(ClientError::TicketUnavailable)));
    let r = client.revision(); client.retry_submission(r, 9000).unwrap();
    let result = exchange(&mut client, &mut server);
    assert!(matches!(result.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert_eq!(supervisor.host().unwrap().inspect(), before_server);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
    assert_eq!(before_server.control.ledger.charged, units);
    assert_eq!(client.state().unwrap().request_count(), 1);
    assert!(client.state().unwrap().work().exchanges > before.exchanges);
}

#[test]
fn stream_retry_uses_old_exact_intent_after_prefix_advances_and_finish_stays_explicit() {
    let root = stream::Directory::new(); let (port, mut supervisor, human) = stream::create(&root);
    let contract = profile(b"independent-local-server/stream-id-7-generation-1-4-1024-4096");
    let directory = root.0.join("client");
    let state = FileActorClient::create(&directory, contract.clone()).unwrap();
    let (mut client, mut server) = sockets(state, port.clone());
    let Command::Submit { proposal, .. } = stream::command(&supervisor.host().unwrap(), 501, Some("Hello 🌎")) else { unreachable!() };
    stream::admit(&mut supervisor); client.submit(0, 501, &proposal).unwrap();
    assert!(matches!(exchange(&mut client, &mut server).result, Ok(Knowledge::Pending { .. })));
    let keys = { let mut host = supervisor.host_mut().unwrap(); stream::approve(&mut host, &human, 501, 1) };
    {
        let mut host = supervisor.host_mut().unwrap(); stream::ordinary::dispatch(&mut host, &keys);
        let r = host.revision(); host.publish_checked(r, 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
    }
    let ticket = port.submit(501, &proposal).unwrap();
    assert_eq!(port.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown });
    // Client loss cannot manufacture a second message or a finish marker.
    let revision = client.revision(); drop(client); drop(server);
    { let mut host = supervisor.host_mut().unwrap(); let r = host.revision(); host.reconcile(r, 1).unwrap(); }
    let state = FileActorClient::open(&directory, contract, revision).unwrap();
    assert_eq!(state.state().original_proposal(501), Some(&proposal));
    let (mut client, mut server) = sockets(state, port);
    let r = client.revision(); client.retry_submission(r, 501).unwrap();
    assert!(matches!(exchange(&mut client, &mut server).result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert_eq!(supervisor.host().unwrap().stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), vec!["Hello 🌎"]);
    assert!(!supervisor.host().unwrap().stream_snapshot().unwrap().confirmed.finished());
    let Command::Submit { proposal: finish, .. } = stream::command(&supervisor.host().unwrap(), 502, None) else { unreachable!() };
    stream::admit(&mut supervisor); let r = client.revision(); client.submit(r, 502, &finish).unwrap();
    assert!(matches!(exchange(&mut client, &mut server).result, Ok(Knowledge::Pending { .. })));
    let mut host = supervisor.host_mut().unwrap(); let keys = stream::approve(&mut host, &human, 502, 2);
    stream::ordinary::dispatch(&mut host, &keys); let r = host.revision();
    host.publish_checked(r, 2, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
    let r = host.revision(); host.reconcile(r, 2).unwrap();
    assert!(host.stream_snapshot().unwrap().confirmed.finished()); assert_eq!(host.inspect().executions, 2);
}
