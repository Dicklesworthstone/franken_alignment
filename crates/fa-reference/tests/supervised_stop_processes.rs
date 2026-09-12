//! Actual direct-child cleanup; fixture verdicts are not model evaluations.
#![cfg(unix)]
#![forbid(unsafe_code)]

#[path = "support/supervised_driver.rs"]
#[allow(dead_code)]
mod support;

use support::{Rig, proposal, snapshot};
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::delivery::StopRequest;
use fa_reference::action::consequence::oversight::actor::{ActorError, ActorOutcome, Knowledge};
use fa_reference::action::consequence::oversight::helper_client::{ClientProgress, HelperClient};
use fa_reference::action::consequence::oversight::helper_processes::{HelperProgram, ProcessStatus};
use fa_reference::action::consequence::oversight::supervised::{
    DriverError, DriverEvent, DriverPhase, ProcessReviewLaunch, ReviewLaunch, SupervisedDriver,
};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-stop-child-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
    fn ready(&self, member: &str) -> PathBuf { self.0.join(format!("{member}.ready")) }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("child stop fixture cleanup: {error}"); }
    }
}

fn launch(rig: &mut Rig, directory: &Directory, mode: &str) {
    let ReviewLaunch { request, round, evidence_root, window, expected_input_revision, inputs, streams, limits } = rig.launch(1, 11);
    drop(streams);
    rig.clients.clear();
    let programs = ["alice", "bob"].into_iter().map(|member| {
        (member.to_owned(), HelperProgram::new(std::env::current_exe().unwrap(), directory.0.clone(),
            vec!["--exact".into(), "stop_worker_entry".into(), "--nocapture".into()],
            BTreeMap::from([
                ("FA_STOP_CHILD_MEMBER".into(), member.into()),
                ("FA_STOP_CHILD_MODE".into(), mode.into()),
                ("FA_STOP_CHILD_READY".into(), directory.ready(member).into_os_string()),
            ])).unwrap())
    }).collect();
    rig.driver.start_process_review(ProcessReviewLaunch {
        request, round, evidence_root, window, expected_input_revision, inputs, programs, limits,
    }, &snapshot()).unwrap();
}

