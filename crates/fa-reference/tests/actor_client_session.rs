#![cfg(unix)]
#[path = "support/file_driver.rs"] mod fixture;
use fixture::{Rig, profile, snapshot};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::Reconciliation;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileSupervisedDriver};
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileActorPort;
use fa_reference::action::consequence::oversight::actor::{ActorBasis, ActorOutcome, BasisSource, Knowledge};
use fa_reference::action::consequence::oversight::actor_wire::{ActorChannel, ActorWire, ChannelLimits, ChannelState, WireError, WireResponse, ResponseError};
use fa_reference::action::consequence::oversight::actor_wire::client::*;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;

type Channel = ActorChannel<FileActorPort<FileOversight>>;
fn connect(state: ActorClientState, port: FileActorPort<FileOversight>) -> (ActorClient<UnixStream>, Channel, UnixStream) {
    let (local, peer) = UnixStream::pair().unwrap(); peer.set_nonblocking(true).unwrap();
    let client = state.connect_unix(local).unwrap();
    let channel = ActorChannel::new(ActorWire::new(port), ChannelLimits::default()).unwrap();
    (client, channel, peer)
}
fn fresh(rig: &Rig) -> (ActorClient<UnixStream>, Channel, UnixStream) {
    connect(ActorClientState::new(ClientSessionLimits::default()).unwrap(), rig.port.clone())
}
fn prepare(driver: &mut FileSupervisedDriver) {
    let revision = driver.supervisor().host().unwrap().revision();
    driver.supervisor_mut().set_snapshot(revision, Some(snapshot())).unwrap();
}
fn serve(channel: &mut Channel, peer: &mut UnixStream, send: bool) {
    match channel.state() {
        ChannelState::Reading => {
            let mut bytes = [0; 7];
            match peer.read(&mut bytes) {
                Ok(0) => { channel.finish_input(); }
                Ok(n) => assert_eq!(channel.feed(&bytes[..n]).consumed, n),
                Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) => {}
                Err(error) => panic!("server read: {error}"),
            }
        }
        ChannelState::ReplyReady if send => {
            if let Err(error) = channel.write_once(peer) {
                assert!(matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted));
            }
        }
        _ => {}
    }
}
fn pump(client: &mut ActorClient<UnixStream>, channel: &mut Channel, peer: &mut UnixStream) -> WireResponse {
    for _ in 0..4096 {
        if let ClientProgress::Response(response) = client.step().unwrap() { return response; }
        serve(channel, peer, true);
    }
    panic!("client/server exchange exceeded fixed test bound")
}
fn known(response: &WireResponse, expected: ActorOutcome) {
    assert!(matches!(&response.result, Ok(Knowledge::Known { value, .. }) if *value == expected), "{response:?}");
}

#[test]
fn client_reaches_original_two_key_publication_and_only_reports_executed_after_reconciliation() {
    let mut rig = Rig::new(); let proposal = rig.proposal(); prepare(&mut rig.driver);
    let (mut client, mut channel, mut peer) = fresh(&rig);
    client.submit(71, &proposal).unwrap();
    assert!(matches!(pump(&mut client, &mut channel, &mut peer).result, Ok(Knowledge::Pending { request: 71 })));
    rig.reviewed(71); let key = rig.human(1001, 31);
    client.poll(71).unwrap(); assert!(matches!(pump(&mut client, &mut channel, &mut peer).result, Ok(Knowledge::Pending { .. })));
    assert!(matches!(rig.step(Some(&key)), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(rig.step(None), FileDriverEvent::Published { outcome: EndpointOutcome::Executed { .. }, .. }));
    client.poll(71).unwrap(); assert!(matches!(pump(&mut client, &mut channel, &mut peer).result, Ok(Knowledge::Unknown { .. })));
    assert!(matches!(rig.step(None), FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(EndpointOutcome::Executed { .. }), .. }));
    client.poll(71).unwrap(); known(&pump(&mut client, &mut channel, &mut peer), ActorOutcome::Executed);
    let revision = rig.driver.supervisor().host().unwrap().revision();
    client.retry_submission(71).unwrap(); known(&pump(&mut client, &mut channel, &mut peer), ActorOutcome::Executed);
    let host = rig.driver.supervisor().host().unwrap();
    assert_eq!(host.revision(), revision); assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, 16); assert_eq!(client.state().request_count(), 1);
}

