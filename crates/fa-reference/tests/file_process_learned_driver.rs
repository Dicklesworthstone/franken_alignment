//! Real executable children, original learned source and durable actor tickets.
//! Synthetic worker ballots test transport/authority, not detector quality.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[path = "support/restart_model.rs"]
#[allow(dead_code)]
mod model;
#[path = "file_process_learned_driver/fixture.rs"]
mod fixture;
use fixture::*;
use fa_reference::{Error, Snapshot};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::delivery::persistent::{JournalError,
    observed::{FileOversight as Host, FileOversightProfile, FileHumanReviewer,
        decoder::learned::{FileLearnedConfig, sidecar::FileLearnedSidecarFinish},
        driver::{FileSupervisedDriver, FileDriverPhase, FileDriverEvent,
            process_learned::{FileProcessDriverLaunch, FileProcessSupervisedDriver,
                FileProcessDriverEvent, FileProcessDriverHandoff}},
        helpers::{processes::{FileProcessFailure, HelperRoundAdmission},
            learned_sockets::processes::sequence::{LearnedProcessLimits, LearnedProcessStatus}}}};
use fa_reference::action::consequence::oversight::{
    actor::{ActorProposal, ActorOutcome, Knowledge},
    helper_client::{HelperClient, ClientPhase},
    helper_processes::{HelperProgram, HelperChildren},
    helper_workers::{HelperLimits, HelperPhase}};
use fa_reference::round::Verdict;
use std::collections::BTreeMap;
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

/// This same executable becomes a child only with its explicitly supplied mode.
#[test]
fn process_worker() {
    let Ok(mode) = std::env::var("FA_PROCESS_DRIVER_TEST") else { return; };
    std::fs::write("started", std::process::id().to_string()).unwrap();
    if mode == "exit" { return; }
    let expected = profile().committee.members()["reviewer"].profile_at(0);
    let mut client = HelperClient::<UnixStream>::from_process_stdin(expected).unwrap();
    let end = Instant::now() + Duration::from_secs(10);
    let mut observed = false;
    loop {
        if client.step().is_err() { return; }
        if client.phase() == ClientPhase::NeedsInference {
            if !observed {
                let input = client.input().unwrap();
                assert_eq!(input.member(), "reviewer");
                let sidecar = input.actual_input().part_bytes(1).unwrap();
                assert_eq!(&sidecar[..8], b"FASIDE\0\x01");
                std::fs::write("observed", sidecar).unwrap();
                observed = true;
            }
            if mode != "wait" && (mode != "gated" || std::path::Path::new("release").exists()) {
                let verdict = if mode == "abstain" { Verdict::Abstain } else { Verdict::Allow };
                client.respond(verdict, &[11; 32]).unwrap();
            }
        }
        if client.phase() == ClientPhase::AwaitingReveal {
            std::fs::write("commit_sent", b"original commitment fully written").unwrap();
        }
        if client.phase() == ClientPhase::ReplySent { return; }
        assert!(Instant::now() < end, "original worker protocol exceeded fixture bound");
        pause();
    }
}

