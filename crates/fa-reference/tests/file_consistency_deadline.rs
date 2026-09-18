//! Silence drives native containment; it cannot become an observed action.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod ordinary;
#[path = "support/file_consistency.rs"] mod prediction;
#[path = "support/actor_process.rs"] mod process_support;
use ordinary::{Directory, profile, snapshot};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::observed::consistency::FileConsistencyObserver;
use fa_reference::action::consequence::delivery::persistent::observed::stream::actor_wire::process::ActorProcessWithdrawal;
use fa_reference::action::consequence::oversight::consistency::{ConsistencyDeadline, ConsistencyStopPolicy};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::actor_process::ActorProcess;
use fa_reference::action::consequence::oversight::actor_transport::DriveBudget;
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, ChannelLimits, WireError};
use fa_reference::action::consequence::oversight::actor_wire::client::{ActorClientState, ClientSessionLimits, ClientProgress};
use fa_reference::action::consequence::oversight::evidence_source::{FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use fa_reference::Error;
use std::time::{Duration, Instant};

fn owner(root: &Directory) -> (FileOversight, FileHumanReviewer, FileConsistencyObserver) {
    let (mut host, human) = ordinary::create(root);
    let config = prediction::configuration().with_terminal_stop(ConsistencyStopPolicy::new(91, 1, 7007).unwrap()).unwrap();
    let observer = host.enable_action_consistency(host.revision(), config).unwrap();
    (host, human, observer)
}
fn forecast_request(host: &mut FileOversight, observer: &FileConsistencyObserver, key: u64, sequence: u64) -> ConsistencyDeadline {
    let frame = prediction::frame(host, sequence, -1.0);
    let r = host.revision(); let actor = host.actor_snapshot().unwrap().actor_revision;
    observer.forecast_request(host, r, key, actor, &frame).unwrap().unwrap();
    host.action_consistency_deadline().unwrap().unwrap()
}

#[test]
fn early_and_foreign_timers_preserve_two_key_publication_and_cannot_consume_the_forecast() {
    let root = Directory::new(); let other = Directory::new();
    let (mut host, human, observer) = owner(&root); let (_, _, foreign) = owner(&other);
    prediction::forecast(&mut host, &observer, 1, 1, -1.0);
    let deadline = host.action_consistency_deadline().unwrap().unwrap();
    let before = host.inspect(); let r = host.revision();
    assert_eq!(foreign.expire_forecast(&mut host, r, deadline, deadline.expires_at), Err(Error::Binding.into()));
    assert_eq!(observer.expire_forecast(&mut host, r - 1, deadline, deadline.expires_at), Err(Error::Stale.into()));
    assert_eq!(observer.expire_forecast(&mut host, r, deadline, ElapsedTick(deadline.expires_at.0 - 1)), Ok(Ok(false)));
    assert_eq!(host.inspect(), before); assert!(host.storage_failure().is_none());
    let keys = ordinary::ready(&mut host, &human, 1, b"ordinary");
    let r = host.revision();
    assert_eq!(observer.expire_forecast(&mut host, r, deadline, deadline.expires_at), Err(Error::Missing.into()));
    ordinary::dispatch(&mut host, &keys);
    let publication = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
    assert_eq!(publication.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(publication.outcome));
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
    assert!(host.inspect().stop.is_none());
}

#[test]
fn recorded_expiry_survives_reopen_without_rearming_or_inventing_a_request() {
    let root = Directory::new(); let (mut host, _, observer) = owner(&root);
    let deadline = forecast_request(&mut host, &observer, 9000, 1);
    let before = host.action_consistency_snapshot().unwrap(); let r = host.revision();
    assert_eq!(observer.expire_forecast(&mut host, r, deadline, deadline.expires_at), Ok(Ok(true)));
    assert!(matches!(host.request_status(9000), Err(JournalError::Contract(Error::Missing))));
    assert_eq!(host.retained_requests(), 0); assert_eq!(host.inspect().executions, 0);
    let stopped = host.inspect(); let r = host.revision();
    assert_eq!(observer.expire_forecast(&mut host, r, deadline, deadline.expires_at), Ok(Ok(true)));
    assert_eq!(host.inspect(), stopped);
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), stopped);
    drop(host);
    let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
    let after = host.action_consistency_snapshot().unwrap();
    assert_eq!(after.evidence, before.evidence); assert_eq!(after.pending_attempt, before.pending_attempt);
    assert!(after.coverage_lost); assert!(host.inspect().stop.is_some()); assert!(!host.clock_ready());
    assert_eq!(host.action_consistency_deadline().unwrap(), Some(deadline));
    let r = host.revision();
    assert_eq!(observer.expire_forecast(&mut host, r, deadline, deadline.expires_at), Err(Error::Binding.into()));
    assert_eq!(host.retained_requests(), 0);
}