#[test]
fn lost_submit_ack_reconnects_without_new_snapshot_or_duplicate_admission() {
    let mut rig = Rig::new(); let proposal = rig.proposal(); prepare(&mut rig.driver);
    let (mut client, mut channel, mut peer) = fresh(&rig); client.submit(9, &proposal).unwrap();
    for _ in 0..4096 {
        client.step().unwrap(); serve(&mut channel, &mut peer, false);
        if channel.state() == ChannelState::ReplyReady { break; }
    }
    assert_eq!(channel.state(), ChannelState::ReplyReady);
    assert!(client.state().last_response(9).is_none());
    let revision = rig.driver.supervisor().host().unwrap().revision();
    drop(channel); drop(peer);
    assert_eq!(client.step(), Err(ClientError::Io(io::ErrorKind::UnexpectedEof)));
    assert!(client.state().last_failure().unwrap().request_may_have_reached_peer);
    let work = client.state().work(); let state = client.into_state();
    assert_eq!(state.original_proposal(9), Some(&proposal)); assert!(state.interrupted_command().is_some());
    let (mut client, mut channel, mut peer) = connect(state, rig.port.clone());
    assert_eq!(client.poll(9), Err(ClientError::TicketUnavailable));
    assert_eq!(client.cancel(9), Err(ClientError::TicketUnavailable));
    assert_eq!(client.state().work(), work);
    let mut byte = [0]; assert_eq!(peer.read(&mut byte).unwrap_err().kind(), io::ErrorKind::WouldBlock);
    client.retry_submission(9).unwrap(); assert!(pump(&mut client, &mut channel, &mut peer).result.is_ok());
    assert_eq!(rig.driver.supervisor().host().unwrap().revision(), revision);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.stages.len(), 1);
    assert_eq!(client.state().work().exchanges, work.exchanges + 1);
}

#[test]
fn server_process_state_can_be_reopened_without_reviving_approvals_or_republishing() {
    let mut rig = Rig::new(); let proposal = rig.proposal(); prepare(&mut rig.driver);
    let (mut client, mut channel, mut peer) = fresh(&rig); client.submit(1, &proposal).unwrap();
    pump(&mut client, &mut channel, &mut peer);
    rig.reviewed(1); let key = rig.human(1001, 31); rig.step(Some(&key));
    assert!(matches!(rig.step(None), FileDriverEvent::Published { .. }));
    let state = client.into_state(); drop(channel); drop(peer); drop(key);
    let Rig { root, port, driver, reviewer, clients, inputs } = rig;
    drop((port, driver, reviewer, clients, inputs));
    let (mut host, reviewer) = FileOversight::open(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let (port, mut driver) = host.into_supervised_driver();
    let (mut client, mut channel, mut peer) = connect(state, port);
    let revision = driver.supervisor().host().unwrap().revision();
    client.retry_submission(1).unwrap(); assert!(matches!(pump(&mut client, &mut channel, &mut peer).result, Ok(Knowledge::Unknown { .. })));
    assert_eq!(driver.supervisor().host().unwrap().revision(), revision);
    driver.resume_reconciliation(1).unwrap();
    let result = driver.step_with_evidence(|| ElapsedTick(2), |_, _| panic!("recovery tried to acquire new review evidence"), None).unwrap();
    assert!(matches!(result, FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(EndpointOutcome::Executed { .. }), .. }));
    client.poll(1).unwrap(); known(&pump(&mut client, &mut channel, &mut peer), ActorOutcome::Executed);
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 1);
    assert_eq!(driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
    drop(reviewer);
}

