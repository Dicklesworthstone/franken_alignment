//! Original stream gate, request book, clients and endpoints, not a replacement evaluator.
#![cfg(unix)]
#[path = "support/file_stream_wire.rs"] mod fixture;
use fixture::{Directory, ordinary, snapshot};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::{Reconciliation, MAX_JOURNAL_BYTES};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::observed::stream::{FileStreamProposal,
    actor_wire::{FileStreamActorRecoveryPort, encode_stream_proposal}};
use fa_reference::action::consequence::delivery::persistent::observed::stream::actor_wire::client::{FileActorClient, FileClientProfile};
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileActorSupervisor;
use fa_reference::action::consequence::delivery::stream::{ReleaseFrame, StreamProfile};
use fa_reference::action::consequence::oversight::actor::{ActorError, ActorOutcome, ActorProposal, Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::actor_transport::{DriveBudget, UnixActorConnection};
use fa_reference::action::consequence::oversight::actor_wire::{ActorRequestPort, ActorWire, ActorChannel, ChannelLimits, Command};
use fa_reference::action::consequence::oversight::actor_wire::client::{ClientSessionLimits, ClientProgress};
use fa_reference::action::consequence::oversight::actor_wire::client::drive::ClientDriveBudget;
use std::os::unix::net::UnixStream;

type Supervisor = FileActorSupervisor<FileOversight>;
fn proposal(s: &Supervisor, request: u64, message: Option<&str>) -> ActorProposal {
    let Command::Submit { proposal, .. } = fixture::command(&s.host().unwrap(), request, message) else { unreachable!() };
    proposal
}
fn executed(value: Knowledge<ActorOutcome>) { assert!(matches!(value, Knowledge::Known { value: ActorOutcome::Executed, .. })); }
fn publish(s: &mut Supervisor, human: &FileHumanReviewer, request: u64, attempt: u64, reconcile: bool) {
    let mut host = s.host_mut().unwrap();
    let keys = fixture::approve(&mut host, human, request, attempt);
    ordinary::dispatch(&mut host, &keys);
    let r = host.revision();
    host.publish_checked(r, attempt, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
    if reconcile {
        let r = host.revision(); assert!(matches!(host.reconcile(r, attempt).unwrap(), Reconciliation::Resolved(_)));
    }
}

#[test]
fn missing_stream_keys_cannot_consume_a_snapshot_or_create_finish() {
    let root = Directory::new(); let (port, mut s, _) = fixture::create(&root);
    let recovery = port.clone().recovery_only();
    let message = proposal(&s, 7, Some("hello")); let finish = proposal(&s, 8, None);
    fixture::admit(&mut s); let before = s.host().unwrap().stream_snapshot().unwrap();
    for (request, original) in [(7, &message), (8, &finish)] {
        assert!(matches!(recovery.submit(request, original), Err(ActorError::Withheld)));
    }
    assert_eq!(s.host().unwrap().stream_snapshot().unwrap(), before);
    assert_eq!(s.host().unwrap().retained_requests(), 0);
    // No replacement observation: the independently held ordinary port can use
    // the exact same still-present slot and the original stream admission path.
    let ticket = port.submit(7, &message).unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Pending { request: 7 }));
    assert_eq!(s.host().unwrap().retained_requests(), 1);
    assert!(!s.host().unwrap().stream_snapshot().unwrap().confirmed.finished());
}

