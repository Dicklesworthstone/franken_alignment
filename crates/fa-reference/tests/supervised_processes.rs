//! Configured Rust worker processes through the original supervised control path.
#![cfg(unix)]
#![forbid(unsafe_code)]

#[path = "support/supervised_driver.rs"]
mod support;

use support::{Rig, proposal, snapshot, target};
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::delivery::{PublicationEndpoint, EndpointStatus};
use fa_reference::action::consequence::delivery::filesystem::FilePublicationLimits;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use fa_reference::action::consequence::oversight::helper_client::{ClientProgress, HelperClient};
use fa_reference::action::consequence::oversight::helper_processes::{HelperProgram};
use fa_reference::action::consequence::oversight::supervised::{
    DriverError, DriverEvent, DriverPhase, ProcessReviewError, ProcessReviewLaunch, ReviewLaunch, SupervisedDriver,
};
use fa_reference::action::consequence::oversight::DispatchKeys;
use fa_reference::action::consequence::oversight::helper_workers::io::WorkerIoError;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn commands(modes: [&str; 2]) -> BTreeMap<String, HelperProgram> {
    ["alice", "bob"].into_iter().zip(modes).map(|(name, mode)| {
        (name.to_owned(), HelperProgram::new(std::env::current_exe().unwrap(),
            std::env::temp_dir().canonicalize().unwrap(),
            vec!["--exact".into(), "managed_worker_entry".into(), "--nocapture".into()],
            BTreeMap::from([
                ("FA_MANAGED_WORKER".into(), name.into()), ("FA_MANAGED_MODE".into(), mode.into()),
            ])).unwrap())
    }).collect()
}
fn launch(rig: &mut Rig, request: u64, round: u64, modes: [&str; 2]) -> ProcessReviewLaunch {
    let ReviewLaunch { request, round, evidence_root, window, expected_input_revision, inputs, streams, limits } = rig.launch(request, round);
    // Reuse only the fixture's independently constructed input, not its clients.
    drop(streams);
    rig.clients.clear();
    ProcessReviewLaunch { request, round, evidence_root, window, expected_input_revision,
        inputs, programs: commands(modes), limits }
}
fn reap(driver: &mut SupervisedDriver) {
    let end = Instant::now() + Duration::from_secs(10);
    while !driver.helpers_reaped() {
        driver.reap_helpers();
        assert!(Instant::now() < end, "retained children: {:?}", driver.helper_processes());
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn finish(rig: &mut Rig) -> DriverEvent {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        let result = rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap();
        if !matches!(result, DriverEvent::Workers { .. }) { return result; }
        assert!(Instant::now() < end, "managed review did not complete");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn managed_worker_entry() {
    let Some(member) = std::env::var_os("FA_MANAGED_WORKER") else { return; };
    let name = member.to_str().unwrap();
    assert!(matches!(name, "alice" | "bob"));
    let mode = std::env::var("FA_MANAGED_MODE").unwrap();
    if mode == "silent" { std::thread::sleep(Duration::from_secs(30)); return; }
    let mut client = HelperClient::from_process_stdin(InputProfileBinding {
        profile_id: 1, profile_bytes: name.as_bytes().to_vec(), model_epoch: 1,
        tokenizer_epoch: 1, policy_epoch: 0,
    }).unwrap();
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(Instant::now() < end);
        match client.step() {
            Ok(ClientProgress::NeedsInference) => {
                let bytes = client.input().unwrap().actual_input().submitted_bytes();
                assert!(bytes.ends_with(b"Check the frozen effect"));
                let vote = if mode == "hold" { Verdict::Hold }
                    else if bytes.windows(7).any(|part| part == b"publish") { Verdict::Allow }
                    else { Verdict::Deny };
                client.respond(vote, name.as_bytes()).unwrap();
            }
            Ok(ClientProgress::ReplySent) => return,
            Ok(ClientProgress::Blocked) => std::thread::sleep(Duration::from_millis(1)),
            Ok(ClientProgress::Progress) => {}
            // Setup/cancellation tests close the private stream intentionally.
            Err(_) if mode == "cancel" => return,
            Err(error) => panic!("worker protocol failure: {error:?}"),
        }
    }
}

#[test]
fn executable_review_matches_connected_review_then_publishes_once() {
    let (mut legacy, _) = Rig::new(false);
    legacy.accept(1);
    legacy.start(1, 11);
    let legacy_review = legacy.finish_review([Verdict::Allow; 2]);
    let (mut rig, _) = Rig::new(false);
    let ticket = rig.accept(1);
    let configuration = launch(&mut rig, 1, 11, ["allow"; 2]);
    rig.driver.start_process_review(configuration, &snapshot()).unwrap();
    let review = finish(&mut rig);
    match (legacy_review, review) {
        (DriverEvent::ReviewApplied { receipt: left, .. }, DriverEvent::ReviewApplied { receipt: right, .. }) => {
            assert_eq!(left.policy.control.decision, right.policy.control.decision);
            assert_eq!(right.policy.control.decision.consequence, Consequence::Continue);
        }
        pair => panic!("unexpected review pair: {pair:?}"),
    }
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Pending { .. }));
    reap(&mut rig.driver);
    let pids = rig.driver.helper_processes();
    assert!(matches!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap(),
        DriverEvent::PublicationResolved { .. }));
    assert_eq!(rig.driver.endpoint().execution_count(), 1);
    assert_eq!(rig.driver.endpoint().payload(), b"publish");
    assert_eq!(rig.driver.helper_processes(), pids);
    let retry = rig.port.submit(1, &proposal()).unwrap();
    assert_eq!(rig.port.poll(&retry), rig.port.poll(&ticket));
    assert!(matches!(rig.driver.step(ElapsedTick(1), None, &snapshot(), None).unwrap(), DriverEvent::Idle));
}