#[test]
fn request_tombstones_survive_cancel_and_capacity_cannot_evict_them() {
    let mut rig = Rig::new(); let proposal = rig.proposal(); prepare(&mut rig.driver);
    let state = ActorClientState::new(ClientSessionLimits { requests: 1, retained_payload_bytes: proposal.payload.len(),
        ..ClientSessionLimits::default() }).unwrap();
    let (mut client, mut channel, mut peer) = connect(state, rig.port.clone());
    client.submit(1, &proposal).unwrap(); pump(&mut client, &mut channel, &mut peer);
    client.cancel(1).unwrap(); known(&pump(&mut client, &mut channel, &mut peer), ActorOutcome::CancelledBeforeDispatch);
    let work = client.state().work();
    assert_eq!(client.submit(2, &proposal), Err(ClientError::Limit));
    assert_eq!(client.state().work(), work);
    for field in 0..6 {
        let mut changed = proposal.clone();
        match field { 0 => changed.payload[0] ^= 1, 1 => changed.units += 1, 2 => changed.deadline.0 += 1,
            3 => changed.expected_policy_epoch += 1, 4 => changed.target.object += 1, _ => changed.target.expected_version += 1 }
        assert_eq!(client.submit(1, &changed), Err(ClientError::Command(WireError::IdempotencyConflict)));
    }
    assert_eq!(client.state().work(), work); assert_eq!(client.state().retained_payload_bytes(), proposal.payload.len());
    client.retry_submission(1).unwrap(); known(&pump(&mut client, &mut channel, &mut peer), ActorOutcome::CancelledBeforeDispatch);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn a_remote_unavailable_response_does_not_grant_ticket_visibility_or_change_original_bytes() {
    let mut rig = Rig::new(); let proposal = rig.proposal();
    let (mut client, mut channel, mut peer) = fresh(&rig); client.submit(1, &proposal).unwrap();
    assert_eq!(pump(&mut client, &mut channel, &mut peer).result, Err(WireError::Unavailable));
    assert_eq!(client.poll(1), Err(ClientError::TicketUnavailable)); assert!(client.connected());
    assert!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.stages.is_empty());
    prepare(&mut rig.driver); client.retry_submission(1).unwrap();
    assert!(matches!(pump(&mut client, &mut channel, &mut peer).result, Ok(Knowledge::Pending { .. })));
    assert_eq!(client.state().request_count(), 1); assert_eq!(client.state().original_proposal(1), Some(&proposal));
}

#[test]
fn reconnect_moves_the_same_exhausted_budget_instead_of_minting_more_attempts() {
    let mut rig = Rig::new(); let proposal = rig.proposal(); prepare(&mut rig.driver);
    let state = ActorClientState::new(ClientSessionLimits { io: ClientIoLimits { exchanges: 1, ..ClientIoLimits::default() },
        ..ClientSessionLimits::default() }).unwrap();
    let (mut client, mut channel, mut peer) = connect(state, rig.port.clone());
    client.submit(1, &proposal).unwrap(); assert_eq!(client.poll(1), Err(ClientError::Busy));
    pump(&mut client, &mut channel, &mut peer);
    let work = client.state().work(); let state = client.into_state(); drop(channel); drop(peer);
    let (mut client, _, _) = connect(state, rig.port.clone());
    assert_eq!(client.retry_submission(1), Err(ClientError::Limit));
    assert_eq!(client.state().work(), work); assert_eq!(client.state().request_count(), 1);
}

#[test]
fn contradictory_terminal_observations_close_transport_without_overwriting_history() {
    let rig = Rig::new(); let proposal = rig.proposal();
    for contradiction in 0..3 {
        let state = ActorClientState::new(ClientSessionLimits::default()).unwrap();
        let (local, mut peer) = UnixStream::pair().unwrap();
        let mut client = state.connect_unix(local).unwrap();
        let original = WireResponse { request: Some(1), result: Ok(Knowledge::Known { value: ActorOutcome::Executed,
            basis: ActorBasis { request: 1, generation: 4, source: BasisSource::ControlLedger } }) };
        let mut send = |response: &WireResponse| { let mut bytes = response.encode(); bytes.push(b'\n'); peer.write_all(&bytes).unwrap(); };
        client.submit(1, &proposal).unwrap(); send(&original);
        for _ in 0..64 { if matches!(client.step().unwrap(), ClientProgress::Response(_)) { break; } }
        assert_eq!(client.state().last_response(1), Some(&original));
        let next = match contradiction {
            0 => Knowledge::Pending { request: 1 },
            1 => Knowledge::Known { value: ActorOutcome::ConfirmedNotExecuted,
                basis: ActorBasis { request: 1, generation: 5, source: BasisSource::ControlLedger } },
            _ => Knowledge::Known { value: ActorOutcome::Executed,
                basis: ActorBasis { request: 1, generation: 3, source: BasisSource::ControlLedger } },
        };
        client.poll(1).unwrap(); send(&WireResponse { request: Some(1), result: Ok(next) });
        let mut refused = false;
        for _ in 0..64 {
            if let Err(error) = client.step() {
                assert_eq!(error, ClientError::Response(ResponseError::Binding)); refused = true; break;
            }
        }
        assert!(refused); assert!(!client.connected()); assert_eq!(client.state().last_response(1), Some(&original));
    }
}

