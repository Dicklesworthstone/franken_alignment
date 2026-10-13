//! Real sockets: server credentials do not replace congress or effect receipts.
#![cfg(target_os = "linux")]
#[path = "support/file_oversight.rs"] mod ordinary;
#[path = "support/file_stream_wire.rs"] mod stream;
use ordinary::{Directory, snapshot};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::{Reconciliation, MAX_JOURNAL_BYTES};
use fa_reference::action::consequence::delivery::persistent::observed::stream::actor_wire::client::{
    FileClientError, FileClientProfile,
    peer::{FilePeerClient, FilePeerConnection, FilePeerClientProfile},
};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy};
use fa_reference::action::consequence::oversight::actor_transport::{UnixActorConnection, DriveBudget};
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, ActorChannel, ActorRequestPort, ChannelLimits, Command, WireResponse};
use fa_reference::action::consequence::oversight::actor_wire::client::{ClientSessionLimits, ClientError, ClientProgress};
use fa_reference::action::consequence::oversight::actor_wire::client::drive::ClientDriveBudget;
use std::{fs, os::{fd::OwnedFd, unix::net::UnixStream}};

fn profile(binding: &[u8]) -> FilePeerClientProfile {
    // Test fixture credentials come from the actual local kernel. Production
    // callers provision their expected server policy independently, never TOFU.
    let (socket, _) = UnixStream::pair().unwrap();
    let c = PeerCredentials::observe(&socket).unwrap();
    FileClientProfile::new(binding.to_vec(), ClientSessionLimits::default(), MAX_JOURNAL_BYTES).unwrap()
        .with_peer_policy(PeerPolicy::new(c.uid(), c.gid(), Some(c.pid())).unwrap()).unwrap()
}
fn sockets<P: ActorRequestPort>(client: FilePeerClient, port: P)
    -> (FilePeerConnection, UnixActorConnection<P>) {
    let (socket, server) = UnixStream::pair().unwrap();
    (client.connect(socket).unwrap(), UnixActorConnection::new(server,
        ActorChannel::new(ActorWire::new(port), ChannelLimits::default()).unwrap()).unwrap())
}
fn exchange<P: ActorRequestPort>(client: &mut FilePeerConnection, server: &mut UnixActorConnection<P>) -> WireResponse {
    for _ in 0..256 {
        let r = client.revision(); let report = client.drive(r, ClientDriveBudget::default()).unwrap();
        server.drive(DriveBudget::default()).unwrap();
        if let ClientProgress::Response(response) = report.progress { return response; }
    }
    panic!("bounded local peer exchange did not complete")
}

