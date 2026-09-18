//! Actual raw/stream child clients through source acquisition and native stopping.
#![cfg(unix)]
#[path = "support/actor_process.rs"] mod process_support;
#[path = "support/file_stream_source.rs"] mod source_support;
use source_support::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::{EndpointOutcome, StopRequest};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::source::FileSourcePolicy;
use fa_reference::action::consequence::delivery::persistent::observed::stream::{FileStreamProposal,
    actor_wire::{FileStreamActorPort, encode_stream_proposal, process::{ActorProcessWithdrawal, FileActorProcessError}}};
use fa_reference::action::consequence::oversight::actor::{ActorProposal, ActorOutcome, Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::actor_process::ActorProcess;
use fa_reference::action::consequence::oversight::actor_transport::DriveBudget;
use fa_reference::action::consequence::oversight::actor_wire::{ActorRequestPort, ActorWire, ChannelLimits, WireError};
use fa_reference::action::consequence::oversight::actor_wire::client::{ActorClientState, ClientSessionLimits, ClientProgress};
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use fa_reference::action::consequence::oversight::policy_state::{StateFreshness, StateLimits, StateSource};
use fa_reference::Error;
use std::time::{Duration, Instant};

fn proposal() -> ActorProposal {
    encode_stream_proposal(stream::stream(), &FileStreamProposal { target: profile().delivery.target,
        expected_policy_epoch: 0, deadline: ElapsedTick(100), message: Some("child stream".into()) }).unwrap()
}
#[test]
fn actor_child_fixture() {
    let Some(mode) = std::env::var_os("FA_ACTOR_CHILD") else { return; };
    let mode = mode.to_str().unwrap();
    let mut client = ActorClientState::new(ClientSessionLimits::default()).unwrap().connect_process_stdin().unwrap();
    let mut submitted = if mode == "source-raw" { process_support::proposal() } else { proposal() };
    if mode == "malformed" { submitted.payload = vec![0]; submitted.units = 1; }
    client.submit(9000, &submitted).unwrap();
    let until = Instant::now() + Duration::from_secs(20);
    let mut retried = false;
    loop {
        assert!(Instant::now() < until, "child source-protocol deadline");
        if let ClientProgress::Response(response) = client.step().unwrap() {
            if mode == "malformed" { assert_eq!(response.result, Err(WireError::MalformedRequest)); return; }
            if mode == "source-pending" {
                assert!(matches!(response.result, Ok(Knowledge::Pending { .. })));
                std::thread::sleep(Duration::from_secs(20));
                panic!("actor was not stopped");
            }
            match response.result {
                Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }) => return,
                Ok(Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }) if !retried => {
                    client.retry_submission(9000).unwrap(); retried = true;
                }
                Ok(Knowledge::Pending { .. } | Knowledge::Unknown { .. }) => { client.poll(9000).unwrap(); }
                other => panic!("unexpected original actor response: {other:?}"),
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn launch(port: FileStreamActorPort, mode: &str) -> ActorProcess<FileStreamActorPort> {
    ActorProcess::launch(&process_support::program(mode), ActorWire::new(port), ChannelLimits::default()).unwrap()
}
fn await_stream_request(actor: &mut ActorProcess<FileStreamActorPort>,
    driver: &mut fa_reference::action::consequence::delivery::persistent::observed::driver::FileSupervisedDriver,
    source: &mut FileEvidenceSource) {
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        let result = driver.drive_stream_process_from_file(actor, source, || ElapsedTick(1), DriveBudget::default()).unwrap();
        assert!(result.withdrawal.is_none());
        if driver.supervisor().host().unwrap().request_status(9000).is_ok() { break; }
        assert!(Instant::now() < until); std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn stream_child_uses_actual_source_then_read_free_retry_and_original_receipt() {
    let root = Directory::new(); let (port, mut driver, human, mut source, evidence) = create(&root);
    let mut actor = launch(port.clone(), "source-stream");
    await_stream_request(&mut actor, &mut driver, &mut source);
    assert_eq!(source.status().read_attempts, 1);
    let ticket = port.submit(9000, &proposal()).unwrap();
    let keys = review_dispatch(&mut driver, &human, &evidence, 9000, 1);
    {
        let mut host = driver.supervisor_mut().host_mut().unwrap(); let r = host.revision();
        assert_eq!(host.publish_checked(r, 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap().outcome,
            EndpointOutcome::Executed { resulting_version: 2 });
    }
    assert_eq!(port.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown });
    std::fs::remove_file(root.0.join("private-evidence.json")).unwrap();
    let until = Instant::now() + Duration::from_secs(20); let mut frames = 0;
    while frames < 6 {
        let report = driver.drive_stream_process_from_file(&mut actor, &mut source,
            || panic!("poll/retry acquired source time"), DriveBudget::default()).unwrap();
        assert!(report.intakes.is_empty());
        if let Ok(Some(drive)) = report.transport { frames += drive.progress.frames; }
        assert!(Instant::now() < until); std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(source.status().read_attempts, 1);
    {
        let mut host = driver.supervisor_mut().host_mut().unwrap(); let r = host.revision();
        assert_eq!(host.reconcile(r, 1).unwrap(), Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }));
    }
    while actor.poll().child.exit.is_none() {
        let report = driver.drive_stream_process_from_file(&mut actor, &mut source,
            || panic!("reconciliation observation acquired source"), DriveBudget::default()).unwrap();
        assert!(report.intakes.is_empty());
        assert!(Instant::now() < until); std::thread::sleep(Duration::from_millis(1));
    }
    assert!(actor.status().child.exit.unwrap().success);
    assert_eq!(driver.supervisor().host().unwrap().stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), vec!["child stream"]);
}

#[test]
fn malformed_stream_and_foreign_supervisors_cannot_consume_sources_or_stop_other_actors() {
    let root = Directory::new(); let (port, mut driver, _, mut source, _) = create(&root);
    let other = Directory::new(); let (_, mut foreign, _, mut other_source, _) = create(&other);
    let mut actor = launch(port, "malformed"); let before = actor.status();
    assert!(matches!(foreign.drive_stream_process_from_file(&mut actor, &mut other_source,
        || panic!("foreign time"), DriveBudget::default()), Err(FileActorProcessError::Journal(JournalError::Contract(Error::Binding)))));
    assert_eq!(actor.status(), before); assert_eq!(other_source.status().read_attempts, 0);
    let until = Instant::now() + Duration::from_secs(20);
    while actor.poll().child.exit.is_none() {
        let report = driver.drive_stream_process_from_file(&mut actor, &mut source,
            || panic!("malformed intent accessed source"), DriveBudget::default()).unwrap();
        assert!(report.intakes.is_empty());
        assert!(Instant::now() < until); std::thread::sleep(Duration::from_millis(1));
    }
    assert!(actor.status().child.exit.unwrap().success);
    assert_eq!(source.status().read_attempts, 0);
    assert!(driver.supervisor().host().unwrap().inspect().control.ledger.stages.is_empty());
}

#[test]
fn native_stop_terminates_child_but_only_native_drain_resolves_published_or_missing_effects() {
    for published in [false, true] {
        let root = Directory::new(); let (port, mut driver, human, mut source, evidence) = create(&root);
        let mut actor = launch(port.clone(), "source-pending");
        await_stream_request(&mut actor, &mut driver, &mut source);
        let ticket = port.submit(9000, &proposal()).unwrap();
        let keys = review_dispatch(&mut driver, &human, &evidence, 9000, 1);
        let units = keys.action.spec().units;
        {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            if published {
                let r = host.revision();
                assert_eq!(host.publish_checked(r, 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap().outcome,
                    EndpointOutcome::Executed { resulting_version: 2 });
            }
            let c = host.inspect().control; let r = host.revision();
            host.request_stop(r, StopRequest { operation: 81, expected_control_sequence: c.sequence,
                expected_authority_epoch: c.ledger.epoch }).unwrap();
        }
        let before = driver.supervisor().host().unwrap().inspect();
        let result = driver.drive_stream_process_from_file(&mut actor, &mut source,
            || panic!("stopping used source"), DriveBudget::default()).unwrap();
        assert_eq!(result.withdrawal, Some(ActorProcessWithdrawal::TerminalStop));
        assert!(result.transport.unwrap().is_none()); assert!(result.process.child.stop_requested);
        process_support::reap(&mut actor);
        assert_eq!(driver.supervisor().host().unwrap().inspect(), before);
        assert_eq!(before.control.ledger.charged, units);
        assert_eq!(port.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown });
        let mut host = driver.supervisor_mut().host_mut().unwrap(); let r = host.revision();
        let sweep = host.progress_stop(r, ElapsedTick(2)).unwrap(); assert!(sweep.progress.drained());
        assert_eq!(host.inspect().control.ledger.charged, if published { units } else { 0 });
        assert_eq!(host.inspect().executions, u64::from(published));
    }
}

#[test]
fn unacknowledged_source_and_caught_intake_unwind_withdraw_process_without_replaying_request() {
    for interrupted in [false, true] {
        let root = Directory::new(); let (port, mut driver, _, mut source, _) = create(&root);
        let mut actor = launch(port, "source-pending");
        if !interrupted { std::fs::write(root.store().join("delivery.pending"), b"retain staged evidence").unwrap(); }
        let until = Instant::now() + Duration::from_secs(20);
        loop {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                driver.drive_stream_process_from_file(&mut actor, &mut source, || {
                    assert!(!interrupted, "deliberate clock interruption"); ElapsedTick(1)
                }, DriveBudget::default())
            }));
            if interrupted && result.is_err() {
                assert!(actor.status().interrupted_drive);
                let reads = source.status().read_attempts;
                let next = driver.drive_stream_process_from_file(&mut actor, &mut source,
                    || panic!("interrupted intake was replayed"), DriveBudget::default()).unwrap();
                assert_eq!(next.withdrawal, Some(ActorProcessWithdrawal::InterruptedDrive));
                assert_eq!(source.status().read_attempts, reads);
                break;
            }
            let report = result.unwrap().unwrap();
            if report.withdrawal.is_some() {
                assert!(!interrupted);
                assert_eq!(report.withdrawal, Some(ActorProcessWithdrawal::OwnerUnavailable));
                assert!(driver.supervisor().host().unwrap().storage_failure().is_some());
                assert_eq!(report.intakes.len(), 1);
                assert!(report.intakes[0].result.is_err());
                break;
            }
            assert!(Instant::now() < until); std::thread::sleep(Duration::from_millis(1));
        }
        assert!(actor.status().ingress_closed && actor.status().child.stop_requested);
        process_support::reap(&mut actor);
        assert!(driver.supervisor().host().unwrap().inspect().control.ledger.stages.is_empty());
        assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
    }
}

