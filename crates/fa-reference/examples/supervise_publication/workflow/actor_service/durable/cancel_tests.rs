//! Cancellation uses the actual durable request book, not a synthetic receipt.
use super::*;
use super::super::{cancel, receive};
use fa_reference::action::ActionState;
use fa_reference::action::consequence::oversight::actor::{ActorBasis, BasisSource};
use fa_reference::action::consequence::oversight::actor_wire::WireError;

fn acknowledged(root: &Directory, a: &Profile, config: &Settings)
    -> (Port, FileActorSupervisor<FileOversight>, FileHumanReviewer, u64)
{
    let (port, supervisor, reviewer, cmd) = setup(root);
    let state = FilePeerClient::create(&config.directory, config.profile.clone()).unwrap();
    let (socket, endpoint) = UnixStream::pair().unwrap();
    let mut peer = server(endpoint, port.clone()); let mut client = state.connect(socket).unwrap();
    let Command::Submit { request, proposal } = &cmd else { unreachable!() };
    client.submit(0, *request, proposal).unwrap(); let mut waits = 0;
    let response = receive(&mut client, a, config, &mut || Duration::ZERO, &mut |_| {
        waits += 1; assert!(waits < 16); peer.drive(DriveBudget::default()).unwrap();
    }).unwrap();
    assert_eq!(response.result, Ok(Knowledge::Pending { request: a.request }));
    let revision = client.revision();
    (port, supervisor, reviewer, revision)
}
fn approve(supervisor: &mut FileActorSupervisor<FileOversight>, reviewer: &FileHumanReviewer) -> fixture::Keys {
    let mut host = supervisor.host_mut().unwrap(); let action = host.request_action(9000).unwrap().clone();
    let inputs = fixture::inputs(&action, b"original full-input cancellation control");
    fixture::review_existing(&mut host, 1, 101, &inputs);
    let revision = host.revision(); let automatic = host.authorize(revision, 1, &inputs, snapshot()).unwrap();
    let revision = host.revision();
    let request = host.request_human_approval(revision, 1001, 1, &inputs, ElapsedTick(30)).unwrap();
    let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
    fixture::Keys { action, inputs, automatic, human, request }
}
fn listen(a: &Profile) -> UnixListener {
    let listener = UnixListener::bind(&a.socket).unwrap(); listener.set_nonblocking(true).unwrap(); listener
}
fn run(root: &Directory, port: &Port, listener: &UnixListener, minimum: u64,
    elapsed: &mut impl FnMut() -> Duration, checkpoint: &mut impl FnMut(), out: &mut impl Write)
    -> Result<(), String>
{
    let mut peer = None; let mut waits = 0;
    command_with(&args(root, "cancel", &[&minimum.to_string()]), None, elapsed, &mut |_| {
        waits += 1; assert!(waits < 16, "cancellation must not poll an unknown effect indefinitely");
        if peer.is_none() { let (socket, _) = listener.accept().unwrap(); peer = Some(server(socket, port.clone())); }
        peer.as_mut().unwrap().drive(DriveBudget::default()).unwrap(); checkpoint();
    }, out)
}

