//! A native consistency incident stops the real direct child, not its effect ledger.
#![cfg(unix)]
#[path = "support/actor_process.rs"] mod launch;
#[path = "support/file_consistency.rs"] mod prediction;
use launch::{Directory, profile, snapshot};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::driver::FileSupervisedDriver;
use fa_reference::action::consequence::delivery::persistent::observed::stream::actor_wire::process::ActorProcessWithdrawal;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::actor_process::ActorProcess;
use fa_reference::action::consequence::oversight::actor_transport::DriveBudget;
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, ChannelLimits};
use fa_reference::action::consequence::oversight::actor_wire::client::{ActorClientState, ClientSessionLimits, ClientProgress};
use fa_reference::action::consequence::oversight::consistency::{ConsistencyStopPolicy, ConsistencyStopCause};
use fa_reference::action::consequence::oversight::evidence_source::{FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use std::time::{Duration, Instant};

#[test]
fn actor_child_fixture() {
    if std::env::var_os("FA_ACTOR_CHILD").is_none() { return; }
    let mut client = ActorClientState::new(ClientSessionLimits::default()).unwrap().connect_process_stdin().unwrap();
    client.submit(9000, &launch::proposal()).unwrap();
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        if let ClientProgress::Response(response) = client.step().unwrap() {
            assert!(matches!(response.result, Ok(Knowledge::Pending { .. })));
            std::thread::sleep(Duration::from_secs(20)); panic!("supervisor did not terminate the actor");
        }
        assert!(Instant::now() < until); std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn acknowledged_consistency_loss_closes_ingress_without_source_reads_and_keeps_original_liability() {
    for published in [false, true] {
        let root = Directory::new(); let p = profile();
        let (mut h, human) = FileOversight::create(root.store(), p.clone()).unwrap();
        let cfg = prediction::configuration().with_terminal_stop(ConsistencyStopPolicy::new(11, 12, 7000).unwrap()).unwrap();
        let observer = h.enable_action_consistency(0, cfg).unwrap(); h.observe_time(h.revision(), ElapsedTick(1)).unwrap();
        let frame = prediction::frame(&h, 1, -1.0); let r = h.revision(); let actor_revision = h.actor_snapshot().unwrap().actor_revision;
        observer.forecast_request(&mut h, r, 9000, actor_revision, &frame).unwrap().unwrap();
        let (port, mut supervisor) = h.into_actor_gateway(); let r = supervisor.host().unwrap().revision();
        supervisor.set_snapshot(r, Some(snapshot())).unwrap();
        let mut process = ActorProcess::launch(&launch::program("consistency"), ActorWire::new(port.clone()), ChannelLimits::default()).unwrap();
        launch::submitted(&mut process, &supervisor);
        let ticket = port.submit(9000, &launch::proposal()).unwrap();
        {
            let mut h = supervisor.host_mut().unwrap(); let keys = launch::approve(&mut h, &human);
            launch::ordinary::dispatch(&mut h, &keys);
            if published {
                let r = h.revision(); assert_eq!(h.publish_checked(r, 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap().outcome,
                    EndpointOutcome::Executed { resulting_version: 2 });
            }
            let r = h.revision(); observer.unavailable(&mut h, r).unwrap();
            assert_eq!(h.consistency_stop_incident().unwrap().unwrap().cause, ConsistencyStopCause::CoverageLost);
        }
        let before = supervisor.host().unwrap().inspect(); assert_eq!(before.control.ledger.charged, 16);
        let mut driver = FileSupervisedDriver::new(supervisor);
        let mut source = FileEvidenceSource::new(root.0.join("missing.json"), 17, p.delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
        let report = driver.drive_actor_process_from_file(&mut process, &mut source,
            || panic!("terminal stop attempted fresh evidence"), DriveBudget::default()).unwrap();
        assert_eq!(report.withdrawal, Some(ActorProcessWithdrawal::TerminalStop));
        assert!(report.transport.unwrap().is_none()); assert!(report.intakes.is_empty());
        assert!(report.process.ingress_closed && report.process.child.stop_requested); launch::reap(&mut process);
        assert_eq!(driver.supervisor().host().unwrap().inspect(), before); assert_eq!(source.status().read_attempts, 0);
        assert_eq!(port.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown });
        {
            let mut h = driver.supervisor_mut().host_mut().unwrap(); let r = h.revision();
            assert!(h.progress_stop(r, ElapsedTick(2)).unwrap().progress.drained());
            assert_eq!(h.inspect().control.ledger.charged, if published { 16 } else { 0 });
        }
        assert!(matches!(port.poll(&ticket), Knowledge::Known { value, .. }
            if value == if published { ActorOutcome::Executed } else { ActorOutcome::ConfirmedNotExecuted }));
    }
}
