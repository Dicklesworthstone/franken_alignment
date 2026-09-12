//! Actual numerical reset through mailbox, socket workers and file recovery.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[path = "support/supervised_driver.rs"]
#[allow(dead_code)]
mod driving;
#[path = "support/hosted_decoder.rs"]
#[allow(dead_code)]
mod numerical;
use driving::{Rig, proposal, snapshot, target};
use fa_reference::action::consequence::activation::monitor::decoder::MonitoredStep;
use fa_reference::action::consequence::delivery::{PublicationEndpoint, FilePublicationLimits};
use fa_reference::action::consequence::gate::{ReviewBinding, TargetCeiling};
use fa_reference::action::consequence::oversight::{DispatchKeys, OversightBroker};
use fa_reference::action::consequence::oversight::actor::{ActorError, ActorOutcome, Knowledge};
use fa_reference::action::consequence::oversight::decoder_host::{HostedCheckpointHandle, HostedResetRequest};
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use fa_reference::action::consequence::oversight::supervised::{DriverError, DriverEvent, DriverPhase, ProcessReviewLaunch};
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

fn own(rig: &mut Rig) -> HostedCheckpointHandle {
    let broker = rig.driver.supervisor_mut().broker_mut();
    broker.own_sampled_decoder(numerical::quiet(), DecoderBindingLimits::default()).unwrap();
    let host = broker.hosted_decoder().unwrap();
    assert!(matches!(broker.advance_hosted_forced(host.actor_revision, 0, 0, numerical::compute()).unwrap(), MonitoredStep::Released(_)));
    rig.driver.capture_hosted_checkpoint(1, rig.driver.supervisor().broker().actor_revision()).unwrap()
}
fn request(broker: &OversightBroker, handle: &HostedCheckpointHandle, round: u64) -> HostedResetRequest {
    let state = broker.inspect();
    HostedResetRequest { checkpoint: handle.clone(), expected_control_sequence: state.sequence,
        expected_actor_revision: broker.actor_revision(), expected_authority_epoch: state.ledger.epoch,
        binding: ReviewBinding { round, reducer_generation: 1, evidence_root: [9; 32] },
        retained_targets: TargetCeiling::new(&[target()]).unwrap(), replay_budget: numerical::compute() }
}

#[test]
fn reset_cancels_the_abandoned_mailbox_prefix_then_new_socket_review_can_publish() {
    for two_key in [false, true] {
        let (mut rig, reviewer) = Rig::new(two_key); let checkpoint = own(&mut rig);
        let old = rig.accept(1); rig.start(1, 1);
        let queued = rig.port.submit(2, &proposal()).unwrap();
        let reset = rig.driver.reset_hosted_decoder(request(rig.driver.supervisor().broker(), &checkpoint, 100)).unwrap();
        assert_eq!(rig.driver.phase(), DriverPhase::Idle);
        assert_eq!(reset.control.cancelled, vec![1]);
        for ticket in [&old, &queued] {
            assert!(matches!(rig.port.poll(ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
        }
        assert_eq!(rig.port.submit(1, &proposal()).unwrap().request(), 1);
        assert_eq!(rig.port.submit(2, &proposal()).unwrap().request(), 2);
        assert!(rig.driver.accept_next(&snapshot()).unwrap().is_none());
        let mut next = proposal(); next.expected_policy_epoch = reset.control.revocation_floor;
        let fresh = rig.port.submit(3, &next).unwrap();
        assert!(rig.driver.accept_next(&snapshot()).unwrap().unwrap().result.is_ok());
        rig.start(3, 3); rig.finish_review([Verdict::Allow; 2]);
        let human = reviewer.as_ref().map(|reviewer| {
            let request = rig.driver.request_human_approval(500, rig.inputs.as_ref(), ElapsedTick(40)).unwrap();
            reviewer.approve(&request, ElapsedTick(1)).unwrap()
        });
        assert!(matches!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), human.as_ref()).unwrap(), DriverEvent::PublicationResolved { .. }));
        assert!(matches!(rig.port.poll(&fresh), Knowledge::Known { value: ActorOutcome::Executed, .. }));
        assert_eq!(rig.driver.endpoint().execution_count(), 1);
        assert_eq!(rig.driver.supervisor().broker().inspect().ledger.charged, 16);
    }
}

