#![cfg(unix)]
#[path = "support/file_helper.rs"] mod fixture;
use fixture::*;
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::helpers::processes::{
    FileHelperProcessLaunch, FileHelperProcesses, FileProcessFailure, HelperRoundAdmission,
};
use fa_reference::action::consequence::oversight::helper_client::{ClientPhase, HelperClient};
use fa_reference::action::consequence::oversight::helper_processes::{HelperChildren, HelperProgram};
use fa_reference::action::consequence::oversight::helper_workers::HelperLimits;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

const MODE: &str = "FA_DURABLE_HELPER_FIXTURE_MODE";
const MEMBER: &str = "FA_DURABLE_HELPER_FIXTURE_MEMBER";
const CHILD: &str = "durable_helper_process_child";

/// An explicitly synthetic helper executable. It uses the real inherited
/// socket and original client, inspects the input and emits one frozen response.
#[test]
fn durable_helper_process_child() {
    let Ok(mode) = std::env::var(MODE) else { return; };
    if mode == "exit_without_vote" { return; }
    let member = std::env::var(MEMBER).unwrap();
    let contracts = profile().committee;
    let expected = contracts.members()[&member].profile_at(0);
    let mut client = HelperClient::from_process_stdin(expected).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference && mode != "wait" {
            let input = client.input().unwrap();
            assert_eq!(input.member(), member);
            assert_eq!(input.round(), 101);
            let submitted = input.actual_input().submitted_bytes();
            let evidence = b"complete original helper context";
            assert!(submitted.windows(evidence.len()).any(|part| part == evidence));
            assert_eq!(mode, "allow");
            client.respond(Verdict::Allow, &oversight::salt(&member)).unwrap();
        }
        if client.phase() == ClientPhase::ReplySent { return; }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("fixture exceeded its process bound");
}

