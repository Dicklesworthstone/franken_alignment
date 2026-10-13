//! Actual original request ledgers, approval gates and Unix socket transport.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod fixture;
use fixture::{Directory, snapshot};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::{FileDelivery, JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileActorSupervisor;
use fa_reference::action::consequence::oversight::actor::{ActorError, ActorOutcome, ActorProposal, Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, ActorChannel, ActorRequestPort,
    ChannelLimits, Command, WireError, encode_command};
use fa_reference::action::consequence::oversight::actor_transport::{UnixActorConnection, DriveBudget};
use fa_reference::action::consequence::oversight::actor_wire::client::{ActorClientState, ClientSessionLimits, ClientProgress};
use fa_reference::action::consequence::oversight::actor_wire::client::drive::ClientDriveBudget;
use fa_reference::Error;
use std::{fs, os::unix::net::UnixStream};

type Supervisor = FileActorSupervisor<FileOversight>;
fn proposal(h: &FileOversight) -> ActorProposal {
    ActorProposal { target: h.inspect().target, payload: b"retained".to_vec(),
        expected_policy_epoch: 0, deadline: ElapsedTick(100), units: 16 }
}
fn admit(s: &mut Supervisor) {
    let r = s.host().unwrap().revision(); s.set_snapshot(r, Some(snapshot())).unwrap();
}
fn approve(s: &mut Supervisor, human: &FileHumanReviewer) -> fixture::Keys {
    let mut h = s.host_mut().unwrap(); let action = h.request_action(700).unwrap().clone();
    let inputs = fixture::inputs(&action, b"original complete view");
    fixture::review_existing(&mut h, 1, 101, &inputs);
    let r = h.revision(); let automatic = h.authorize(r, 1, &inputs, snapshot()).unwrap();
    let r = h.revision(); let request = h.request_human_approval(r, 1001, 1, &inputs, ElapsedTick(30)).unwrap();
    let r = h.revision(); let human = human.approve(&mut h, r, &request).unwrap();
    fixture::Keys { action, inputs, automatic, human, request }
}

#[test]
fn missing_recovery_never_spends_an_installed_snapshot_or_allocates_a_request() {
    let root = Directory::new(); let (h, _) = fixture::create(&root); let p = proposal(&h);
    let (port, mut s) = h.into_actor_gateway(); let recovery = port.clone().recovery_only();
    admit(&mut s); let before = fs::read(root.store().join("delivery.bin")).unwrap();
    assert!(matches!(recovery.submit(700, &p), Err(ActorError::Withheld)));
    assert_eq!(s.host().unwrap().retained_requests(), 0);
    assert_eq!(fs::read(root.store().join("delivery.bin")).unwrap(), before);
    // Near-identical positive control: the original admitting port still owns
    // exactly the one installed observation. No second set_snapshot is needed.
    let original = port.submit(700, &p).unwrap();
    assert_eq!(port.poll(&original), Knowledge::Pending { request: 700 });
    let revision = s.host().unwrap().revision();
    let exact = recovery.submit(700, &p).unwrap();
    assert_eq!(recovery.poll(&exact), port.poll(&original));
    assert_eq!(s.host().unwrap().revision(), revision);
    assert_eq!(s.host().unwrap().retained_requests(), 1);
    assert!(matches!(recovery.submit(701, &p), Err(ActorError::Withheld)));
}

#[test]
fn exact_binding_selection_and_foreign_tickets_cannot_be_bypassed() {
    let root = Directory::new(); let (h, _) = fixture::create(&root); let p = proposal(&h);
    let (port, mut s) = h.into_actor_gateway(); admit(&mut s); let original = port.submit(700, &p).unwrap();
    let only = port.clone().recovery_only().for_request(700).unwrap();
    assert!(matches!(only.clone().for_request(701), Err(Error::Binding)));
    assert!(matches!(port.clone().recovery_only().for_request(0), Err(Error::InvalidInput)));
    assert_eq!(only.clone().for_request(700).unwrap().selected_request(), Some(700));
    let before = fs::read(root.store().join("delivery.bin")).unwrap();
    for field in 0..5 {
        let mut changed = p.clone();
        match field { 0 => changed.payload.push(b'x'), 1 => changed.deadline.0 += 1,
            2 => changed.target.expected_version += 1, 3 => changed.expected_policy_epoch += 1,
            _ => changed.units += 1 }
        assert!(matches!(only.submit(700, &changed), Err(ActorError::IdempotencyConflict)));
    }
    let mut wire = ActorWire::new(only.clone());
    assert_eq!(wire.exchange(&encode_command(&Command::Poll { request: 700 }).unwrap()).result,
        Ok(Knowledge::Withheld { authority_required: "own_request" }));
    assert_eq!(wire.exchange(&encode_command(&Command::Submit { request: 701, proposal: p.clone() }).unwrap()).result,
        Err(WireError::Withheld));
    assert_eq!(wire.exchange(&encode_command(&Command::Submit { request: 700, proposal: p.clone() }).unwrap()).result,
        Ok(Knowledge::Pending { request: 700 }));
    assert_eq!(fs::read(root.store().join("delivery.bin")).unwrap(), before);
    admit(&mut s); let other_key = port.submit(701, &p).unwrap();
    let current = s.host().unwrap().inspect();
    assert_eq!(only.poll(&other_key), Knowledge::Withheld { authority_required: "own_request" });
    assert_eq!(only.cancel(&other_key), Err(ActorError::Withheld));
    assert!(matches!(only.submit(701, &p), Err(ActorError::Withheld)));
    assert_eq!(s.host().unwrap().inspect(), current);
    let other = Directory::new(); let (h, _) = fixture::create(&other); let other_p = proposal(&h);
    let (other_port, mut other_s) = h.into_actor_gateway(); admit(&mut other_s);
    let foreign = other_port.submit(700, &other_p).unwrap();
    assert_eq!(only.poll(&foreign), Knowledge::Withheld { authority_required: "own_request" });
    assert_eq!(only.cancel(&foreign), Err(ActorError::Withheld));
    assert_eq!(only.poll(&original), Knowledge::Pending { request: 700 });
}

#[test]
fn recovered_cancellation_preserves_original_dispatch_and_execution_accounting() {
    for phase in 0..4 {
        let root = Directory::new(); let (mut h, human) = fixture::create(&root);
        h.enable_publication_guard(h.revision()).unwrap(); let p = proposal(&h);
        let (port, mut s) = h.into_actor_gateway(); admit(&mut s); port.submit(700, &p).unwrap();
        let keys = approve(&mut s, &human);
        if phase >= 1 { fixture::dispatch(&mut s.host_mut().unwrap(), &keys); }
        if phase >= 2 {
            let mut h = s.host_mut().unwrap(); let r = h.revision();
            h.publish_checked(r, 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
        }
        if phase == 3 { let mut h = s.host_mut().unwrap(); let r = h.revision(); h.reconcile(r, 1).unwrap(); }
        let recovery = port.recovery_only().for_request(700).unwrap();
        let before = s.host().unwrap().inspect(); let ticket = recovery.submit(700, &p).unwrap();
        assert_eq!(s.host().unwrap().inspect(), before, "ticket recovery mutated the original owner");
        recovery.cancel(&ticket).unwrap();
        if phase == 0 {
            assert!(matches!(recovery.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
            assert_eq!(s.host().unwrap().inspect().control.ledger.reserved, 0);
            let mut h = s.host_mut().unwrap(); let r = h.revision();
            assert!(h.dispatch(r, &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).is_err());
        } else {
            assert_eq!(s.host().unwrap().inspect(), before);
            assert_eq!(before.control.ledger.charged, 16);
            if phase < 3 { assert_eq!(recovery.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }); }
            else { assert!(matches!(recovery.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. })); }
        }
    }
}

#[test]
fn original_delivery_and_refused_requests_use_the_same_non_admitting_path() {
    let root = Directory::new(); let profile = fixture::profile().delivery;
    let mut h = FileDelivery::create(root.store(), profile.clone()).unwrap();
    h.observe_time(h.revision(), ElapsedTick(1)).unwrap();
    let p = ActorProposal { target: profile.target, payload: b"original".to_vec(),
        expected_policy_epoch: 99, deadline: ElapsedTick(100), units: 16 };
    let (port, mut s) = h.into_actor_gateway();
    let recovery = port.clone().recovery_only();
    assert!(matches!(recovery.submit(700, &p), Err(ActorError::Withheld)));
    let r = s.host().unwrap().revision(); s.set_snapshot(r, Some(snapshot())).unwrap();
    let ticket = port.submit(700, &p).unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::NotAdmitted, .. }));
    let before = fs::read(root.store().join("delivery.bin")).unwrap();
    let recovered = recovery.submit(700, &p).unwrap(); assert_eq!(recovery.poll(&recovered), port.poll(&ticket));
    assert_eq!(fs::read(root.store().join("delivery.bin")).unwrap(), before);
    drop(s);
    assert!(matches!(recovery.submit(700, &p), Err(ActorError::Unavailable)));
    assert_eq!(recovery.poll(&recovered), Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable });
    let reopened = FileDelivery::open(root.store(), profile).unwrap(); drop(reopened);
}

