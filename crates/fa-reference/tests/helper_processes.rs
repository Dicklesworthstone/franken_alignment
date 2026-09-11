//! Real direct-child launches; deterministic fixtures are not trained helpers.
#![cfg(unix)]
#![forbid(unsafe_code)]

#[path = "support/helper_workers.rs"]
mod support;

use support::{Fixture, snapshot};
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::oversight::helper_client::{ClientProgress, HelperClient};
use fa_reference::action::consequence::oversight::helper_processes::{
    HelperChildren, HelperProgram, ProcessFailure, ProcessStage, launch_helpers,
    MAX_PROGRAM_ARGUMENTS, MAX_PROGRAM_ENVIRONMENT, MAX_PROGRAM_FIELD_BYTES,
};
use fa_reference::action::consequence::oversight::helper_workers::HelperLimits;
use fa_reference::action::consequence::oversight::helper_workers::io::HelperPool;
use fa_reference::action::ElapsedTick;
use fa_reference::full_input::InputProfileBinding;
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

fn program(member: &str, mode: &str, executable: PathBuf) -> HelperProgram {
    HelperProgram::new(executable, std::env::temp_dir().canonicalize().unwrap(),
        vec!["--exact".into(), "worker_entry".into(), "--nocapture".into()],
        BTreeMap::from([
            ("FA_LAUNCH_WORKER".into(), "1".into()),
            ("FA_LAUNCH_MEMBER".into(), member.into()),
            ("FA_LAUNCH_MODE".into(), mode.into()),
        ])).unwrap()
}
fn programs(mode: &str) -> BTreeMap<String, HelperProgram> {
    ["alpha", "beta"].into_iter().map(|member| (member.to_owned(),
        program(member, mode, std::env::current_exe().unwrap()))).collect()
}
fn reap(children: &mut HelperChildren) {
    let until = Instant::now() + Duration::from_secs(5);
    while !children.all_reaped() {
        children.reap();
        assert!(Instant::now() < until, "children not reaped: {children:?}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn worker_entry() {
    if std::env::var_os("FA_LAUNCH_WORKER").is_none() { return; }
    assert!(std::env::var_os("FA_PROCESS_SECRET").is_none());
    assert!(std::env::var_os("PATH").is_none());
    let member = std::env::var("FA_LAUNCH_MEMBER").unwrap();
    let mode = std::env::var("FA_LAUNCH_MODE").unwrap();
    let index = match member.as_str() { "alpha" => 1, "beta" => 2, _ => panic!("unexpected member") };
    if mode == "exit" { return; }
    if mode == "hang" { std::thread::sleep(Duration::from_secs(30)); return; }
    // Test-harness and worker logs must not be interpreted as vote bytes.
    println!("private worker diagnostic for {member}");
    let mut client = HelperClient::from_process_stdin(InputProfileBinding {
        profile_id: index, profile_bytes: format!("{member}-profile").into_bytes(),
        model_epoch: index, tokenizer_epoch: 1, policy_epoch: 0,
    }).unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(Instant::now() < until, "worker protocol did not finish");
        match client.step().unwrap() {
            ClientProgress::NeedsInference => {
                let input = client.input().unwrap().actual_input().submitted_bytes();
                assert!(input.ends_with(format!("{member}-private-question").as_bytes()));
                let verdict = if mode == "hold" { Verdict::Hold }
                    else if input.windows(7).any(|bytes| bytes == b"publish") { Verdict::Allow }
                    else { Verdict::Deny };
                client.respond(verdict, b"fixed-fixture-salt").unwrap();
            }
            ClientProgress::ReplySent => break,
            ClientProgress::Blocked => std::thread::sleep(Duration::from_millis(1)),
            ClientProgress::Progress => {}
        }
    }
}

fn exercise(mode: &str) {
    let mut fixture = Fixture::new();
    let session = fixture.start(11);
    let (streams, mut children) = launch_helpers(fixture.broker.contracts(), &programs(mode)).unwrap();
    assert_eq!(children.statuses().len(), 2);
    assert_ne!(children.statuses()["alpha"].pid, children.statuses()["beta"].pid);
    let mut pool = HelperPool::new(session, streams, HelperLimits::default()).unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    while !pool.ready_to_finish() {
        pool.pump(ElapsedTick(1)).unwrap();
        children.reap();
        assert!(Instant::now() < until, "helper responses not completed");
        std::thread::sleep(Duration::from_millis(1));
    }
    let review = pool.finish(ElapsedTick(1)).unwrap();
    let expected = if mode == "hold" { Consequence::HoldEffect } else { Consequence::Continue };
    assert_eq!(review.decision().consequence, expected);
    fixture.broker.apply_review(review, Some(&fixture.inputs), &snapshot()).unwrap();
    if mode == "hold" {
        assert!(fixture.broker.authorize(1, Some(&fixture.inputs), &snapshot()).is_err());
        assert_eq!(fixture.endpoint.execution_count(), 0);
    } else {
        let permit = fixture.broker.authorize(1, Some(&fixture.inputs), &snapshot()).unwrap();
        let message = fixture.broker.dispatch(&permit, &fixture.action, Some(&fixture.inputs), &snapshot()).unwrap();
        let receipt = fixture.endpoint.deliver(&message).unwrap();
        fixture.broker.accept_receipt(receipt).unwrap();
        assert_eq!(fixture.endpoint.payload(), b"publish");
        assert_eq!(fixture.endpoint.execution_count(), 1);
    }
    children.request_stop_all();
    reap(&mut children);
    assert!(children.statuses().values().all(|status| status.exit.is_some()));
}

#[test]
fn configured_processes_vote_through_the_original_congress_and_publish_once() { exercise("allow"); }

#[test]
fn adverse_process_votes_remain_holds_not_successful_process_exits() { exercise("hold"); }

#[test]
fn environment_probe_entry() {
    if std::env::var_os("FA_ENVIRONMENT_PROBE").is_none() { return; }
    assert_eq!(std::env::var("FA_PROCESS_SECRET").unwrap(), "must-not-be-inherited");
    exercise("allow");
}

#[test]
fn an_explicitly_seeded_parent_credential_is_not_inherited_by_helpers() {
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "environment_probe_entry", "--nocapture"])
        .env("FA_ENVIRONMENT_PROBE", "1").env("FA_PROCESS_SECRET", "must-not-be-inherited")
        .output().unwrap();
    assert!(output.status.success(), "probe stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
}

