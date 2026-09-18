//! Real executable + inherited socket + original durable two-key effect path.
#![cfg(unix)]
#[path = "support/actor_process.rs"] mod support;
use support::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::Reconciliation;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::actor_process::*;
use fa_reference::action::consequence::oversight::actor_transport::{DriveBudget, MAX_DRIVE_FRAMES};
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, ChannelLimits, Command, WireError, encode_command};
use fa_reference::action::consequence::oversight::actor_wire::client::{ActorClientState, ClientSessionLimits, ClientProgress};
use std::collections::BTreeMap;
use std::process::{Command as OsCommand, Stdio};
use std::time::{Duration, Instant};

// Re-executed by the launcher as a REAL child. The ordinary suite invocation
// has no child mode and performs no launch. This is an analytic actor fixture,
// not a claim to have executed model inference or an OS sandbox.
#[test]
fn actor_child_fixture() {
    let Some(mode) = std::env::var_os("FA_ACTOR_CHILD") else { return; };
    assert!(std::env::var_os("PATH").is_none(), "ambient environment was inherited");
    let state = ActorClientState::new(ClientSessionLimits::default()).unwrap();
    if mode == "pipe" {
        let error = state.connect_process_stdin().unwrap_err();
        assert_eq!(error.state.request_count(), 0);
        return;
    }
    let mut client = state.connect_process_stdin().unwrap();
    // These protocol-looking diagnostics MUST NOT reach the request socket.
    println!("{{\"version\":1,\"operation\":\"cancel\",\"request\":\"9000\"}}");
    eprintln!("actor fixture private diagnostic");
    client.submit(9000, &proposal()).unwrap();
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        assert!(Instant::now() < until, "actor never received request response");
        if let ClientProgress::Response(response) = client.step().unwrap() {
            assert!(matches!(response.result, Ok(Knowledge::Pending { request: 9000 })));
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    if mode == "exit" { return; }
    if mode == "pending" {
        std::thread::sleep(Duration::from_secs(20));
        panic!("supervisor did not terminate pending actor");
    }
    assert_eq!(mode, "execute");
    loop {
        assert!(Instant::now() < until, "actor never observed original reconciliation");
        client.poll(9000).unwrap();
        loop {
            if let ClientProgress::Response(response) = client.step().unwrap() {
                if matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })) { return; }
                assert!(matches!(response.result, Ok(Knowledge::Pending { .. } | Knowledge::Unknown { .. })));
                break;
            }
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(1));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn child_request_reaches_original_congress_human_publication_and_reconciliation() {
    let root = Directory::new(); let (port, mut supervisor, human, mut process) = setup(&root, "execute");
    assert!(process.socket_fd().is_some());
    submitted(&mut process, &supervisor);
    let ticket = port.submit(9000, &proposal()).unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Pending { .. }));
    {
        let mut host = supervisor.host_mut().unwrap();
        let r = host.revision();
        assert!(host.publish(r, 1).is_err());
        assert_eq!(host.inspect().executions, 0);
        let keys = approve(&mut host, &human); ordinary::dispatch(&mut host, &keys);
        let r = host.revision();
        let published = host.publish_checked(r, 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
        assert_eq!(published.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    }
    assert_eq!(port.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown });
    {
        let mut host = supervisor.host_mut().unwrap(); let r = host.revision();
        assert_eq!(host.reconcile(r, 1).unwrap(), Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }));
    }
    let until = Instant::now() + Duration::from_secs(20);
    while process.poll().child.exit.is_none() {
        if !process.status().ingress_closed {
            match process.drive(DriveBudget::default()) {
                Ok(_) => {}
                Err(WireError::Unavailable) => assert!(process.status().child.exit.is_some()),
                Err(error) => panic!("unexpected actor transport error: {error:?}"),
            }
        }
        assert!(Instant::now() < until, "actor did not exit after reconciliation");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(process.status().child.exit.unwrap().success);
    assert!(!process.status().child.termination_sent);
    let mut session = process.into_session().unwrap();
    let response = session.exchange(&encode_command(&Command::Poll { request: 9000 }).unwrap());
    assert!(matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert_eq!(supervisor.host().unwrap().inspect().control.ledger.charged, 16);
}

#[test]
fn child_termination_does_not_cancel_or_refund_an_original_dispatched_effect() {
    let root = Directory::new(); let (port, mut supervisor, human, mut process) = setup(&root, "pending");
    submitted(&mut process, &supervisor);
    let ticket = port.submit(9000, &proposal()).unwrap();
    { let mut host = supervisor.host_mut().unwrap(); let keys = approve(&mut host, &human); ordinary::dispatch(&mut host, &keys); }
    let before = supervisor.host().unwrap().inspect();
    let status = process.request_stop(); assert!(status.ingress_closed && status.child.stop_requested);
    reap(&mut process);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(port.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown });
    assert_eq!(process.drive(DriveBudget::default()), Err(WireError::Withheld));
    let mut host = supervisor.host_mut().unwrap(); let r = host.revision();
    assert_eq!(host.seal_unexecuted(r, 1).unwrap(), Reconciliation::Resolved(EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed }));
    assert_eq!(host.inspect().control.ledger.charged, 0);
}