#[test]
fn external_refinement_reaps_then_uses_both_keys_for_the_original_actor_publication() {
    let (root, port, ticket, reviewer, driver, launch) = setup(&["abstain", "allow"]);
    let mut source = policy_source(&root);
    let run = driver.start_process_learned_sequence(launch, snapshot(), || ElapsedTick(1)).unwrap();
    let initial = run.review().input().clone();
    let mut run = *run.into_handoff().unwrap_err();
    assert_eq!(run.progress().admitted_starts, 2);
    assert_eq!(run.review().records()[&102].admission, None);
    until(|| {
        if run.review().status() == LearnedProcessStatus::Draining { return true; }
        run.step_from_policy_file(&mut source, || ElapsedTick(1), None).result.unwrap();
        false
    });
    assert!(matches!(run.review().history()[0], FileLearnedSidecarFinish::Refined { .. }));
    assert!(!root.round(102).join("started").exists());
    assert_ne!(run.review().input(), &initial);
    assert!(run.progress().input_revision > 1);
    let mut run = *run.into_handoff().unwrap_err(); // Draining is still sealed.
    finish(&mut run, &mut source, 1);
    assert_eq!(run.review().history().len(), 2);
    let first = &run.review().records()[&101].processes["reviewer"];
    let next = &run.review().records()[&102].processes["reviewer"];
    assert!(first.exit.is_some() && next.exit.is_some());
    for (round, process) in [(101, first), (102, next)] {
        assert_eq!(std::fs::read_to_string(root.round(round).join("started")).unwrap().parse::<u32>().unwrap(), process.pid);
    }
    assert_ne!(std::fs::read(root.round(101).join("observed")).unwrap(),
        std::fs::read(root.round(102).join("observed")).unwrap());
    assert_eq!(run.progress().phase, FileDriverPhase::AwaitingDispatch { request: 41 });
    assert!(matches!(run.step_from_policy_file(&mut source, || ElapsedTick(1), None).result.unwrap(),
        FileProcessDriverEvent::Driver(FileDriverEvent::AwaitingHuman { request: 41 })));
    assert_eq!(run.supervisor().host().unwrap().inspect().control.ledger.available, 100);
    let request = run.request_human_approval_from_policy_file(&mut source, 1001,
        ElapsedTick(40), || ElapsedTick(1)).result.unwrap();
    let human = {
        let mut host = run.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        reviewer.approve(&mut host, revision, &request).unwrap()
    };
    assert!(matches!(run.step_from_policy_file(&mut source, || ElapsedTick(1), Some(&human)).result.unwrap(),
        FileProcessDriverEvent::Driver(FileDriverEvent::Dispatched { request: 41, .. })));
    assert_eq!(run.supervisor().host().unwrap().inspect().executions, 0);
    assert!(matches!(run.step_from_policy_file(&mut source, || ElapsedTick(2), None).result.unwrap(),
        FileProcessDriverEvent::Driver(FileDriverEvent::PublicationChecked { .. })));
    assert!(matches!(run.step_from_policy_file(&mut source, || ElapsedTick(2), None).result.unwrap(),
        FileProcessDriverEvent::Driver(FileDriverEvent::Reconciled { .. })));
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let publication = Host::read_publication_with_learned_generation(root.store(), &profile(), &config()).unwrap();
    assert_eq!(publication.executions, 1);
    assert_eq!(publication.payload, b"visible");
    assert_eq!(publication.control.ledger.charged, 16);
    let FileProcessDriverHandoff { driver, review } = run.into_handoff().unwrap();
    assert!(review.all_reaped());
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 1);
}

