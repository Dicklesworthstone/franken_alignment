//! Real child protocols and original learned custody; synthetic ballots test
//! process/authority composition, not detector quality or production readiness.

use super::*;
use crate::action::FrozenAction;
use crate::action::consequence::delivery::persistent::observed::{
    FileHumanReviewer, FileOversightProfile,
    decoder::learned::FileLearnedConfig,
    helpers::learned::{FileLearnedProbeReview, FileLearnedProbeStatus},
};
use crate::action::consequence::oversight::{
    ReviewWindow,
    helper_client::{ClientPhase, HelperClient},
    helper_workers::HelperLimits,
    learned_host::sidecar::workers::{
        LearnedWorkerSchedule,
        probes::{ProbeReviewLimits, ProbeReviewMember},
    },
};
use crate::round::Verdict;
use std::ffi::OsString;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[path = "../../../../learned/tests/fixture.rs"]
mod fixture;
use fixture::{Directory, config, owner, profile, propose, snapshot, step};

fn path(root: &Directory, round: u64, member: &str) -> PathBuf {
    root.store().parent().unwrap().join(format!("{round}-{member}"))
}

fn program(directory: &Path, mode: &str) -> HelperProgram {
    let (_, module) = module_path!().split_once("::").unwrap();
    HelperProgram::new(
        std::env::current_exe().unwrap(),
        directory.to_path_buf(),
        vec![
            OsString::from("--exact"),
            OsString::from(format!("{module}::process_worker")),
            OsString::from("--nocapture"),
        ],
        BTreeMap::from([(
            OsString::from("FA_HELD_PROCESS_TEST"),
            OsString::from(mode),
        )]),
    )
    .unwrap()
}

#[test]
fn process_worker() {
    let Ok(mode) = std::env::var("FA_HELD_PROCESS_TEST") else {
        return;
    };
    std::fs::write("started", std::process::id().to_string()).unwrap();
    let expected = profile().committee.members()["alpha"].profile_at(0);
    let mut client = HelperClient::<UnixStream>::from_process_stdin(expected).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if client.step().is_err() {
            return;
        }
        if client.phase() == ClientPhase::NeedsInference {
            let input = client.input().unwrap().actual_input();
            let payload = input.part_bytes(1).unwrap();
            assert_eq!(&payload[..8], b"FASIDE\0\x01");
            std::fs::write("observed", payload).unwrap();
            client
                .respond(if mode == "hold" { Verdict::Hold } else { Verdict::Allow }, &[11; 32])
                .unwrap();
        }
        if client.phase() == ClientPhase::ReplySent {
            return;
        }
        assert!(Instant::now() < deadline, "child exceeded its protocol fixture limit");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn setup(first_holds: bool) -> (Directory, FileOversight, FileLearnedProcessReview) {
    let root = Directory::new();
    let (mut host, _) = owner(&root, &config());
    step(&mut host);
    step(&mut host);
    let (_, sidecar) = propose(&mut host);
    let rounds: Vec<_> = [101, 102]
        .into_iter()
        .enumerate()
        .map(|(index, round)| LearnedWorkerRound {
            round,
            evidence_root: [9 + index as u8; 32],
            window: ReviewWindow {
                commit_by: ElapsedTick(20 + index as u64 * 20),
                reveal_by: ElapsedTick(30 + index as u64 * 20),
            },
        })
        .collect();
    let mut programs = BTreeMap::new();
    for round in [101, 102] {
        let mut members = BTreeMap::new();
        for member in ["alpha", "beta"] {
            let directory = path(&root, round, member);
            std::fs::create_dir(&directory).unwrap();
            // One Hold falls below the original narrowing threshold but
            // prevents Continue. Both later original programs return Allow.
            let mode = if first_holds && round == 101 && member == "alpha" {
                "hold"
            } else {
                "allow"
            };
            members.insert(member.into(), program(&directory, mode));
        }
        programs.insert(round, members);
    }
    let run = host
        .begin_learned_process_review(
            host.revision(),
            sidecar,
            LearnedWorkerSchedule {
                rounds,
                helpers: HelperLimits::default(),
                polls: 4096,
            },
            programs,
            LearnedProcessLimits { starts: 4 },
            snapshot(),
            || ElapsedTick(1),
        )
        .unwrap();
    (root, host, run)
}

fn until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline, "original process fixture did not settle");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn finish(host: &mut FileOversight, run: &mut FileLearnedProcessReview) {
    until(|| {
        if run.status() != LearnedProcessStatus::Running {
            return true;
        }
        run.advance(host, run.revision(), ElapsedTick(1), snapshot()).unwrap();
        false
    });
    assert_eq!(run.status(), LearnedProcessStatus::Finished);
    until(|| {
        run.reap();
        run.all_reaped()
    });
}

fn assert_unstarted(root: &Directory, run: &FileLearnedProcessReview) {
    assert_eq!(run.records()[&102].admission, None);
    for member in ["alpha", "beta"] {
        assert!(!path(root, 102, member).join("started").exists());
    }
}