#[test]
fn cancel_after_dispatch_is_unknown_not_a_refund_or_nonexecution_claim() {
    let mut rig = Rig::new(); let proposal = rig.proposal(); prepare(&mut rig.driver);
    let (mut client, mut channel, mut peer) = fresh(&rig);
    client.submit(1, &proposal).unwrap(); pump(&mut client, &mut channel, &mut peer);
    rig.reviewed(1); let key = rig.human(1001, 31);
    assert!(matches!(rig.step(Some(&key)), FileDriverEvent::Dispatched { .. }));
    let revision = rig.driver.supervisor().host().unwrap().revision();
    client.cancel(1).unwrap();
    assert!(matches!(pump(&mut client, &mut channel, &mut peer).result, Ok(Knowledge::Unknown { .. })));
    assert_eq!(rig.driver.supervisor().host().unwrap().revision(), revision);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
    assert!(matches!(rig.step(None), FileDriverEvent::Published { .. }));
    rig.step(None); client.poll(1).unwrap();
    known(&pump(&mut client, &mut channel, &mut peer), ActorOutcome::Executed);
}

#[path = "support/file_source_intake.rs"] mod source_fixture;
fn pump_source(client: &mut ActorClient<UnixStream>, channel: &mut Channel, peer: &mut UnixStream,
    file: &mut source_fixture::FileRig) -> (WireResponse, usize)
{
    let mut captures = 0;
    for _ in 0..4096 {
        if let ClientProgress::Response(response) = client.step().unwrap() { return (response, captures); }
        if channel.state() != ChannelState::Reading { serve(channel, peer, true); continue; }
        let mut bytes = [0; 7];
        match peer.read(&mut bytes) {
            Ok(0) => panic!("unexpected actor EOF"),
            Ok(n) => {
                let result = file.rig.driver.feed_actor_from_file(channel, &bytes[..n], &mut file.source, || ElapsedTick(1)).unwrap();
                assert_eq!(result.feed.consumed, n);
                if let Some(intake) = result.intake { assert!(intake.result.is_ok()); captures += 1; }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("actor source read: {error}"),
        }
    }
    panic!("source exchange exceeded fixed fixture limit")
}

#[test]
fn client_bytes_trigger_cold_registered_capture_and_recover_outcomes_without_the_file() {
    use fa_reference::action::consequence::oversight::policy_state::StateLimits;
    let mut file = source_fixture::cold(10, StateLimits::default());
    let proposal = file.rig.proposal();
    let (mut client, mut channel, mut peer) = connect(ActorClientState::new(ClientSessionLimits::default()).unwrap(), file.rig.port.clone());
    client.submit(1, &proposal).unwrap();
    let (reply, captures) = pump_source(&mut client, &mut channel, &mut peer, &mut file);
    assert_eq!(captures, 1); assert!(matches!(reply.result, Ok(Knowledge::Pending { .. })));
    assert_eq!(file.source.status().read_attempts, 1);
    file.reviewed(); let key = file.human(); file.dispatch(&key);
    let publication = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(1), None);
    assert!(matches!(publication.result, Ok(FileDriverEvent::PublicationChecked { publication, .. })
        if publication.outcome == EndpointOutcome::Executed { resulting_version: 2 }));
    std::fs::remove_file(&file.path).unwrap();
    let result = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(1), None);
    assert!(matches!(result.result, Ok(FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(EndpointOutcome::Executed { .. }), .. })));
    let reads = file.source.status().read_attempts;
    client.poll(1).unwrap(); let (reply, captures) = pump_source(&mut client, &mut channel, &mut peer, &mut file);
    assert_eq!(captures, 0); known(&reply, ActorOutcome::Executed);
    client.retry_submission(1).unwrap(); let (reply, captures) = pump_source(&mut client, &mut channel, &mut peer, &mut file);
    assert_eq!(captures, 0); known(&reply, ActorOutcome::Executed);
    assert_eq!(file.source.status().read_attempts, reads);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().executions, 1);
}