#[test]
fn cancellation_after_reconnect_releases_only_an_undispatched_reservation() {
    for authorized in [false, true] {
        let root = Directory::new(); let a = actor(&root); configure(&root, &a); let config = settings(&root, &a);
        let (port, mut supervisor, reviewer, minimum) = acknowledged(&root, &a, &config);
        let keys = authorized.then(|| approve(&mut supervisor, &reviewer));
        assert_eq!(supervisor.host().unwrap().inspect().control.ledger.reserved, if authorized { 16 } else { 0 });
        let listener = listen(&a); let mut out = Vec::new();
        run(&root, &port, &listener, minimum, &mut || Duration::ZERO, &mut || {}, &mut out).unwrap();
        let host = supervisor.host().unwrap(); let state = host.inspect();
        assert_eq!(state.control.ledger.stages[&1], ActionState::Cancelled);
        assert_eq!(state.control.ledger.reserved, 0); assert_eq!(state.control.ledger.charged, 0);
        assert_eq!(state.executions, 0); assert_eq!(host.retained_requests(), 1); drop(host);
        let cut = FilePeerClient::read(&config.directory, &config.profile, minimum).unwrap();
        assert!(matches!(cut.last_response(9000).unwrap().result,
            Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
        if let Some(keys) = keys {
            let mut host = supervisor.host_mut().unwrap(); let r = host.revision();
            assert!(host.dispatch(r, &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).is_err());
            assert_eq!(host.inspect().executions, 0);
        }
    }
}

#[test]
fn cancellation_cannot_refund_dispatch_or_relabel_an_executed_effect() {
    for stage in 0..3 {
        let root = Directory::new(); let a = actor(&root); configure(&root, &a); let config = settings(&root, &a);
        let (port, mut supervisor, reviewer, minimum) = acknowledged(&root, &a, &config);
        let keys = approve(&mut supervisor, &reviewer);
        {
            let mut host = supervisor.host_mut().unwrap(); fixture::dispatch(&mut host, &keys);
            if stage > 0 { let r = host.revision(); host.publish_checked(r, 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap(); }
            if stage == 2 { let r = host.revision(); host.reconcile(r, 1).unwrap(); }
        }
        let before = supervisor.host().unwrap().inspect(); let listener = listen(&a); let mut out = Vec::new();
        let error = run(&root, &port, &listener, minimum, &mut || Duration::ZERO, &mut || {}, &mut out).unwrap_err();
        let cut = FilePeerClient::read(&config.directory, &config.profile, minimum).unwrap();
        if stage == 2 {
            assert!(error.contains("executed"));
            assert!(matches!(cut.last_response(9000).unwrap().result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
            assert_eq!(cut.work().exchanges, 2, "terminal retry needs no Cancel");
        } else {
            assert!(error.contains("did not establish nonexecution"));
            assert_eq!(cut.last_response(9000).unwrap().result, Ok(Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }));
            assert_eq!(cut.work().exchanges, 3, "exact retry and one explicit Cancel only");
        }
        assert_eq!(supervisor.host().unwrap().inspect(), before);
        assert_eq!(before.control.ledger.charged, 16);
        assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
        if stage == 1 { reconcile(&mut supervisor); assert_eq!(supervisor.host().unwrap().inspect().control.ledger.charged, 16); }
    }
}

#[test]
fn unanswered_or_missing_requests_refuse_before_socket_access_and_staging_cleanup() {
    for prepared in [false, true] {
        let root = Directory::new(); let mut a = actor(&root); a.socket = root.0.join("absent").join("actor.sock");
        configure(&root, &a); let config = settings(&root, &a);
        let state = FilePeerClient::create(&config.directory, config.profile.clone()).unwrap();
        if prepared {
            let (_, _, _, cmd) = setup(&root); let (socket, _) = UnixStream::pair().unwrap();
            let mut client = state.connect(socket).unwrap();
            let Command::Submit { request, proposal } = cmd else { unreachable!() };
            client.submit(0, request, &proposal).unwrap();
        } else { drop(state); }
        let canonical = fs::read(config.directory.join("delivery.bin")).unwrap();
        let staging = config.directory.join("delivery.pending"); fs::write(&staging, b"unacknowledged staging").unwrap();
        let mut out = Vec::new();
        let error = command_with(&args(&root, "cancel", &["0"]), None, &mut || Duration::ZERO,
            &mut |_| panic!("unacknowledged cancellation cannot enter transport"), &mut out).unwrap_err();
        assert!(error.contains("requires a retained server acknowledgment")); assert!(out.is_empty());
        assert_eq!(fs::read(config.directory.join("delivery.bin")).unwrap(), canonical);
        assert_eq!(fs::read(&staging).unwrap(), b"unacknowledged staging");
    }
}

#[test]
fn lost_cancel_reply_is_recovered_by_the_same_request_without_a_second_cancellation() {
    let root = Directory::new(); let a = actor(&root); configure(&root, &a); let config = settings(&root, &a);
    let (port, supervisor, _, minimum) = acknowledged(&root, &a, &config); let listener = listen(&a);
    let expired = Cell::new(false); let mut out = Vec::new();
    let error = run(&root, &port, &listener, minimum,
        &mut || Duration::from_millis(if expired.get() { 1000 } else { 0 }), &mut || {
            if supervisor.host().unwrap().inspect().control.ledger.stages[&1] == ActionState::Cancelled { expired.set(true); }
        }, &mut out).unwrap_err();
    assert_eq!(error, EXPIRED);
    let cut = FilePeerClient::read(&config.directory, &config.profile, minimum).unwrap();
    assert_eq!(cut.interrupted_command(), Some(&Command::Cancel { request: 9000 }));
    assert_eq!(cut.last_response(9000).unwrap().result, Ok(Knowledge::Pending { request: 9000 }));
    let before = supervisor.host().unwrap().inspect(); let spent = cut.work().exchanges; out.clear();
    run(&root, &port, &listener, cut.revision(), &mut || Duration::ZERO, &mut || {}, &mut out).unwrap();
    let recovered = FilePeerClient::read(&config.directory, &config.profile, cut.revision()).unwrap();
    assert_eq!(recovered.work().exchanges, spent + 1);
    assert!(matches!(recovered.last_response(9000).unwrap().result,
        Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    assert_eq!(supervisor.host().unwrap().inspect(), before);
}

#[test]
fn deadline_after_ticket_recovery_precedes_cancel_preparation() {
    let root = Directory::new(); let a = actor(&root); let config = settings(&root, &a);
    let (port, supervisor, _, minimum) = acknowledged(&root, &a, &config);
    let state = FilePeerClient::open(&config.directory, config.profile.clone(), minimum).unwrap();
    let before = state.state().work(); let (socket, endpoint) = UnixStream::pair().unwrap();
    let mut peer = server(endpoint, port); let mut client = state.connect(socket).unwrap();
    let mut waits = 0;
    let error = cancel::exchange(&mut client, &a, &config, &mut || {
        let cut = FilePeerClient::read(&config.directory, &config.profile, minimum).unwrap();
        if cut.work().exchanges > before.exchanges && cut.interrupted_command().is_none() {
            Duration::from_millis(1000)
        } else { Duration::ZERO }
    }, &mut |_| { waits += 1; assert!(waits < 16); peer.drive(DriveBudget::default()).unwrap(); }).unwrap_err();
    assert_eq!(error, EXPIRED); assert_eq!(client.state().unwrap().work().exchanges, before.exchanges + 1);
    assert_eq!(supervisor.host().unwrap().inspect().control.ledger.stages[&1], ActionState::Reviewing);
    assert!(client.state().unwrap().interrupted_command().is_none());
}

#[test]
fn unavailable_evidence_and_extra_cancel_arguments_cannot_be_treated_as_acknowledgment() {
    let request = 9000;
    for knowledge in [Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable },
        Knowledge::Withheld { authority_required: "own_request" },
        Knowledge::Stale { generation: 1, current: 2 }, Knowledge::Absent { closed_domain: 1, frontier: 2 }] {
        let response = WireResponse { request: Some(request), result: Ok(knowledge) };
        assert!(cancel::check_record(true, Some(&response), request).is_err());
        assert!(cancel::confirmed(&response).is_err());
    }
    let response = WireResponse { request: Some(request), result: Err(WireError::Unavailable) };
    assert!(cancel::check_record(true, Some(&response), request).is_err());
    for outcome in [ActorOutcome::NotAdmitted, ActorOutcome::Denied, ActorOutcome::CancelledBeforeDispatch,
        ActorOutcome::ConfirmedNotExecuted, ActorOutcome::Executed] {
        let response = WireResponse { request: Some(request), result: Ok(Knowledge::Known { value: outcome,
            basis: ActorBasis { request, generation: 1, source: if outcome == ActorOutcome::NotAdmitted { BasisSource::Intake } else { BasisSource::ControlLedger } } }) };
        assert!(cancel::check_record(true, Some(&response), request).is_ok());
        assert!(cancel::check_record(false, Some(&response), request).is_err());
        assert!(cancel::check_record(true, Some(&response), request + 1).is_err());
        assert_eq!(cancel::confirmed(&response).is_ok(), outcome != ActorOutcome::Executed);
    }
    let root = Directory::new();
    for rest in [vec!["00"], vec!["0", "replacement.json"], vec!["0", "--peers", "peer.json"]] {
        assert!(Invocation::parse(&args(&root, "cancel", &rest), None).is_err());
    }
}

#[test]
fn output_failure_does_not_erase_acknowledged_cancel_or_send_another_command() {
    struct Broken(bool);
    impl Write for Broken {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.0 { Ok(bytes.len()) } else { Err(io::ErrorKind::BrokenPipe.into()) }
        }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    for fail_flush in [false, true] {
        let root = Directory::new(); let a = actor(&root); configure(&root, &a); let config = settings(&root, &a);
        let (port, supervisor, _, minimum) = acknowledged(&root, &a, &config); let listener = listen(&a);
        let error = run(&root, &port, &listener, minimum, &mut || Duration::ZERO, &mut || {}, &mut Broken(fail_flush)).unwrap_err();
        assert!(error.contains("report output failed"));
        let before = supervisor.host().unwrap().inspect();
        assert_eq!(before.control.ledger.stages[&1], ActionState::Cancelled);
        let cut = FilePeerClient::read(&config.directory, &config.profile, minimum).unwrap();
        assert!(matches!(cut.last_response(9000).unwrap().result,
            Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
        run(&root, &port, &listener, cut.revision(), &mut || Duration::ZERO, &mut || {}, &mut Vec::new()).unwrap();
        assert_eq!(supervisor.host().unwrap().inspect(), before);
    }
}