#[test]
fn raw_process_uses_the_same_registered_source_and_two_key_owner() {
    let root = Directory::new(); let p = ordinary::profile();
    let (mut host, human) = FileOversight::create(root.store(), p.clone()).unwrap();
    host.enable_file_source(host.revision(), FileSourcePolicy {
        source: StateSource { scope: p.delivery.scope, source: 17, generation: 1 },
        limits: StateLimits::default(), freshness: StateFreshness::new(50).unwrap(),
    }).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let evidence = EvidenceSnapshot::new(EvidenceIdentity { source: 17, generation: 1, scope: p.delivery.scope }, snapshot(),
        ordinary::MEMBERS.into_iter().map(|m| (m.to_owned(), b"source".to_vec())).collect()).unwrap();
    let path = root.0.join("raw-source.json"); std::fs::write(&path, evidence.encode()).unwrap();
    let mut source = FileEvidenceSource::new(path, 17, p.delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    let (port, mut driver) = host.into_supervised_driver();
    let mut actor = ActorProcess::launch(&process_support::program("source-raw"), ActorWire::new(port), ChannelLimits::default()).unwrap();
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        driver.drive_actor_process_from_file(&mut actor, &mut source, || ElapsedTick(1), DriveBudget::default()).unwrap();
        if driver.supervisor().host().unwrap().request_status(9000).is_ok() { break; }
        assert!(Instant::now() < until); std::thread::sleep(Duration::from_millis(1));
    }
    let keys = review_dispatch(&mut driver, &human, &evidence, 9000, 1);
    {
        let mut host = driver.supervisor_mut().host_mut().unwrap(); let r = host.revision();
        let result = host.publish_checked(r, 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
        assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
        let r = host.revision(); assert_eq!(host.reconcile(r, 1).unwrap(), Reconciliation::Resolved(result.outcome));
    }
    while actor.poll().child.exit.is_none() {
        driver.drive_actor_process_from_file(&mut actor, &mut source, || panic!("poll reread source"), DriveBudget::default()).unwrap();
        assert!(Instant::now() < until); std::thread::sleep(Duration::from_millis(1));
    }
    assert!(actor.status().child.exit.unwrap().success);
    assert_eq!(source.status().read_attempts, 1);
}