#[test]
fn original_intents_recover_after_later_messages_and_finish_without_rebuilding_history() {
    let root = Directory::new(); let (port, mut s, human) = fixture::create(&root);
    let recovery = port.clone().recovery_only(); let mut saved = Vec::new();
    for (request, attempt, text) in [(7, 1, Some("é")), (8, 2, Some("second")), (9, 3, None)] {
        let original = proposal(&s, request, text); fixture::admit(&mut s);
        port.submit(request, &original).unwrap();
        let action = s.host().unwrap().request_action(request).unwrap().clone();
        let frame = ReleaseFrame::decode(&action.spec().payload).unwrap();
        assert_eq!(frame.message(), text);
        assert_ne!(original.payload, action.spec().payload);
        publish(&mut s, &human, request, attempt, true);
        saved.push((request, original, action));
    }
    let before = s.host().unwrap().stream_snapshot().unwrap();
    assert!(before.confirmed.finished()); assert_eq!(before.publication.executions, 3);
    for (request, original, action) in &saved {
        let ticket = recovery.submit(*request, original).unwrap(); executed(recovery.poll(&ticket));
        assert_eq!(s.host().unwrap().request_action(*request).unwrap(), action);
    }
    assert_eq!(s.host().unwrap().stream_snapshot().unwrap(), before);
    let original = &saved[0].1;
    for field in 0..6 {
        let mut changed = original.clone();
        match field {
            0 => changed.target.expected_version += 1,
            1 => changed.expected_policy_epoch += 1,
            2 => changed.deadline.0 += 1,
            3 => { let n = changed.payload.len(); changed.payload[n - 1] ^= 1; },
            4 => changed.units += 1,
            _ => changed.payload.push(0),
        }
        assert!(recovery.submit(7, &changed).is_err());
        assert_eq!(s.host().unwrap().stream_snapshot().unwrap(), before);
    }
    let wrong = StreamProfile::new(7, 2, 4, 1024, 4096).unwrap();
    let intent = FileStreamProposal { target: original.target, expected_policy_epoch: original.expected_policy_epoch,
        deadline: original.deadline, message: Some("é".into()) };
    assert!(recovery.submit(7, &encode_stream_proposal(wrong, &intent).unwrap()).is_err());
}

#[test]
fn narrowing_foreign_tickets_borrows_and_weak_custody_keep_original_boundaries() {
    let root = Directory::new(); let (port, mut s, _) = fixture::create(&root);
    let original = proposal(&s, 7, Some("hello")); fixture::admit(&mut s);
    let ticket = port.submit(7, &original).unwrap();
    let recovery = port.clone().recovery_only().for_request(7).unwrap();
    assert_eq!(recovery.profile(), fixture::stream()); assert_eq!(recovery.selected_request(), Some(7));
    assert!(recovery.clone().for_request(0).is_err()); assert!(recovery.clone().for_request(8).is_err());
    assert!(matches!(recovery.submit(8, &original), Err(ActorError::Withheld)));
    assert!(matches!(recovery.clone().for_request(7).unwrap().poll(&ticket), Knowledge::Pending { .. }));
    let other_root = Directory::new(); let (other, mut other_s, _) = fixture::create(&other_root);
    let p = proposal(&other_s, 7, Some("hello")); fixture::admit(&mut other_s);
    let foreign = other.submit(7, &p).unwrap();
    assert!(matches!(recovery.poll(&foreign), Knowledge::Withheld { .. }));
    assert_eq!(recovery.cancel(&foreign), Err(ActorError::Withheld));
    {
        let _borrow = s.host_mut().unwrap();
        assert!(matches!(recovery.submit(7, &original), Err(ActorError::Unavailable)));
    }
    assert!(recovery.submit(7, &original).is_ok());
    drop(s);
    assert!(matches!(recovery.submit(7, &original), Err(ActorError::Unavailable)));
    assert_eq!(recovery.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable });
    let (host, _) = FileOversight::open_stream(root.store(), fixture::profile(), fixture::stream()).unwrap();
    assert_eq!(host.retained_requests(), 1, "weak ports must not retain exclusive custody");
}