#[test]
fn actual_child_exit_never_becomes_a_vote_or_a_ready_actor_job() {
    let (root, _port, _ticket, _reviewer, driver, launch) = setup(&["exit"]);
    let mut source = policy_source(&root);
    let mut run = driver.start_process_learned_sequence(launch, snapshot(), || ElapsedTick(1)).unwrap();
    until(|| {
        if !active(&run) { return true; }
        let _ = run.step_from_policy_file(&mut source, || ElapsedTick(1), None);
        !run.review().records()[&101].socket.wire_failures.is_empty()
    });
    if active(&run) { let _ = run.step_from_policy_file(&mut source, || ElapsedTick(30), None); }
    assert!(!active(&run));
    cleanup(&mut run);
    let record = &run.review().records()[&101];
    assert!(record.processes["reviewer"].exit.is_some());
    assert!(!record.socket.wire_failures.is_empty());
    assert!(!record.socket.workers["reviewer"].revealed);
    assert!(!root.round(101).join("observed").exists());
    assert_ne!(run.progress().phase, FileDriverPhase::AwaitingDispatch { request: 41 });
    assert!(run.request_human_approval(1001, ElapsedTick(60), ElapsedTick(30)).is_err());
    assert_eq!(run.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn late_or_panicking_post_launch_clock_returns_the_same_gateway_and_real_children() {
    for panic_after_start in [false, true] {
        let (root, port, ticket, _reviewer, driver, launch) = setup(&["allow"]);
        let mut calls = 0;
        let mut error = driver.start_process_learned_sequence(launch, snapshot(), || {
            calls += 1;
            if calls == 2 {
                if panic_after_start { panic!("post-launch observation interrupted"); }
                return ElapsedTick(20);
            }
            ElapsedTick(1)
        }).unwrap_err();
        assert_eq!(calls, 2);
        assert_eq!(error.admission, HelperRoundAdmission::Committed);
        assert_eq!(error.error, if panic_after_start { FileProcessFailure::PostLaunchClockPanicked }
            else { FileProcessFailure::Journal(JournalError::Contract(Error::Stale)) });
        let children = error.children.as_mut().expect("launched child retained");
        assert_eq!(children.statuses().len(), 1);
        assert!(children.statuses()["reviewer"].stop_requested);
        cleanup_children(children);
        assert!(children.statuses()["reviewer"].exit.is_some());
        assert!(!root.round(101).join("observed").exists());
        assert!(matches!(port.poll(&ticket), Knowledge::Pending { request: 41 }));
        assert_eq!(error.driver.phase(), FileDriverPhase::Idle);
        assert_eq!(error.driver.supervisor().host().unwrap().inspect().executions, 0);
    }
}

#[test]
fn buffered_commitment_uses_its_post_io_time_including_the_exact_commit_cutoff() {
    for late in [false, true] {
        let (root, _port, _ticket, _reviewer, driver, launch) = setup(&["gated"]);
        let mut source = policy_source(&root);
        let mut run = driver.start_process_learned_sequence(launch, snapshot(), || ElapsedTick(1)).unwrap();
        until(|| {
            run.advance(run.review().revision(), ElapsedTick(1), snapshot()).unwrap();
            root.round(101).join("observed").exists()
        });
        assert!(!run.review().records()[&101].socket.workers["reviewer"].committed);
        std::fs::write(root.round(101).join("release"), b"allow original response").unwrap();
        until(|| root.round(101).join("commit_sent").exists());
        let before = run.review().records()[&101].socket.connection_steps;
        let mut calls = 0;
        let post = if late { 20 } else { 19 };
        run.advance_with_clock(run.review().revision(), || {
            calls += 1;
            ElapsedTick(if calls < 3 { 1 } else { post })
        }, snapshot()).unwrap();
        assert_eq!(calls, 3);
        assert_eq!(run.review().records()[&101].socket.connection_steps, before + 1);
        assert_eq!(run.review().records()[&101].socket.workers["reviewer"].committed, !late);
        if late {
            assert_eq!(run.review().records()[&101].socket.workers["reviewer"].phase, HelperPhase::Failed);
            finish(&mut run, &mut source, 30);
            assert_ne!(run.progress().phase, FileDriverPhase::AwaitingDispatch { request: 41 });
            assert!(run.request_human_approval(1001, ElapsedTick(60), ElapsedTick(30)).is_err());
        } else {
            finish(&mut run, &mut source, 19);
            assert_eq!(run.progress().phase, FileDriverPhase::AwaitingDispatch { request: 41 });
        }
        assert_eq!(run.supervisor().host().unwrap().inspect().executions, 0);
    }
}

#[test]
fn cancellation_and_withdrawn_policy_stop_only_the_launched_roster() {
    for cancelled in [false, true] {
        let (root, port, ticket, _reviewer, driver, launch) = setup(&["wait", "allow"]);
        let mut source = policy_source(&root);
        let mut run = driver.start_process_learned_sequence(launch, snapshot(), || ElapsedTick(1)).unwrap();
        until(|| {
            run.step_from_policy_file(&mut source, || ElapsedTick(1), None).result.unwrap();
            root.round(101).join("observed").exists()
        });
        if cancelled {
            port.cancel(&ticket).unwrap();
            run.step(|| panic!("cancelled action sampled time"),
                || panic!("cancelled action read policy"), None).unwrap();
        } else {
            std::fs::remove_file(root.0.join("policy.json")).unwrap();
            assert!(run.step_from_policy_file(&mut source, || ElapsedTick(1), None).result.is_err());
        }
        assert_eq!(run.review().status(), LearnedProcessStatus::Cancelled);
        cleanup(&mut run);
        assert_eq!(run.review().records()[&102].admission, None);
        assert!(!root.round(102).join("started").exists());
        assert!(run.review().records()[&101].processes["reviewer"].stop_requested);
        assert!(run.request_human_approval(1001, ElapsedTick(40), ElapsedTick(1)).is_err());
        assert_eq!(run.supervisor().host().unwrap().inspect().executions, 0);
    }
}

#[test]
fn source_loss_after_refinement_is_checked_before_policy_read_or_successor_start() {
    let (root, _port, _ticket, _reviewer, driver, launch) = setup(&["abstain", "allow"]);
    let mut source = policy_source(&root);
    let mut run = driver.start_process_learned_sequence(launch, snapshot(), || ElapsedTick(1)).unwrap();
    until(|| {
        if run.review().status() == LearnedProcessStatus::Draining { return true; }
        run.step_from_policy_file(&mut source, || ElapsedTick(1), None).result.unwrap();
        false
    });
    let archive = run.review().history()[0].archive().clone();
    step_source(&mut run.supervisor_mut().host_mut().unwrap());
    let reads = source.status().read_attempts;
    assert!(run.step_from_policy_file(&mut source, || ElapsedTick(1), None).result.is_err());
    assert_eq!(source.status().read_attempts, reads);
    cleanup(&mut run);
    assert_eq!(run.review().records()[&102].admission, None);
    assert!(!root.round(102).join("started").exists());
    assert_eq!(run.review().history()[0].archive(), &archive);
    assert_ne!(run.supervisor().host().unwrap().inspect().control.decisions.get(&1), Some(&Consequence::Continue));
}