fn program(root: &Directory, member: &str, mode: &str) -> HelperProgram {
    HelperProgram::new(std::env::current_exe().unwrap(), root.0.clone(),
        vec![OsString::from("--exact"), OsString::from(CHILD), OsString::from("--nocapture")],
        BTreeMap::from([(OsString::from(MODE), OsString::from(mode)),
            (OsString::from(MEMBER), OsString::from(member))])).unwrap()
}
fn programs(root: &Directory, alpha: &str, beta: &str) -> BTreeMap<String, HelperProgram> {
    BTreeMap::from([("alpha".to_owned(), program(root, "alpha", alpha)),
        ("beta".to_owned(), program(root, "beta", beta))])
}
fn config(host: &FileOversight, programs: BTreeMap<String, HelperProgram>) -> FileHelperProcessLaunch {
    FileHelperProcessLaunch { attempt: 1, round: 101, evidence_root: ROOT,
        window: window(host), expected_input_revision: host.input_revision(1).unwrap(),
        programs, limits: HelperLimits::default() }
}
fn drive_until<F>(pool: &mut FileHelperProcesses, host: &mut FileOversight, now: ElapsedTick, done: F)
where F: Fn(&FileHelperProcesses) -> bool {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        pool.pump(host, now).unwrap();
        if done(pool) { return; }
        assert!(Instant::now() < deadline, "worker progress exceeded the fixture bound");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn reap(pool: &mut FileHelperProcesses) {
    pool.stop_workers();
    let deadline = Instant::now() + Duration::from_secs(15);
    while !pool.all_reaped() {
        pool.reap();
        assert!(Instant::now() < deadline, "retained child cleanup did not complete");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn reap_children(children: &mut HelperChildren) {
    children.request_stop_all();
    let deadline = Instant::now() + Duration::from_secs(15);
    while !children.all_reaped() {
        children.reap();
        assert!(Instant::now() < deadline, "failed-launch cleanup did not complete");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn spawned_helpers_drive_the_original_full_input_human_and_publication_pipeline() {
    let root = Directory::new();
    let (mut host, reviewer) = create(&root);
    let (action, input) = prepare(&mut host, 1, b"from processes");
    let launch = config(&host, programs(&root, "allow", "allow"));
    let mut pool = host.begin_helper_processes(host.revision(), launch, snapshot(), || ElapsedTick(1)).unwrap();
    let children = pool.process_statuses();
    assert_eq!(children.len(), 2);
    assert_ne!(children["alpha"].pid, children["beta"].pid);
    drive_until(&mut pool, &mut host, ElapsedTick(1), FileHelperProcesses::ready_to_finish);
    let receipt = pool.finish(&mut host, ElapsedTick(1), Some(&input), snapshot()).unwrap().unwrap();
    assert_eq!(receipt.policy.control.decision.consequence, Consequence::Continue);
    assert_eq!(receipt.inputs.as_ref(), &input);
    let automatic = host.authorize(host.revision(), 1, &input, snapshot()).unwrap();
    assert!(host.publish(host.revision(), 1).is_err());
    let request = host.request_human_approval(host.revision(), 1001, 1, &input, ElapsedTick(31)).unwrap();
    let revision = host.revision();
    let human = reviewer.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
    assert_eq!(host.publish(host.revision(), 1).unwrap(), EndpointOutcome::Executed { resulting_version: 2 });
    reap(&mut pool);
    assert!(pool.process_statuses().values().all(|status| status.exit.is_some()));
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().payload, b"from processes");
}

#[test]
fn successful_process_exit_without_a_reveal_is_missing_evidence_not_allow() {
    let root = Directory::new();
    let (mut host, _) = create(&root);
    let (_, input) = prepare(&mut host, 1, b"must hold");
    let launch = config(&host, programs(&root, "allow", "exit_without_vote"));
    let cutoffs = launch.window;
    let mut pool = host.begin_helper_processes(host.revision(), launch, snapshot(), || ElapsedTick(1)).unwrap();
    drive_until(&mut pool, &mut host, ElapsedTick(1), |pool| {
        pool.statuses()["alpha"].committed && pool.process_statuses()["beta"].exit.is_some()
    });
    assert!(pool.process_statuses()["beta"].exit.unwrap().success);
    assert!(!pool.statuses()["beta"].revealed);
    drive_until(&mut pool, &mut host, cutoffs.commit_by, |pool| pool.statuses()["alpha"].revealed);
    assert!(!pool.ready_to_finish());
    let receipt = pool.finish(&mut host, cutoffs.reveal_by, Some(&input), snapshot()).unwrap().unwrap();
    assert_eq!(receipt.policy.control.decision.consequence, Consequence::HoldEffect);
    assert!(host.authorize(host.revision(), 1, &input, snapshot()).is_err());
    assert_eq!(host.inspect().executions, 0);
    reap(&mut pool);
}

#[test]
fn partial_spawn_returns_the_started_child_owner_and_does_not_enable_manual_votes() {
    let root = Directory::new();
    let (mut host, _) = create(&root);
    let (_, input) = prepare(&mut host, 1, b"not launched");
    let not_executable = root.0.join("not-executable");
    std::fs::write(&not_executable, b"not an executable image").unwrap();
    std::fs::set_permissions(&not_executable, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut programs = programs(&root, "wait", "allow");
    programs.insert("beta".to_owned(), HelperProgram::new(not_executable, root.0.clone(), Vec::new(), BTreeMap::new()).unwrap());
    let launch = config(&host, programs);
    let mut error = host.begin_helper_processes(host.revision(), launch, snapshot(), || ElapsedTick(1)).unwrap_err();
    assert_eq!(error.admission, HelperRoundAdmission::Committed);
    assert!(matches!(&error.failure, FileProcessFailure::Launch { member: Some(member), .. } if member == "beta"));
    let children = error.children.as_mut().unwrap();
    assert_eq!(children.statuses().len(), 1);
    assert!(children.statuses()["alpha"].stop_requested);
    reap_children(children);
    assert_eq!(host.commit_review(host.revision(), 101, "alpha", 0).unwrap_err(), JournalError::Contract(Error::WrongState));
    assert!(host.authorize(host.revision(), 1, &input, snapshot()).is_err());
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn time_spent_spawning_cannot_extend_the_original_commit_deadline() {
    let root = Directory::new();
    let (mut host, _) = create(&root);
    let (_, input) = prepare(&mut host, 1, b"too late");
    let launch = config(&host, programs(&root, "wait", "wait"));
    let cutoff = launch.window.commit_by;
    let mut calls = 0;
    let mut error = host.begin_helper_processes(host.revision(), launch, snapshot(), || {
        calls += 1;
        if calls == 1 { ElapsedTick(1) } else { cutoff }
    }).unwrap_err();
    assert_eq!(error.admission, HelperRoundAdmission::Committed);
    assert_eq!(error.failure, FileProcessFailure::Journal(JournalError::Contract(Error::Stale)));
    assert_eq!(host.inspect().control.ledger.elapsed, Some(cutoff));
    let children = error.children.as_mut().unwrap();
    assert_eq!(children.statuses().len(), 2);
    reap_children(children);
    assert!(host.authorize(host.revision(), 1, &input, snapshot()).is_err());
    assert!(host.open_reveals(host.revision(), 101).is_err());
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn post_spawn_storage_failure_keeps_cleanup_ownership_and_recovers_no_review_permission() {
    let root = Directory::new();
    let (mut host, _) = create(&root);
    let _ = prepare(&mut host, 1, b"no input sent");
    let launch = config(&host, programs(&root, "wait", "wait"));
    let pending = root.store().join("delivery.pending");
    let mut calls = 0;
    let mut error = host.begin_helper_processes(host.revision(), launch, snapshot(), || {
        calls += 1;
        if calls == 2 { std::fs::write(&pending, b"inert staged bytes").unwrap(); }
        ElapsedTick(calls)
    }).unwrap_err();
    assert_eq!(error.admission, HelperRoundAdmission::Committed);
    assert!(matches!(error.failure, FileProcessFailure::Journal(JournalError::Io(_))));
    assert!(host.storage_failure().is_some());
    reap_children(error.children.as_mut().unwrap());
    drop(host);
    let (mut recovered, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(recovered.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(recovered.inspect().control.ledger.available, 100);
    recovered.observe_time(recovered.revision(), ElapsedTick(2)).unwrap();
    assert!(recovered.commit_review(recovered.revision(), 101, "alpha", 0).is_err());
    assert_eq!(recovered.inspect().executions, 0);
}

#[test]
fn cancelling_the_original_attempt_stops_child_io_without_creating_a_review() {
    let root = Directory::new();
    let (mut host, _) = create(&root);
    let _ = prepare(&mut host, 1, b"cancelled");
    let launch = config(&host, programs(&root, "wait", "wait"));
    let mut pool = host.begin_helper_processes(host.revision(), launch, snapshot(), || ElapsedTick(1)).unwrap();
    host.cancel(host.revision(), 1).unwrap();
    let before = host.inspect();
    let failure = pool.pump(&mut host, ElapsedTick(1)).unwrap_err();
    assert_eq!(failure.error, JournalError::Contract(Error::WrongState));
    assert!(failure.progress.io.is_empty());
    assert!(pool.is_closed());
    reap(&mut pool);
    assert_eq!(host.inspect(), before);
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(host.inspect().executions, 0);
}