#[test]
fn cancellation_and_unknown_publication_keep_original_stream_liabilities() {
    for mode in 0..4 {
        let root = Directory::new(); let (port, mut s, human) = fixture::create(&root);
        let original = proposal(&s, 7, Some("hello")); fixture::admit(&mut s);
        port.submit(7, &original).unwrap();
        let charge = s.host().unwrap().request_action(7).unwrap().spec().units;
        let keys = { let mut h = s.host_mut().unwrap(); fixture::approve(&mut h, &human, 7, 1) };
        if mode > 0 { let mut h = s.host_mut().unwrap(); ordinary::dispatch(&mut h, &keys); }
        if mode > 1 {
            let mut h = s.host_mut().unwrap(); let r = h.revision();
            h.publish_checked(r, 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
            if mode == 3 { let r = h.revision(); h.reconcile(r, 1).unwrap(); }
        }
        let recovery = port.recovery_only().for_request(7).unwrap();
        let ticket = recovery.submit(7, &original).unwrap();
        recovery.cancel(&ticket).unwrap();
        let state = s.host().unwrap().stream_snapshot().unwrap();
        assert!(!state.confirmed.finished()); assert!(!state.published.finished());
        if mode == 0 {
            assert!(matches!(recovery.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
            assert_eq!(state.publication.control.ledger.reserved, 0);
            assert_eq!(state.publication.control.ledger.charged, 0);
            let mut h = s.host_mut().unwrap(); let r = h.revision();
            assert!(h.dispatch(r, &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).is_err());
        } else {
            assert_eq!(state.publication.control.ledger.charged, charge);
            if mode == 3 { executed(recovery.poll(&ticket)); }
            else { assert_eq!(recovery.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }); }
            assert_eq!(state.published.message_count(), usize::from(mode > 1));
        }
    }
}

#[test]
fn retained_stream_refusals_survive_recovery_without_a_fictional_action() {
    let root = Directory::new(); let (port, mut s, _) = fixture::create(&root);
    let mut second = proposal(&s, 8, Some("refused"));
    second.expected_policy_epoch = 1;
    {
        // Original supervisor admission with a stale epoch yields a retained
        // refusal without a frozen action. This is not a made-up request record.
        let mut host = s.host_mut().unwrap();
        let mut spec = host.stream_message_spec("refused", second.deadline).unwrap();
        spec.policy_epoch = second.expected_policy_epoch;
        let r = host.revision(); host.submit_request(r, 8, spec, snapshot()).unwrap();
    }
    let refused = port.submit(8, &second).unwrap();
    assert!(matches!(port.poll(&refused), Knowledge::Known { value: ActorOutcome::NotAdmitted, .. }));
    assert!(s.host().unwrap().request_action(8).is_err());
    drop(s);
    let (host, _) = FileOversight::open_stream(root.store(), fixture::profile(), fixture::stream()).unwrap();
    let (port, s) = host.into_stream_actor_gateway().unwrap();
    let before = s.host().unwrap().stream_snapshot().unwrap();
    let recovery = port.recovery_only(); let ticket = recovery.submit(8, &second).unwrap();
    assert!(matches!(recovery.poll(&ticket), Knowledge::Known { value: ActorOutcome::NotAdmitted, .. }));
    assert_eq!(s.host().unwrap().stream_snapshot().unwrap(), before);
    assert_eq!(s.host().unwrap().retained_requests(), 1);
}

#[test]
fn durable_client_replays_unanswered_message_intent_over_non_admitting_unix_transport() {
    let root = Directory::new(); let (port, mut s, human) = fixture::create(&root);
    let original = proposal(&s, 7, Some("hello"));
    let directory = root.0.join("client");
    let config = FileClientProfile::new(b"exact-server/stream7/intent-v1".to_vec(), ClientSessionLimits::default(), MAX_JOURNAL_BYTES).unwrap();
    let state = FileActorClient::create(&directory, config.clone()).unwrap();
    let (client_socket, _) = UnixStream::pair().unwrap();
    let mut client = state.connect(client_socket); client.submit(0, 7, &original).unwrap();
    drop(client); // No response is known; only the exact intent is retained.
    fixture::admit(&mut s); port.submit(7, &original).unwrap(); publish(&mut s, &human, 7, 1, true);
    let second = proposal(&s, 8, Some("second")); fixture::admit(&mut s);
    port.submit(8, &second).unwrap(); publish(&mut s, &human, 8, 2, true);
    let before = s.host().unwrap().stream_snapshot().unwrap();
    let state = FileActorClient::open(&directory, config.clone(), 1).unwrap();
    let (client_socket, server_socket) = UnixStream::pair().unwrap(); client_socket.set_nonblocking(true).unwrap();
    let mut client = state.connect(client_socket);
    let recovery: FileStreamActorRecoveryPort = port.recovery_only().for_request(7).unwrap();
    let mut server = UnixActorConnection::new(server_socket,
        ActorChannel::new(ActorWire::new(recovery), ChannelLimits::default()).unwrap()).unwrap();
    client.retry_submission(1, 7).unwrap(); let mut done = false;
    for _ in 0..32 {
        let r = client.revision(); let report = client.drive(r, ClientDriveBudget::default()).unwrap();
        server.drive(DriveBudget::default()).unwrap();
        if let ClientProgress::Response(response) = report.progress {
            executed(response.result.unwrap()); done = true; break;
        }
    }
    assert!(done); assert_eq!(s.host().unwrap().stream_snapshot().unwrap(), before);
    assert_eq!(s.host().unwrap().retained_requests(), 2); assert_eq!(before.publication.executions, 2);
    let cut = FileActorClient::read(&directory, &config, client.revision()).unwrap();
    assert_eq!(cut.original_proposal(7), Some(&original)); assert!(cut.reserved_work().is_none());
    assert_eq!(before.confirmed.messages().collect::<Vec<_>>(), vec!["hello", "second"]);
}