#[test]
fn reset_drops_the_drivers_original_reserved_permit_without_automatically_resending() {
    let (mut rig, _) = Rig::new(false); let checkpoint = own(&mut rig);
    let ticket = rig.accept(1); rig.start(1, 1); rig.finish_review([Verdict::Allow; 2]);
    rig.driver.supervisor_mut().broker_mut().restart_dispatcher().unwrap();
    assert_eq!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap_err(), DriverError::Control(Error::Incomplete));
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.reserved, 16);
    let reset = rig.driver.reset_hosted_decoder(request(rig.driver.supervisor().broker(), &checkpoint, 100)).unwrap();
    assert_eq!(reset.control.refunded_units, 16);
    assert_eq!(rig.driver.phase(), DriverPhase::Idle);
    rig.driver.confirm_dispatcher_fence().unwrap();
    assert!(matches!(rig.driver.step(ElapsedTick(1), None, &snapshot(), None).unwrap(), DriverEvent::Idle));
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.available, 100);
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
}

#[test]
fn refused_reset_preflight_preserves_a_valid_review_and_its_pending_actor_ticket() {
    let (mut rig, _) = Rig::new(false); let checkpoint = own(&mut rig);
    let ticket = rig.accept(1); rig.start(1, 1);
    let before = rig.driver.supervisor().broker().hosted_decoder().unwrap();
    let mut bad = request(rig.driver.supervisor().broker(), &checkpoint, 100);
    bad.expected_actor_revision += 1;
    assert_eq!(rig.driver.reset_hosted_decoder(bad).unwrap_err(), Error::Stale);
    assert_eq!(rig.driver.phase(), DriverPhase::Reviewing { request: 1 });
    assert_eq!(rig.driver.supervisor().broker().hosted_decoder().unwrap(), before);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Pending { .. }));
    rig.finish_review([Verdict::Allow; 2]);
    assert!(matches!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap(), DriverEvent::PublicationResolved { .. }));
    assert_eq!(rig.driver.endpoint().execution_count(), 1);
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-hosted-reset-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("hosted reset fixture cleanup failed: {error}"); }
    }
}

#[test]
fn offline_numerical_reset_preserves_file_execution_and_recovers_without_helpers_or_human_role() {
    let directory = Directory::new();
    let (endpoint, key) = PublicationEndpoint::create_file_publication(directory.0.join("publication"),
        target(), b"old".to_vec(), 200, 16, FilePublicationLimits { mutations: 128, bytes: 1_048_576 }).unwrap();
    let (mut rig, reviewer) = Rig::with_endpoint(endpoint, true); let checkpoint = own(&mut rig);
    let ticket = rig.accept(1); rig.start(1, 1); rig.finish_review([Verdict::Allow; 2]);
    let automatic = rig.driver.supervisor_mut().authorize_request(1, rig.inputs.as_ref(), &snapshot()).unwrap();
    let human_request = rig.driver.request_human_approval(500, rig.inputs.as_ref(), ElapsedTick(40)).unwrap();
    let human = reviewer.as_ref().unwrap().approve(&human_request, ElapsedTick(1)).unwrap();
    let envelope = rig.driver.supervisor_mut().dispatch_request(1, DispatchKeys::two(&automatic, &human),
        rig.inputs.as_ref(), &snapshot()).unwrap();
    let _lost = rig.driver.endpoint_mut().deliver(&envelope).unwrap();
    rig.driver.supervisor_mut().acknowledgment_lost(1).unwrap();
    rig.driver.supervisor_mut().broker_mut().inputs_unavailable(1, 1).unwrap();
    drop(reviewer); rig.clients.clear(); rig.inputs = None;
    let reset_request = request(rig.driver.supervisor().broker(), &checkpoint, 100);
    let (mut offline, endpoint) = rig.driver.detach_endpoint();
    let reset = offline.reset_hosted_decoder(reset_request).unwrap();
    assert!(reset.control.restored); assert_eq!(reset.control.refunded_units, 0);
    assert_eq!(offline.supervisor().broker().inspect().ledger.stages[&1], ActionState::Unknown);
    assert_eq!(offline.supervisor().broker().inspect().ledger.charged, 16);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Unknown { .. }));
    drop(endpoint);
    let endpoint = key.reopen().unwrap();
    let mut driver = offline.reconnect(endpoint, ElapsedTick(2)).unwrap();
    assert_eq!(driver.phase(), DriverPhase::Idle);
    assert_eq!(driver.reconcile_pending(ElapsedTick(2)).unwrap().len(), 1);
    assert!(driver.reconcile_pending(ElapsedTick(2)).unwrap().is_empty());
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert_eq!(driver.endpoint().execution_count(), 1); assert_eq!(driver.endpoint().payload(), b"publish");
    assert_eq!(driver.supervisor().broker().inspect().ledger.charged, 16);
    assert_eq!(driver.supervisor().broker().dispatched_decoder_evidence(1).unwrap().unwrap().stream(), 7);
    assert_eq!(rig.port.submit(1, &proposal()).unwrap().request(), 1);
}