// Re-executed as a real analytic client, not model inference or isolation proof.
#[test]
fn actor_child_fixture() {
    let Some(mode) = std::env::var_os("FA_ACTOR_CHILD") else { return; };
    let mut client = ActorClientState::new(ClientSessionLimits::default()).unwrap().connect_process_stdin().unwrap();
    client.submit(9000, &process_support::proposal()).unwrap();
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        assert!(Instant::now() < until, "actor deadline fixture did not finish");
        if let ClientProgress::Response(response) = client.step().unwrap() {
            if mode == "silent" {
                assert!(matches!(response.result, Ok(Knowledge::Pending { .. })));
                // Deliberately never produce the next forecast's action.
                std::thread::sleep(Duration::from_secs(20));
                panic!("silent actor was not terminated");
            }
            assert_eq!(mode, "answer");
            match response.result {
                Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }) => return,
                Ok(Knowledge::Pending { .. } | Knowledge::Unknown { .. }) => client.poll(9000).unwrap(),
                other => panic!("unexpected original response: {other:?}"),
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn silent_child_is_terminated_without_intake_but_effects_wait_for_original_reconciliation() {
    for (silent, published) in [(false, true), (true, false), (true, true)] {
        let root = Directory::new(); let (mut host, human, observer) = owner(&root);
        let first = forecast_request(&mut host, &observer, 9000, 1); let r = host.revision();
        assert_eq!(observer.expire_forecast(&mut host, r, first, ElapsedTick(10)), Ok(Ok(false)));
        let (port, mut driver) = host.into_supervised_driver();
        assert_eq!(driver.next_consistency_deadline().unwrap(), Some(first));
        let r = driver.supervisor().host().unwrap().revision();
        driver.supervisor_mut().set_snapshot(r, Some(snapshot())).unwrap();
        let mut process = ActorProcess::launch(&process_support::program(if silent { "silent" } else { "answer" }),
            ActorWire::new(port.clone()), ChannelLimits::default()).unwrap();
        process_support::submitted(&mut process, driver.supervisor());
        let ticket = port.submit(9000, &process_support::proposal()).unwrap();
        assert_eq!(driver.next_consistency_deadline().unwrap(), None);
        {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            let keys = process_support::approve(&mut host, &human); process_support::ordinary::dispatch(&mut host, &keys);
            if published {
                let r = host.revision();
                assert_eq!(host.publish_checked(r, 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap().outcome,
                    EndpointOutcome::Executed { resulting_version: 2 });
            }
        }
        assert_eq!(port.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown });
        if silent {
            let deadline = {
                let mut host = driver.supervisor_mut().host_mut().unwrap();
                forecast_request(&mut host, &observer, 9001, 2)
            };
            assert_eq!(driver.next_consistency_deadline().unwrap(), Some(deadline));
            let samples = driver.supervisor().host().unwrap().action_consistency_snapshot().unwrap().evidence.samples();
            assert_eq!(driver.expire_consistency_forecast(&observer, deadline, deadline.expires_at), Ok(Ok(true)));
            assert_eq!(driver.next_consistency_deadline().unwrap(), None);
            let mut absent_source = FileEvidenceSource::new(root.0.join("never-read.json"), 17,
                profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
            let report = driver.drive_actor_process_from_file(&mut process, &mut absent_source,
                || panic!("deadline stop tried to read a source clock"), DriveBudget::default()).unwrap();
            assert_eq!(report.withdrawal, Some(ActorProcessWithdrawal::TerminalStop));
            assert!(report.transport.unwrap().is_none()); assert!(report.intakes.is_empty());
            assert_eq!(absent_source.status().read_attempts, 0);
            process_support::reap(&mut process);
            assert!(process.status().ingress_closed);
            assert_eq!(port.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown });
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            assert_eq!(host.inspect().control.ledger.charged, 16);
            assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), samples);
            assert!(matches!(host.request_status(9001), Err(JournalError::Contract(Error::Missing))));
            let r = host.revision(); assert!(host.progress_stop(r, ElapsedTick(12)).unwrap().progress.drained());
            assert_eq!(host.inspect().control.ledger.charged, if published { 16 } else { 0 });
            assert_eq!(host.inspect().executions, u64::from(published));
        } else {
            assert_eq!(driver.expire_consistency_forecast(&observer, first, first.expires_at), Err(Error::Missing.into()));
            {
                let mut host = driver.supervisor_mut().host_mut().unwrap(); let r = host.revision();
                assert_eq!(host.reconcile(r, 1).unwrap(), Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }));
                assert!(host.inspect().stop.is_none());
            }
            let until = Instant::now() + Duration::from_secs(20);
            while process.poll().child.exit.is_none() {
                if !process.status().ingress_closed {
                    match process.drive(DriveBudget::default()) {
                        Ok(_) => {},
                        Err(WireError::Unavailable) => assert!(process.status().child.exit.is_some()),
                        Err(error) => panic!("unexpected process failure: {error:?}"),
                    }
                }
                assert!(Instant::now() < until); std::thread::sleep(Duration::from_millis(1));
            }
            assert!(process.status().child.exit.unwrap().success);
            assert!(!process.status().child.termination_sent);
        }
    }
}