fn live_children(rig: &mut Rig, directory: &Directory) -> BTreeMap<String, ProcessStatus> {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        let states = rig.driver.reap_helpers();
        assert_eq!(states.len(), 2);
        assert!(states.values().all(|state| !state.stop_requested && state.exit.is_none()));
        let ready = states.iter().all(|(member, state)| match fs::read_to_string(directory.ready(member)) {
            Ok(pid) => { assert_eq!(pid.parse::<u32>().unwrap(), state.pid); true }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => panic!("child readiness read failed: {error}"),
        });
        if ready { return states; }
        assert!(Instant::now() < end, "children did not become ready: {states:?}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn reap(driver: &mut SupervisedDriver) {
    let end = Instant::now() + Duration::from_secs(10);
    while !driver.helpers_reaped() {
        driver.reap_helpers();
        assert!(Instant::now() < end, "unreaped children: {:?}", driver.helper_processes());
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn request(rig: &Rig) -> StopRequest {
    let state = rig.driver.supervisor().broker().inspect();
    StopRequest { operation: 1, expected_control_sequence: state.sequence, expected_authority_epoch: state.ledger.epoch }
}
fn assert_reaped(original: &BTreeMap<String, ProcessStatus>, current: &BTreeMap<String, ProcessStatus>) {
    assert!(original.keys().eq(current.keys()));
    for (member, state) in current {
        assert_eq!(state.pid, original[member].pid);
        assert!(state.stop_requested && state.exit.is_some(), "{member}: {state:?}");
    }
}

/// Test-subprocess entry point, not a fourth independent scenario. Silent
/// children prove execution reached this body before the parent's stop operation.
#[test]
fn stop_worker_entry() {
    let Some(member) = std::env::var_os("FA_STOP_CHILD_MEMBER") else { return; };
    let member = member.to_str().unwrap();
    assert!(matches!(member, "alice" | "bob"));
    let ready = PathBuf::from(std::env::var_os("FA_STOP_CHILD_READY").unwrap());
    let pending = ready.with_extension("pending");
    fs::write(&pending, std::process::id().to_string()).unwrap();
    fs::rename(pending, ready).unwrap();
    match std::env::var("FA_STOP_CHILD_MODE").unwrap().as_str() {
        "silent" => {
            std::thread::sleep(Duration::from_secs(30));
            panic!("parent failed to terminate its silent helper");
        }
        "allow" => {}
        other => panic!("unexpected fixture mode: {other}"),
    }
    let mut client = HelperClient::from_process_stdin(InputProfileBinding {
        profile_id: 1, profile_bytes: member.as_bytes().to_vec(), model_epoch: 1,
        tokenizer_epoch: 1, policy_epoch: 0,
    }).unwrap();
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(Instant::now() < end, "helper protocol deadline");
        match client.step().unwrap() {
            ClientProgress::NeedsInference => {
                assert!(client.input().unwrap().actual_input().submitted_bytes().ends_with(b"Check the frozen effect"));
                client.respond(Verdict::Allow, member.as_bytes()).unwrap();
            }
            ClientProgress::ReplySent => return,
            ClientProgress::Blocked => std::thread::sleep(Duration::from_millis(1)),
            ClientProgress::Progress => {}
        }
    }
}

#[test]
fn unstopped_process_review_can_publish_then_shutdown_preserves_the_execution_charge() {
    let directory = Directory::new();
    let (mut rig, _) = Rig::new(false);
    let ticket = rig.accept(1);
    launch(&mut rig, &directory, "allow");
    let pids = rig.driver.helper_processes();
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        match rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap() {
            DriverEvent::Workers { .. } => {}
            DriverEvent::ReviewApplied { receipt, .. } => {
                assert_eq!(receipt.policy.control.decision.consequence, Consequence::Continue);
                break;
            }
            event => panic!("unexpected process review: {event:?}"),
        }
        assert!(Instant::now() < end, "process review did not finish");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
    assert!(matches!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap(), DriverEvent::PublicationResolved { .. }));
    let stop = request(&rig);
    rig.driver.request_stop(stop).unwrap();
    let swept = rig.driver.progress_stop(ElapsedTick(1)).unwrap();
    assert!(swept.progress.drained());
    assert_eq!(swept.progress.charged_units, 16);
    reap(&mut rig.driver);
    assert_reaped(&pids, &rig.driver.helper_processes());
    assert!(rig.driver.stop_progress().unwrap().quiesced());
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert_eq!(rig.driver.endpoint().execution_count(), 1);
    assert_eq!(rig.driver.endpoint().payload(), b"publish");
}

#[test]
fn lower_level_stop_terminates_live_children_even_when_normal_step_clock_refuses() {
    for two_key in [false, true] {
        let directory = Directory::new();
        let (mut rig, reviewer) = Rig::new(two_key);
        let ticket = rig.accept(1);
        launch(&mut rig, &directory, "silent");
        let pids = live_children(&mut rig, &directory);
        let stop = request(&rig);
        rig.driver.supervisor_mut().broker_mut().request_stop(stop).unwrap();
        drop(reviewer);
        assert_eq!(rig.driver.step(ElapsedTick(0), None, &snapshot(), None).unwrap_err(), DriverError::Control(Error::Stale));
        assert_eq!(rig.driver.phase(), DriverPhase::Idle);
        assert!(rig.driver.helper_processes().values().all(|state| state.stop_requested));
        assert!(!rig.driver.stop_progress().unwrap().effects.endpoint_fenced);
        reap(&mut rig.driver);
        assert_reaped(&pids, &rig.driver.helper_processes());
        assert!(!rig.driver.stop_progress().unwrap().quiesced());
        assert!(rig.driver.progress_stop(ElapsedTick(1)).unwrap().progress.drained());
        assert!(rig.driver.stop_progress().unwrap().quiesced());
        assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
        assert_eq!(rig.port.submit(2, &proposal()).unwrap_err(), ActorError::Unavailable);
        assert_eq!(rig.driver.supervisor().broker().inspect().ledger.available, 100);
        assert_eq!(rig.driver.supervisor().broker().inspect().ledger.stages[&1], ActionState::Cancelled);
        assert_eq!(rig.driver.endpoint().execution_count(), 0);
    }
}

#[test]
fn failed_reconnect_still_stops_and_reaps_the_original_live_children() {
    let directory = Directory::new();
    let (mut rig, _) = Rig::new(false);
    let ticket = rig.accept(1);
    launch(&mut rig, &directory, "silent");
    let pids = live_children(&mut rig, &directory);
    let stop = request(&rig);
    rig.driver.supervisor_mut().broker_mut().request_stop(stop).unwrap();
    let (offline, endpoint) = rig.driver.detach_endpoint();
    let mut failure = match offline.reconnect(endpoint, ElapsedTick(0)) {
        Ok(_) => panic!("backwards clock unexpectedly accepted"),
        Err(failure) => failure,
    };
    assert_eq!(failure.error, Error::Stale);
    assert!(failure.offline.helper_processes().values().all(|state| state.stop_requested));
    let end = Instant::now() + Duration::from_secs(10);
    while !failure.offline.helpers_reaped() {
        failure.offline.reap_helpers();
        assert!(Instant::now() < end, "offline child cleanup deadline");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_reaped(&pids, &failure.offline.helper_processes());
    assert_eq!(failure.offline.stop_receipt().unwrap().request(), stop);
    assert!(!failure.offline.supervisor().stop_progress().unwrap().endpoint_fenced);
    let mut driver = failure.offline.reconnect(failure.endpoint, ElapsedTick(1)).unwrap();
    assert_eq!(driver.phase(), DriverPhase::Idle);
    assert!(driver.progress_stop(ElapsedTick(1)).unwrap().progress.drained());
    assert!(driver.stop_progress().unwrap().quiesced());
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    assert_eq!(rig.port.submit(2, &proposal()).unwrap_err(), ActorError::Unavailable);
    assert_eq!(driver.endpoint().execution_count(), 0);
    assert_eq!(driver.supervisor().broker().inspect().ledger.available, 100);
}
