//! Actual inherited-socket workers under ordinary journal pressure.
#![cfg(unix)]
#[path = "support/file_driver.rs"] mod fixture;
use fixture::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::StopRequest;
use fa_reference::action::consequence::delivery::persistent::{JournalError, RecoveryReserve};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::driver::{CapacityDrain, FileDriverEvent, FileDriverLaunch};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use fa_reference::action::consequence::oversight::helper_client::{ClientPhase, HelperClient};
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const CHILD: &str = "capacity_worker_entry";

/// Subprocess entry point, not an independent scenario.
#[test]
fn capacity_worker_entry() {
    let Ok(member) = std::env::var("FA_CAPACITY_WORKER") else { return; };
    assert!(MEMBERS.contains(&member.as_str()));
    let root = PathBuf::from(std::env::var_os("FA_CAPACITY_ROOT").unwrap());
    let mut client = HelperClient::from_process_stdin(profile().committee.members()[&member].profile_at(0)).unwrap();
    let mut announced = false;
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        assert!(Instant::now() < deadline, "fixture worker exceeded its bound");
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference {
            let input = client.input().unwrap();
            assert_eq!(input.member(), member);
            let expected = b"complete evidence used by the real socket workers";
            assert!(input.actual_input().submitted_bytes().windows(expected.len()).any(|part| part == expected));
            if !announced {
                let pending = root.join(format!("{member}.pending"));
                std::fs::write(&pending, std::process::id().to_string()).unwrap();
                std::fs::rename(pending, root.join(format!("{member}.ready"))).unwrap();
                announced = true;
            }
            if root.join("release").exists() {
                client.respond(Verdict::Allow, &helper::oversight::salt(&member)).unwrap();
            }
        }
        if client.phase() == ClientPhase::ReplySent { return; }
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn reserved_capacity_stops_the_same_live_workers_before_a_bad_clock_with_a_positive_control() {
    for exhaust in [false, true] {
        let root = Directory::new();
        let mut p = profile(); p.delivery.limits.events = 48;
        let (mut host, reviewer) = FileOversight::create(root.store(), p).unwrap();
        host.enable_recovery_reserve(0, RecoveryReserve::terminal()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let (port, driver) = host.into_supervised_driver();
        let mut rig = Rig { root, port, driver, reviewer, clients: Clients::new(), inputs: None };
        let ticket = rig.submit(1);
        let launch = rig.launch(1, 101);
        let FileDriverLaunch { request, round, evidence_root, window, expected_input_revision, inputs, workers, limits } = launch;
        drop(workers); rig.clients.clear();
        let workers = MEMBERS.into_iter().map(|member| {
            (member.to_owned(), HelperProgram::new(std::env::current_exe().unwrap(), rig.root.0.clone(),
                vec!["--exact".into(), CHILD.into(), "--nocapture".into()],
                BTreeMap::from([("FA_CAPACITY_WORKER".into(), member.into()),
                    ("FA_CAPACITY_ROOT".into(), rig.root.0.as_os_str().to_owned())])).unwrap())
        }).collect();
        rig.driver.start_process_review(FileDriverLaunch { request, round, evidence_root, window,
            expected_input_revision, inputs, workers, limits }, snapshot(), || ElapsedTick(1)).unwrap();
        let pids = rig.driver.helper_processes();
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let input = rig.inputs.clone();
            let event = rig.driver.step_with_evidence(|| ElapsedTick(1), |_, _| Ok(DriverEvidence {
                snapshot: snapshot(), inputs: input.clone(),
            }), None).unwrap();
            assert!(matches!(event, FileDriverEvent::Workers { .. }));
            let ready = pids.iter().all(|(member, status)| {
                match std::fs::read_to_string(rig.root.0.join(format!("{member}.ready"))) {
                    Ok(pid) => { assert_eq!(pid.parse::<u32>().unwrap(), status.pid); true }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
                    Err(error) => panic!("worker marker read: {error}"),
                }
            });
            if ready { break; }
            assert!(Instant::now() < deadline, "live worker readiness exceeded its bound");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(rig.driver.helper_processes().values().all(|status| status.exit.is_none() && !status.stop_requested));
        if exhaust {
            let request = {
                let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
                while host.journal_capacity().unwrap().ordinary_remaining().events > 0 {
                    let revision = host.revision(); host.observe_time(revision, ElapsedTick(1)).unwrap();
                }
                let control = host.inspect().control;
                StopRequest { operation: 1, expected_control_sequence: control.sequence, expected_authority_epoch: control.ledger.epoch }
            };
            let port = rig.port.clone();
            let result = rig.driver.stop_with_recovery_reserve(request, || {
                assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
                ElapsedTick(0)
            }).unwrap();
            assert_eq!(result.drain, Err(JournalError::Contract(Error::Stale)));
            assert!(rig.driver.helper_processes().values().all(|status| status.stop_requested));
            let retried = rig.driver.stop_with_recovery_reserve(request, || ElapsedTick(2)).unwrap();
            assert!(matches!(retried.drain, Ok(CapacityDrain::Advanced(ref swept)) if swept.progress.drained()));
            assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 0);
        } else {
            std::fs::write(rig.root.0.join("release"), b"explicit fixture release").unwrap();
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                let input = rig.inputs.clone();
                let event = rig.driver.step_with_evidence(|| ElapsedTick(1), |_, _| Ok(DriverEvidence {
                    snapshot: snapshot(), inputs: input.clone(),
                }), None).unwrap();
                if matches!(event, FileDriverEvent::ReviewApplied { .. }) { break; }
                assert!(matches!(event, FileDriverEvent::Workers { .. }));
                assert!(Instant::now() < deadline, "positive original review exceeded its bound");
                std::thread::sleep(Duration::from_millis(1));
            }
            let key = rig.human(1001, 31);
            assert!(matches!(rig.step(Some(&key)), FileDriverEvent::Dispatched { .. }));
            assert!(matches!(rig.step(None), FileDriverEvent::Published { .. }));
            assert!(matches!(rig.step(None), FileDriverEvent::Reconciled { .. }));
            assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 1);
        }
        let deadline = Instant::now() + Duration::from_secs(15);
        while !rig.driver.helpers_reaped() {
            rig.driver.reap_helpers();
            assert!(Instant::now() < deadline, "direct-child reaping exceeded its bound");
            std::thread::sleep(Duration::from_millis(1));
        }
        let final_children = rig.driver.helper_processes();
        assert_eq!(final_children.len(), pids.len());
        for (member, status) in final_children {
            assert_eq!(status.pid, pids[&member].pid);
            assert!(status.exit.is_some());
        }
    }
}