#[test]
fn held_fixed_process_successor_preserves_original_input_work_and_effect_keys() {
    let (root, mut host, mut run) = setup(true);
    finish(&mut host, &mut run);
    assert_eq!(host.inspect().control.decisions[&1], Consequence::HoldEffect);
    assert_unstarted(&root, &run);
    let original_input = run.input().clone();
    let generation = host.learned_generation_inspection().unwrap().numerical.actor_revision;
    let polls = run.polls();
    assert_eq!(run.admitted_starts(), 4);
    assert_eq!(run.check_held_successor(&host, ElapsedTick(1)).unwrap().round, 102);

    let revision = run.revision();
    assert_eq!(
        run.convene_held_with_clock(&mut host, revision, snapshot(), || ElapsedTick(1)),
        Ok(LearnedProcessStatus::Running)
    );
    assert_eq!(run.polls(), polls + 1);
    assert_eq!(run.admitted_starts(), 4);
    assert_eq!(run.history().len(), 1);
    assert_eq!(run.input(), &original_input);
    assert_eq!(run.records()[&102].admission, Some(HelperRoundAdmission::Committed));
    for status in run.records()[&101].processes.values() {
        assert!(status.exit.is_some());
    }
    finish(&mut host, &mut run);
    assert_eq!(run.history().len(), 2);
    assert_eq!(host.inspect().control.decisions[&1], Consequence::Continue);
    assert_eq!(run.input(), &original_input);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.actor_revision, generation);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.available, 100);
    for member in ["alpha", "beta"] {
        assert_eq!(
            std::fs::read(path(&root, 101, member).join("observed")).unwrap(),
            std::fs::read(path(&root, 102, member).join("observed")).unwrap()
        );
    }
    assert!(!run.has_held_successor_custody());
}

#[test]
fn continue_and_stale_source_cannot_reuse_a_finished_process_slot() {
    for held in [false, true] {
        let (root, mut host, mut run) = setup(held);
        finish(&mut host, &mut run);
        if held {
            step(&mut host); // Actual source evolution invalidates the frozen input.
        } else {
            assert_eq!(host.inspect().control.decisions[&1], Consequence::Continue);
        }
        let polls = run.polls();
        let revision = run.revision();
        assert!(run.convene_held_with_clock(&mut host, revision, snapshot(),
            || panic!("ineligible completed review must not acquire a clock")).is_err());
        assert_unstarted(&root, &run);
        assert_eq!(run.polls(), polls);
        assert_eq!(host.inspect().executions, 0);
        if held {
            assert!(run.has_held_successor_custody());
            run.cancel(run.revision()).unwrap();
            assert_eq!(run.status(), LearnedProcessStatus::Cancelled);
            assert!(run.programs.is_empty());
        }
    }
}

#[test]
fn fixed_window_exhausted_polls_and_unknown_startup_never_refresh_old_custody() {
    for case in 0..3 {
        let (root, mut host, mut run) = setup(true);
        finish(&mut host, &mut run);
        match case {
            0 => { host.observe_time(host.revision(), ElapsedTick(40)).unwrap(); }
            // Private causal injections model exhaustion and lost startup
            // custody after the real positive Hold protocol above.
            1 => run.polls = run.schedule.polls,
            2 => run.unresolved_startup = Some(102),
            _ => unreachable!(),
        }
        let revision = run.revision();
        assert!(run.convene_held_with_clock(&mut host, revision, snapshot(),
            || panic!("ineligible fixed slot must not acquire a clock")).is_err());
        assert_unstarted(&root, &run);
        assert!(run.has_held_successor_custody());
        run.cancel(run.revision()).unwrap();
        assert_eq!(run.status(), LearnedProcessStatus::Cancelled);
        assert!(run.programs.is_empty());
        assert_eq!(run.unresolved_startup(), if case == 2 { Some(102) } else { None });
        assert_eq!(run.all_reaped(), case != 2);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn held_successor_post_launch_clock_failure_retains_real_children_and_consumed_round() {
    let (root, mut host, mut run) = setup(true);
    finish(&mut host, &mut run);
    let polls = run.polls();
    let mut calls = 0;
    let revision = run.revision();
    let result = run.convene_held_with_clock(&mut host, revision, snapshot(), || {
        calls += 1;
        if calls == 3 {
            panic!("successor post-launch clock interrupted");
        }
        ElapsedTick(1)
    });
    assert_eq!(result, Err(FileProcessFailure::PostLaunchClockPanicked));
    assert_eq!(calls, 3);
    assert_eq!(run.status(), LearnedProcessStatus::Failed);
    assert_eq!(run.polls(), polls + 1);
    assert_eq!(run.admitted_starts(), 4);
    assert_eq!(run.history().len(), 1);
    assert_eq!(run.records()[&102].admission, Some(HelperRoundAdmission::Committed));
    assert_eq!(run.records()[&102].processes.len(), 2);
    assert!(run.programs.is_empty());
    until(|| {
        run.reap();
        run.all_reaped()
    });
    for member in ["alpha", "beta"] {
        assert!(run.records()[&102].processes[member].exit.is_some());
        assert!(!path(&root, 102, member).join("observed").exists());
    }
    assert!(host.worker_rounds.contains(&102));
    assert!(!run.has_held_successor_custody());
    assert_eq!(host.inspect().executions, 0);
}