#[test]
fn incident_suspension_closes_new_intake_without_erasing_historical_actor_keys() {
    let (mut rig, _) = Rig::new(false); let checkpoint = own(&mut rig);
    for round in [100, 101] {
        assert!(rig.driver.reset_hosted_decoder(request(rig.driver.supervisor().broker(), &checkpoint, round)).unwrap().control.restored);
    }
    let mut pending = proposal(); pending.expected_policy_epoch = 2;
    let ticket = rig.port.submit(1, &pending).unwrap();
    let reset = rig.driver.reset_hosted_decoder(request(rig.driver.supervisor().broker(), &checkpoint, 102)).unwrap();
    assert!(!reset.control.restored); assert!(rig.driver.supervisor().broker().inspect().suspended);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    pending.expected_policy_epoch = 3;
    assert_eq!(rig.port.submit(2, &pending).unwrap_err(), ActorError::Unavailable);
    assert!(rig.driver.supervisor().stop_receipt().is_none());
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
}

// Test-subprocess entry point, not an additional independent scenario.
#[test]
fn hosted_reset_child() {
    let Some(path) = std::env::var_os("FA_HOSTED_RESET_READY") else { return; };
    let path = PathBuf::from(path); let pending = path.with_extension("pending");
    fs::write(&pending, std::process::id().to_string()).unwrap(); fs::rename(pending, path).unwrap();
    std::thread::sleep(Duration::from_secs(30));
    panic!("parent failed to stop the abandoned helper");
}

#[test]
fn lower_level_reset_reaps_real_children_even_when_the_next_driver_clock_refuses() {
    let directory = Directory::new();
    let (mut rig, _) = Rig::new(false); let checkpoint = own(&mut rig);
    rig.accept(1);
    let launch = rig.launch(1, 1); drop(launch.streams); rig.clients.clear();
    let programs = ["alice", "bob"].into_iter().map(|member| {
        (member.to_owned(), HelperProgram::new(std::env::current_exe().unwrap(), directory.0.clone(),
            vec!["--exact".into(), "hosted_reset_child".into(), "--nocapture".into()],
            BTreeMap::from([("FA_HOSTED_RESET_READY".into(), directory.0.join(format!("{member}.ready")).into_os_string())])).unwrap())
    }).collect();
    rig.driver.start_process_review(ProcessReviewLaunch { request: launch.request, round: launch.round,
        evidence_root: launch.evidence_root, window: launch.window, expected_input_revision: launch.expected_input_revision,
        inputs: launch.inputs, programs, limits: launch.limits }, &snapshot()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let pids = loop {
        let states = rig.driver.reap_helpers();
        assert!(states.values().all(|s| !s.stop_requested && s.exit.is_none()));
        if states.len() == 2 && states.iter().all(|(member, state)| {
            fs::read_to_string(directory.0.join(format!("{member}.ready"))).ok()
                .is_some_and(|pid| pid.parse::<u32>().unwrap() == state.pid)
        }) { break states; }
        assert!(Instant::now() < deadline, "helper readiness deadline");
        std::thread::sleep(Duration::from_millis(1));
    };
    let reset = request(rig.driver.supervisor().broker(), &checkpoint, 100);
    rig.driver.supervisor_mut().broker_mut().reset_hosted_decoder(reset).unwrap();
    assert_eq!(rig.driver.step(ElapsedTick(0), None, &snapshot(), None).unwrap_err(), DriverError::Control(Error::Stale));
    assert!(rig.driver.helper_processes().values().all(|s| s.stop_requested));
    let deadline = Instant::now() + Duration::from_secs(10);
    while !rig.driver.helpers_reaped() {
        rig.driver.reap_helpers();
        assert!(Instant::now() < deadline, "helper reaping deadline");
        std::thread::sleep(Duration::from_millis(1));
    }
    for (member, state) in rig.driver.helper_processes() {
        assert_eq!(state.pid, pids[&member].pid); assert!(state.exit.is_some());
    }
    assert!(matches!(rig.driver.step(ElapsedTick(1), None, &snapshot(), None).unwrap(),
        DriverEvent::Stopped { request: 1, state: ActionState::Cancelled }));
    assert_eq!(rig.driver.phase(), DriverPhase::Idle);
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.stages[&1], ActionState::Cancelled);
}