#[test]
fn cancellation_stops_and_reaps_the_same_children_without_publication() {
    let (mut rig, _) = Rig::new(false);
    let ticket = rig.accept(1);
    let configuration = launch(&mut rig, 1, 11, ["silent"; 2]);
    rig.driver.start_process_review(configuration, &snapshot()).unwrap();
    let pids = rig.driver.helper_processes();
    let other = launch(&mut rig, 1, 12, ["allow"; 2]);
    assert_eq!(rig.driver.start_process_review(other, &snapshot()),
        Err(ProcessReviewError::Driver(DriverError::Control(Error::WrongState))));
    assert_eq!(rig.driver.helper_processes(), pids);
    rig.port.cancel(&ticket).unwrap();
    assert!(matches!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap(),
        DriverEvent::Stopped { state: ActionState::Cancelled, .. }));
    reap(&mut rig.driver);
    for (member, status) in rig.driver.helper_processes() {
        assert_eq!(status.pid, pids[&member].pid);
        assert!(status.stop_requested && status.exit.is_some());
    }
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.available, 100);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
}

#[test]
fn silent_child_expires_while_the_healthy_peer_completes_and_no_vote_is_replaced() {
    let (mut rig, _) = Rig::new(false);
    rig.accept(1);
    let configuration = launch(&mut rig, 1, 11, ["silent", "allow"]);
    rig.driver.start_process_review(configuration, &snapshot()).unwrap();
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        let event = rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap();
        if let DriverEvent::Workers { report, .. } = event {
            if report.workers["bob"].committed { break; }
        } else { panic!("premature review completion"); }
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(1));
    }
    loop {
        let event = rig.driver.step(ElapsedTick(5), rig.inputs.as_ref(), &snapshot(), None).unwrap();
        if let DriverEvent::Workers { report, .. } = event {
            assert!(!report.workers["alice"].revealed);
            if report.workers["bob"].revealed { break; }
        } else { panic!("missing member was removed"); }
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(rig.driver.helper_processes()["alice"].stop_requested);
    match rig.driver.step(ElapsedTick(10), rig.inputs.as_ref(), &snapshot(), None).unwrap() {
        DriverEvent::ReviewApplied { receipt, .. } => assert_ne!(receipt.policy.control.decision.consequence, Consequence::Continue),
        event => panic!("expected original restrictive review: {event:?}"),
    }
    reap(&mut rig.driver);
    assert_eq!(rig.driver.phase(), DriverPhase::Idle);
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
}