#[test]
fn lost_reply_recovers_exact_ticket_over_real_socket_without_admission_or_second_effect() {
    let root = Directory::new(); let (mut h, human) = fixture::create(&root);
    h.enable_publication_guard(h.revision()).unwrap(); let p = proposal(&h);
    let (port, mut s) = h.into_actor_gateway(); admit(&mut s);
    let (socket, peer) = UnixStream::pair().unwrap(); socket.set_nonblocking(true).unwrap();
    let mut c = ActorClientState::new(ClientSessionLimits::default()).unwrap().connect(socket);
    let mut server = UnixActorConnection::new(peer, ActorChannel::new(ActorWire::new(port.clone()), ChannelLimits::default()).unwrap()).unwrap();
    c.submit(700, &p).unwrap(); c.step().unwrap(); server.drive(DriveBudget::default()).unwrap();
    assert!(s.host().unwrap().request_status(700).is_ok());
    let state = c.into_state(); drop(server); assert!(state.last_response(700).is_none());
    let keys = approve(&mut s, &human); fixture::dispatch(&mut s.host_mut().unwrap(), &keys);
    { let mut h = s.host_mut().unwrap(); let r = h.revision();
      h.publish_checked(r, 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
      let r = h.revision(); assert!(matches!(h.reconcile(r, 1).unwrap(), Reconciliation::Resolved(_))); }
    let before = s.host().unwrap().inspect();
    let (socket, peer) = UnixStream::pair().unwrap(); socket.set_nonblocking(true).unwrap();
    let mut c = state.connect(socket);
    let mut server = UnixActorConnection::new(peer, ActorChannel::new(
        ActorWire::new(port.recovery_only().for_request(700).unwrap()), ChannelLimits::default()).unwrap()).unwrap();
    c.retry_submission(700).unwrap(); let mut response = None;
    for _ in 0..32 {
        let r = c.drive(ClientDriveBudget::default()).unwrap(); server.drive(DriveBudget::default()).unwrap();
        if let ClientProgress::Response(result) = r.progress { response = Some(result); break; }
    }
    assert!(matches!(response.unwrap().result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert_eq!(s.host().unwrap().inspect(), before); assert_eq!(before.executions, 1);
    assert_eq!(s.host().unwrap().retained_requests(), 1);
}

#[test]
fn unavailable_owner_and_borrow_conflict_never_produce_an_old_ticket() {
    let root = Directory::new(); let (h, _) = fixture::create(&root); let p = proposal(&h);
    let (port, mut s) = h.into_actor_gateway(); admit(&mut s); port.submit(700, &p).unwrap();
    let recovery = port.recovery_only();
    let mut h = s.host_mut().unwrap();
    assert!(matches!(recovery.submit(700, &p), Err(ActorError::Unavailable)));
    fs::write(root.store().join("delivery.pending"), b"unacknowledged staging").unwrap();
    let r = h.revision(); assert!(matches!(h.observe_time(r, ElapsedTick(2)), Err(JournalError::Io(_))));
    drop(h);
    assert!(matches!(recovery.submit(700, &p), Err(ActorError::Unavailable)));
    assert_eq!(fs::read(root.store().join("delivery.pending")).unwrap(), b"unacknowledged staging");
}
