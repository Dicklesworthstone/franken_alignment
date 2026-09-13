//! Real direct-child ownership on the integrated durable control path.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[path = "support/file_driver.rs"] mod fixture;
use fixture::*;
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverLaunch, FileDriverEvent, FileDriverPhase};
use fa_reference::action::consequence::delivery::persistent::observed::helpers::processes::{FileProcessFailure, HelperRoundAdmission};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use fa_reference::action::consequence::oversight::helper_client::{ClientPhase, HelperClient};
use fa_reference::action::consequence::oversight::helper_processes::{HelperProgram, ProcessStatus};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

const ENTRY: &str = "file_driver_child";
const MEMBER: &str = "FA_FILE_DRIVER_MEMBER";
const MODE: &str = "FA_FILE_DRIVER_MODE";
const READY: &str = "FA_FILE_DRIVER_READY";

/// Child entry point, not a sixth independent scenario. Readiness binds the
/// parent's observations to a process which actually reached this function.
#[test]
fn file_driver_child() {
    let Ok(member) = std::env::var(MEMBER) else { return; };
    let ready = std::path::PathBuf::from(std::env::var_os(READY).unwrap());
    let pending = ready.with_extension("pending");
    std::fs::write(&pending, std::process::id().to_string()).unwrap();
    std::fs::rename(pending, ready).unwrap();
    let mode = std::env::var(MODE).unwrap();
    if mode == "silent" {
        std::thread::sleep(Duration::from_secs(30));
        panic!("silent child was not terminated by its owner");
    }
    assert_eq!(mode, "allow");
    let contracts = profile().committee;
    let mut client = HelperClient::from_process_stdin(contracts.members()[&member].profile_at(0)).unwrap();
    let end = Instant::now() + Duration::from_secs(15);
    loop {
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference {
            let input = client.input().unwrap();
            assert_eq!(input.member(), member);
            assert_eq!(input.round(), 101);
            let expected = b"complete evidence used by the real socket workers";
            assert!(input.actual_input().submitted_bytes().windows(expected.len()).any(|bytes| bytes == expected));
            client.respond(Verdict::Allow, &helper::oversight::salt(&member)).unwrap();
        }
        if client.phase() == ClientPhase::ReplySent { return; }
        assert!(Instant::now() < end, "child protocol did not finish");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn ready_path(rig: &Rig, member: &str) -> std::path::PathBuf { rig.root.0.join(format!("{member}.ready")) }
fn launch(rig: &mut Rig, mode: &str) -> FileDriverLaunch<HelperProgram> {
    let FileDriverLaunch { request, round, evidence_root, window, expected_input_revision, inputs, workers, limits } = rig.launch(1, 101);
    drop(workers); rig.clients.clear();
    let workers = MEMBERS.into_iter().map(|member| {
        (member.to_owned(), HelperProgram::new(std::env::current_exe().unwrap(), rig.root.0.clone(),
            vec![OsString::from("--exact"), OsString::from(ENTRY), OsString::from("--nocapture")],
            BTreeMap::from([(OsString::from(MEMBER), OsString::from(member)),
                (OsString::from(MODE), OsString::from(mode)),
                (OsString::from(READY), ready_path(rig, member).into_os_string())])).unwrap())
    }).collect();
    FileDriverLaunch { request, round, evidence_root, window, expected_input_revision, inputs, workers, limits }
}
fn ready(rig: &mut Rig) -> BTreeMap<String, ProcessStatus> {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        let children = rig.driver.reap_helpers();
        assert_eq!(children.len(), 2);
        assert!(children.values().all(|status| !status.stop_requested && status.exit.is_none()));
        let all_ready = children.iter().all(|(member, status)| match std::fs::read_to_string(ready_path(rig, member)) {
            Ok(pid) => { assert_eq!(pid.parse::<u32>().unwrap(), status.pid); true }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => panic!("child readiness read: {error}"),
        });
        if all_ready { return children; }
        assert!(Instant::now() < end, "children did not publish their readiness");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn reap(rig: &mut Rig, pids: &BTreeMap<String, ProcessStatus>) {
    let end = Instant::now() + Duration::from_secs(10);
    while !rig.driver.helpers_reaped() {
        rig.driver.reap_helpers();
        assert!(Instant::now() < end, "retained child reaping deadline");
        std::thread::sleep(Duration::from_millis(1));
    }
    let states = rig.driver.helper_processes();
    assert!(states.keys().eq(pids.keys()));
    for (member, status) in states {
        assert_eq!(status.pid, pids[&member].pid);
        assert!(status.stop_requested && status.exit.is_some());
    }
}

#[test]
fn owned_processes_complete_review_before_independent_human_dispatch_and_publication() {
    let mut rig = Rig::new(); let ticket = rig.submit(1);
    let launch = launch(&mut rig, "allow");
    rig.driver.start_process_review(launch, snapshot(), || ElapsedTick(1)).unwrap();
    let pids = rig.driver.helper_processes(); assert_eq!(pids.len(), 2);
    assert_ne!(pids["alpha"].pid, pids["beta"].pid);
    let end = Instant::now() + Duration::from_secs(15);
    loop {
        match rig.step(None) {
            FileDriverEvent::Workers { .. } => {}
            FileDriverEvent::ReviewApplied { receipt, .. } => {
                assert_eq!(receipt.policy.control.decision.consequence, Consequence::Continue);
                assert_eq!(receipt.inputs.as_ref(), rig.inputs.as_ref().unwrap());
                break;
            }
            event => panic!("unexpected process progress: {event:?}"),
        }
        assert!(Instant::now() < end, "process review deadline");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(matches!(rig.step(None), FileDriverEvent::AwaitingHuman { .. }));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.reserved, 0);
    let key = rig.human(1001, 30);
    assert!(matches!(rig.step(Some(&key)), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(rig.step(None), FileDriverEvent::Published { outcome: EndpointOutcome::Executed { .. }, .. }));
    rig.inputs = None;
    assert!(matches!(rig.step(None), FileDriverEvent::Reconciled { .. }));
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    reap(&mut rig, &pids);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 1);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
}

#[test]
fn actor_cancellation_reaps_the_same_live_children_before_any_clock_or_source_callback() {
    let mut rig = Rig::new(); let ticket = rig.submit(1);
    let launch = launch(&mut rig, "silent");
    rig.driver.start_process_review(launch, snapshot(), || ElapsedTick(1)).unwrap();
    let pids = ready(&mut rig);
    rig.port.cancel(&ticket).unwrap();
    assert!(matches!(rig.driver.step_with_evidence(|| panic!("cancelled clock"), |_, _| panic!("cancelled source"), None).unwrap(),
        FileDriverEvent::Stopped { stage: ActionState::Cancelled, .. }));
    assert!(rig.driver.helper_processes().values().all(|status| status.stop_requested));
    reap(&mut rig, &pids);
    assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn storage_failure_still_preserves_and_reaps_live_children_without_a_review_or_effect() {
    let mut rig = Rig::new(); rig.submit(1);
    let launch = launch(&mut rig, "silent");
    rig.driver.start_process_review(launch, snapshot(), || ElapsedTick(1)).unwrap();
    let pids = ready(&mut rig);
    std::fs::write(rig.root.store().join("delivery.pending"), b"blocked clock replacement").unwrap();
    {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
        assert!(matches!(host.observe_time(revision, ElapsedTick(2)), Err(JournalError::Io(_))));
    }
    assert_eq!(rig.driver.step_with_evidence(|| panic!("faulted clock"), |_, _| panic!("faulted source"), None).unwrap_err(), JournalError::Unavailable);
    assert!(rig.driver.helper_processes().values().all(|status| status.stop_requested));
    reap(&mut rig, &pids);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 0);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.reserved, 0);
}

#[test]
fn partial_spawn_failure_keeps_the_original_child_owner_and_leased_round_in_the_driver() {
    let mut rig = Rig::new(); rig.submit(1);
    let mut launch = launch(&mut rig, "silent");
    let bad = rig.root.0.join("not-executable"); std::fs::write(&bad, b"not executable").unwrap();
    std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o600)).unwrap();
    launch.workers.insert("beta".to_owned(), HelperProgram::new(bad, rig.root.0.clone(), Vec::new(), BTreeMap::new()).unwrap());
    let error = rig.driver.start_process_review(launch, snapshot(), || ElapsedTick(1)).unwrap_err();
    assert_eq!(error.admission, HelperRoundAdmission::Committed);
    assert!(matches!(error.failure, FileProcessFailure::Launch { member: Some(member), .. } if member == "beta"));
    let pids = rig.driver.helper_processes(); assert_eq!(pids.len(), 1);
    assert!(pids["alpha"].stop_requested);
    assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
    reap(&mut rig, &pids);
    {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
        assert_eq!(host.commit_review(revision, 101, "alpha", 0).unwrap_err(), JournalError::Contract(Error::WrongState));
    }
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn releasing_a_driver_transfers_child_cleanup_and_the_same_authority_without_forging_cancellation() {
    let mut rig = Rig::new(); let ticket = rig.submit(1);
    let launch = launch(&mut rig, "silent");
    rig.driver.start_process_review(launch, snapshot(), || ElapsedTick(1)).unwrap();
    let pids = ready(&mut rig);
    let mut released = rig.driver.release();
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Pending { .. }));
    let children = released.children.as_mut().unwrap();
    let end = Instant::now() + Duration::from_secs(10);
    while !children.all_reaped() {
        children.reap();
        assert!(Instant::now() < end, "released cohort reaping deadline");
        std::thread::sleep(Duration::from_millis(1));
    }
    for (member, status) in children.statuses() {
        assert_eq!(status.pid, pids[&member].pid);
        assert!(status.stop_requested && status.exit.is_some());
    }
    let state = released.supervisor.host().unwrap().inspect();
    assert_eq!(state.control.ledger.stages[&1], ActionState::Reviewing);
    assert_eq!(state.executions, 0);
    assert_eq!(state.control.ledger.available, 100);
    drop(released);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Unknown { .. }));
}