#[test]
fn successful_child_exit_is_not_effect_success_or_implicit_cancellation() {
    let root = Directory::new(); let (port, supervisor, _, mut process) = setup(&root, "exit");
    submitted(&mut process, &supervisor);
    let ticket = port.submit(9000, &proposal()).unwrap();
    let before = supervisor.host().unwrap().inspect();
    reap(&mut process);
    assert!(process.status().child.exit.unwrap().success);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert!(matches!(port.poll(&ticket), Knowledge::Pending { .. }));
    assert_eq!(before.executions, 0);
}

#[test]
fn refused_launch_keeps_existing_tickets_and_invalid_drive_budget_touches_nothing() {
    let root = Directory::new(); let (host, _) = ordinary::create(&root);
    let (port, mut supervisor) = host.into_actor_gateway();
    let r = supervisor.host().unwrap().revision(); supervisor.set_snapshot(r, Some(snapshot())).unwrap();
    let mut wire = ActorWire::new(port);
    let submit = Command::Submit { request: 9000, proposal: proposal() };
    assert!(wire.exchange(&encode_command(&submit).unwrap()).result.is_ok());
    let missing = ActorProgram::new(root.0.join("not-an-executable"), root.0.clone(), vec![], BTreeMap::new()).unwrap();
    let mut failed = Process::launch(&missing, wire, ChannelLimits::default()).unwrap_err();
    assert!(matches!(failed.failure, ActorLaunchFailure::Process(ProcessFailure::Io { stage: ProcessStage::InspectProgram, .. })));
    assert!(matches!(failed.session.exchange(&encode_command(&Command::Poll { request: 9000 }).unwrap()).result,
        Ok(Knowledge::Pending { .. })));
    let failed = Process::launch(&program("pending"), failed.session, ChannelLimits { frame_bytes: 0, exchanges: 1 }).unwrap_err();
    assert_eq!(failed.failure, ActorLaunchFailure::Protocol(WireError::MalformedRequest));
    let mut process = Process::launch(&program("pending"), failed.session, ChannelLimits::default()).unwrap();
    let before = process.status();
    assert_eq!(process.drive(DriveBudget { frames: MAX_DRIVE_FRAMES + 1, ..DriveBudget::default() }), Err(WireError::Capacity));
    assert_eq!(process.status(), before);
    let mut process = *process.into_session().unwrap_err();
    process.request_stop(); reap(&mut process);
    assert!(process.into_session().is_ok());
}

#[test]
fn ordinary_pipe_stdin_is_refused_and_program_text_is_not_disclosed_in_debug() {
    let status = OsCommand::new(std::env::current_exe().unwrap()).args(["--exact", "actor_child_fixture", "--nocapture"])
        .env_clear().env("FA_ACTOR_CHILD", "pipe").stdin(Stdio::null()).status().unwrap();
    assert!(status.success());
    let program = ActorProgram::new(std::env::current_exe().unwrap(), std::env::current_dir().unwrap(),
        vec!["private-actor-argument".into()], BTreeMap::from([("TOKEN".into(), "private-secret-value".into())])).unwrap();
    let text = format!("{program:?}");
    assert!(!text.contains("private-actor-argument")); assert!(!text.contains("private-secret-value"));
    assert!(ActorProgram::new("relative".into(), "/".into(), vec![], BTreeMap::new()).is_err());
    assert!(ActorProgram::new("/x".into(), "/".into(), vec![], BTreeMap::from([("BAD=KEY".into(), "v".into())])).is_err());
}