#[test]
fn stale_capture_and_roster_refusals_do_not_launch_children() {
    let (mut rig, _) = Rig::new(false);
    rig.accept(1);
    let mut stale = launch(&mut rig, 1, 11, ["allow"; 2]);
    stale.expected_input_revision = 99;
    assert_eq!(rig.driver.start_process_review(stale, &snapshot()),
        Err(ProcessReviewError::Driver(DriverError::Control(Error::Stale))));
    assert!(rig.driver.helper_processes().is_empty());
    let mut missing = launch(&mut rig, 1, 11, ["allow"; 2]);
    missing.programs.remove("bob");
    assert_eq!(rig.driver.start_process_review(missing, &snapshot()),
        Err(ProcessReviewError::Driver(DriverError::Control(Error::Binding))));
    assert!(rig.driver.helper_processes().is_empty());
    let good = launch(&mut rig, 1, 11, ["hold"; 2]);
    rig.driver.start_process_review(good, &snapshot()).unwrap();
    assert!(matches!(finish(&mut rig), DriverEvent::ReviewApplied { .. }));
    reap(&mut rig.driver);
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
}

#[test]
fn completed_helpers_do_not_remove_the_independent_human_key_requirement() {
    let (mut rig, reviewer) = Rig::new(true);
    rig.accept(1);
    let configured = launch(&mut rig, 1, 11, ["allow"; 2]);
    rig.driver.start_process_review(configured, &snapshot()).unwrap();
    assert!(matches!(finish(&mut rig), DriverEvent::ReviewApplied { .. }));
    reap(&mut rig.driver);
    assert!(matches!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap(),
        DriverEvent::AwaitingHuman { .. }));
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.reserved, 0);
    let request = rig.driver.request_human_approval(90, rig.inputs.as_ref(), ElapsedTick(10)).unwrap();
    let human = reviewer.unwrap().approve(&request, ElapsedTick(1)).unwrap();
    assert!(matches!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), Some(&human)).unwrap(),
        DriverEvent::PublicationResolved { .. }));
    assert_eq!(rig.driver.endpoint().execution_count(), 1);
}

#[test]
fn offline_controller_retains_child_ownership_and_cancels_without_relaunch() {
    let (mut rig, _) = Rig::new(false);
    let ticket = rig.accept(1);
    let configured = launch(&mut rig, 1, 11, ["silent"; 2]);
    rig.driver.start_process_review(configured, &snapshot()).unwrap();
    let pids = rig.driver.helper_processes();
    let (mut offline, endpoint) = rig.driver.detach_endpoint();
    assert_eq!(offline.helper_processes(), pids);
    rig.port.cancel(&ticket).unwrap();
    offline.cancel_active().unwrap();
    let end = Instant::now() + Duration::from_secs(10);
    while !offline.helpers_reaped() {
        offline.reap_helpers();
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(1));
    }
    let statuses = offline.helper_processes();
    let driver = offline.reconnect(endpoint, ElapsedTick(1)).unwrap();
    assert_eq!(driver.helper_processes(), statuses);
    assert_eq!(driver.endpoint().execution_count(), 0);
    assert_eq!(driver.supervisor().broker().inspect().ledger.available, 100);
}

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let tick = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-managed-{}-{tick}", std::process::id()));
        fs::create_dir(&path).unwrap(); Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("fixture cleanup: {error}"); }
    }
}