#[test]
fn verified_client_recovers_an_executed_request_without_a_replacement_effect() {
    let root = Directory::new(); let (mut host, human) = ordinary::create(&root);
    host.enable_publication_guard(host.revision()).unwrap();
    let proposal = ActorProposal { target: host.inspect().target, payload: b"peer-checked publication".to_vec(),
        expected_policy_epoch: 0, deadline: ElapsedTick(100), units: 16 };
    let (port, mut supervisor) = host.into_actor_gateway();
    let r = supervisor.host().unwrap().revision(); supervisor.set_snapshot(r, Some(snapshot())).unwrap();
    let contract = profile(b"independent-peer-server/raw-v1"); let directory = root.0.join("peer-client");
    let state = FilePeerClient::create(&directory, contract.clone()).unwrap();
    let (mut client, mut server) = sockets(state, port.clone());
    client.submit(0, 9000, &proposal).unwrap();
    assert_eq!(client.drive(1, ClientDriveBudget::default()).unwrap().progress, ClientProgress::Blocked);
    assert_eq!(server.drive(DriveBudget::default()).unwrap().progress.frames, 1);
    let revision = client.revision(); drop(client); drop(server);
    assert!(FilePeerClient::read(&directory, &contract, revision).unwrap().last_response(9000).is_none());
    {
        let mut host = supervisor.host_mut().unwrap(); let r = host.revision();
        assert!(host.publish(r, 1).is_err()); assert_eq!(host.inspect().executions, 0);
        let action = host.request_action(9000).unwrap().clone();
        let inputs = ordinary::inputs(&action, b"whole-input review for a verified local peer");
        ordinary::review_existing(&mut host, 1, 101, &inputs);
        let r = host.revision(); let automatic = host.authorize(r, 1, &inputs, snapshot()).unwrap();
        let r = host.revision(); let request = host.request_human_approval(r, 1001, 1, &inputs, ElapsedTick(30)).unwrap();
        let r = host.revision(); let approval = human.approve(&mut host, r, &request).unwrap();
        let r = host.revision(); host.dispatch(r, &automatic, &approval, &action, &inputs, snapshot()).unwrap();
        let r = host.revision(); let result = host.publish_checked(r, 1, Some(&inputs), snapshot(), ElapsedTick(1)).unwrap();
        let r = host.revision(); assert_eq!(host.reconcile(r, 1).unwrap(), Reconciliation::Resolved(result.outcome));
    }
    let before = supervisor.host().unwrap().inspect(); assert_eq!(before.executions, 1);
    let state = FilePeerClient::open(&directory, contract, revision).unwrap();
    // Even after recovery and real server execution, another candidate must
    // verify before any exact-key replay. Refusal returns the original client.
    let descriptor: OwnedFd = fs::File::open("/dev/null").unwrap().into();
    let state = state.connect(UnixStream::from(descriptor)).unwrap_err().client;
    assert_eq!(state.revision(), revision); assert_eq!(state.state().original_proposal(9000), Some(&proposal));
    let (mut client, mut server) = sockets(state, port);
    assert_eq!(client.poll(revision, 9000), Err(FileClientError::Client(ClientError::TicketUnavailable)));
    client.retry_submission(revision, 9000).unwrap();
    assert!(matches!(exchange(&mut client, &mut server).result,
        Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
    assert_eq!(before.control.ledger.charged, 16);
}

#[test]
fn verified_stream_still_waits_for_reconciliation_and_explicit_finish() {
    let root = stream::Directory::new(); let (port, mut supervisor, human) = stream::create(&root);
    let directory = root.0.join("peer-client");
    let contract = profile(b"peer-server/stream7/gen1/4-1024-4096");
    let state = FilePeerClient::create(&directory, contract).unwrap();
    let (mut client, mut server) = sockets(state, port);
    for (request, attempt, message) in [(501, 1, Some("Hello 🌎")), (502, 2, None)] {
        let Command::Submit { proposal, .. } = stream::command(&supervisor.host().unwrap(), request, message) else { unreachable!() };
        stream::admit(&mut supervisor); let r = client.revision(); client.submit(r, request, &proposal).unwrap();
        assert!(matches!(exchange(&mut client, &mut server).result, Ok(Knowledge::Pending { .. })));
        let keys = { let mut h = supervisor.host_mut().unwrap(); stream::approve(&mut h, &human, request, attempt) };
        {
            let mut h = supervisor.host_mut().unwrap(); stream::ordinary::dispatch(&mut h, &keys);
            let r = h.revision(); h.publish_checked(r, attempt, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
        }
        let r = client.revision(); client.poll(r, request).unwrap();
        assert_eq!(exchange(&mut client, &mut server).result,
            Ok(Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }));
        { let mut h = supervisor.host_mut().unwrap(); let r = h.revision(); h.reconcile(r, attempt).unwrap(); }
        let r = client.revision(); client.poll(r, request).unwrap();
        assert!(matches!(exchange(&mut client, &mut server).result,
            Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
        assert_eq!(supervisor.host().unwrap().stream_snapshot().unwrap().confirmed.finished(), message.is_none());
    }
    assert_eq!(supervisor.host().unwrap().stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), vec!["Hello 🌎"]);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 2);
}