#[test]
fn a_clean_exit_without_a_reveal_remains_missing() {
    let mut fixture = Fixture::new();
    let session = fixture.start(12);
    let (streams, mut children) = launch_helpers(fixture.broker.contracts(), &programs("exit")).unwrap();
    let mut pool = HelperPool::new(session, streams, HelperLimits::default()).unwrap();
    reap(&mut children);
    assert!(children.statuses().values().all(|status| status.exit.unwrap().success));
    pool.pump(ElapsedTick(10)).unwrap();
    let review = pool.finish(ElapsedTick(10)).unwrap();
    assert_eq!(review.missing(), &["alpha".to_owned(), "beta".to_owned()]);
    assert_ne!(review.decision().consequence, Consequence::Continue);
    assert_eq!(fixture.endpoint.execution_count(), 0);
}

#[test]
fn roster_and_metadata_failures_spawn_nothing() {
    let fixture = Fixture::new();
    let mut wrong = programs("hang");
    wrong.remove("beta");
    let failure = launch_helpers(fixture.broker.contracts(), &wrong).unwrap_err();
    assert_eq!(failure.failure, ProcessFailure::Refused(Error::Binding));
    assert!(failure.children.statuses().is_empty());
    let mut missing = programs("hang");
    missing.insert("beta".to_owned(), program("beta", "hang",
        std::env::temp_dir().join(format!("fa-absent-program-{}", std::process::id()))));
    let failure = launch_helpers(fixture.broker.contracts(), &missing).unwrap_err();
    assert!(matches!(failure.failure, ProcessFailure::Io { stage: ProcessStage::InspectProgram, .. }));
    assert!(failure.children.statuses().is_empty());
}

#[test]
fn partial_spawn_failure_returns_the_started_child_for_explicit_reaping() {
    let fixture = Fixture::new();
    let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let path = std::env::temp_dir().join(format!("fa-no-execute-{}-{unique}", std::process::id()));
    let file = fs::OpenOptions::new().write(true).create_new(true).open(&path).unwrap();
    file.set_permissions(fs::Permissions::from_mode(0o600)).unwrap();
    drop(file);
    let mut commands = programs("hang");
    commands.insert("beta".to_owned(), program("beta", "hang", path.clone()));
    let mut failure = launch_helpers(fixture.broker.contracts(), &commands).unwrap_err();
    fs::remove_file(path).unwrap();
    assert_eq!(failure.member.as_deref(), Some("beta"));
    assert!(matches!(failure.failure, ProcessFailure::Io { stage: ProcessStage::Spawn, .. }));
    assert_eq!(failure.children.statuses().keys().map(String::as_str).collect::<Vec<_>>(), vec!["alpha"]);
    assert!(failure.children.statuses()["alpha"].stop_requested);
    reap(&mut failure.children);
    assert!(failure.children.statuses()["alpha"].exit.is_some());
    assert_eq!(fixture.endpoint.execution_count(), 0);
}

#[test]
fn command_bounds_and_debug_output_do_not_expose_host_configuration() {
    let make = |args: Vec<OsString>, env| HelperProgram::new("/program".into(), "/".into(), args, env);
    assert!(make(vec!["x".repeat(MAX_PROGRAM_FIELD_BYTES).into()], BTreeMap::new()).is_ok());
    assert_eq!(make(vec!["x".repeat(MAX_PROGRAM_FIELD_BYTES + 1).into()], BTreeMap::new()).unwrap_err(), Error::Limit);
    assert!(make(vec!["x".into(); MAX_PROGRAM_ARGUMENTS], BTreeMap::new()).is_ok());
    assert_eq!(make(vec!["x".into(); MAX_PROGRAM_ARGUMENTS + 1], BTreeMap::new()).unwrap_err(), Error::Limit);
    let env = (0..MAX_PROGRAM_ENVIRONMENT + 1).map(|i| (format!("KEY{i}").into(), "v".into())).collect();
    assert_eq!(make(vec![], env).unwrap_err(), Error::Limit);
    for key in ["", "bad=key", "bad\0key"] {
        assert_eq!(make(vec![], BTreeMap::from([(key.into(), "v".into())])).unwrap_err(), Error::InvalidInput);
    }
    let configured = make(vec!["private-argument".into()], BTreeMap::from([("TOKEN".into(), "private-value".into())])).unwrap();
    let debug = format!("{configured:?}");
    assert!(!debug.contains("private-argument"));
    assert!(!debug.contains("private-value"));
    assert!(HelperProgram::new("relative".into(), "/".into(), vec![], BTreeMap::new()).is_err());
}