#[test]
fn launched_helpers_can_retire_before_lost_ack_file_recovery_without_second_publication() {
    let temp = Temp::new();
    let (endpoint, key) = PublicationEndpoint::create_file_publication(temp.0.join("publication"),
        target(), b"old".to_vec(), 200, 16, FilePublicationLimits { mutations: 128, bytes: 1_048_576 }).unwrap();
    let (mut rig, _) = Rig::with_endpoint(endpoint, false);
    let ticket = rig.accept(1);
    let configured = launch(&mut rig, 1, 11, ["allow"; 2]);
    rig.driver.start_process_review(configured, &snapshot()).unwrap();
    assert!(matches!(finish(&mut rig), DriverEvent::ReviewApplied { .. }));
    reap(&mut rig.driver);
    let pids = rig.driver.helper_processes();
    // Trusted fault seam: interrupt after publication but before local receipt.
    let permit = rig.driver.supervisor_mut().authorize_request(1, rig.inputs.as_ref(), &snapshot()).unwrap();
    let envelope = rig.driver.supervisor_mut().dispatch_request(1, DispatchKeys::single(&permit), rig.inputs.as_ref(), &snapshot()).unwrap();
    rig.driver.endpoint_mut().deliver(&envelope).unwrap();
    rig.driver.supervisor_mut().acknowledgment_lost(1).unwrap();
    let attempt = rig.driver.supervisor().attempt(1).unwrap();
    let revision = rig.driver.supervisor().broker().input_revision(attempt).unwrap();
    rig.driver.supervisor_mut().broker_mut().inputs_unavailable(attempt, revision).unwrap();
    let (offline, endpoint) = rig.driver.detach_endpoint();
    drop(endpoint);
    let mut restored = offline.reconnect(key.reopen().unwrap(), ElapsedTick(2)).unwrap();
    assert_eq!(restored.helper_processes(), pids);
    assert!(matches!(restored.reconcile_pending(ElapsedTick(2)).unwrap()[&attempt], Ok(EndpointStatus::Resolved(_))));
    assert_eq!(restored.endpoint().execution_count(), 1);
    assert_eq!(PublicationEndpoint::read_file_publication(key.directory()).unwrap().payload, b"publish");
    let retry = rig.port.submit(1, &proposal()).unwrap();
    assert_eq!(rig.port.poll(&retry), rig.port.poll(&ticket));
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert!(matches!(restored.step(ElapsedTick(2), None, &snapshot(), None).unwrap(),
        DriverEvent::Stopped { state: ActionState::Confirmed, .. }));
    assert_eq!(restored.endpoint().execution_count(), 1);
    assert_eq!(restored.helper_processes(), pids);
}

#[test]
fn post_spawn_session_refusal_retains_cleanup_and_does_not_recycle_the_round() {
    let (mut rig, _) = Rig::new(false);
    rig.accept(1);
    let mut configuration = launch(&mut rig, 1, 11, ["cancel"; 2]);
    let window = configuration.window;
    configuration.limits.members = 1;
    assert_eq!(rig.driver.start_process_review(configuration, &snapshot()),
        Err(ProcessReviewError::Driver(DriverError::Worker(WorkerIoError::Protocol(Error::Limit)))));
    assert_eq!(rig.driver.phase(), DriverPhase::Idle);
    assert_eq!(rig.driver.helper_processes().len(), 2);
    assert!(rig.driver.helper_processes().values().all(|status| status.stop_requested));
    reap(&mut rig.driver);
    let attempt = rig.driver.supervisor().attempt(1).unwrap();
    assert_eq!(rig.driver.supervisor_mut().broker_mut()
        .begin_review(attempt, 11, [1; 32], window, &snapshot()).unwrap_err(), Error::Duplicate);
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.available, 100);
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
    assert!(matches!(rig.driver.step(ElapsedTick(1), None, &snapshot(), None).unwrap(), DriverEvent::Idle));
}
