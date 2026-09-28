//! Real direct children, original wire/journal/numerics; ballots are synthetic.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[path = "support/restart_model.rs"]
#[allow(dead_code)]
mod model;
use fa_reference::Snapshot;
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{persistent::{
    FileDeliveryProfile, JournalLimits,
    observed::{FileOversight as Host, FileOversightProfile, FileHumanReviewer,
        decoder::learned::{FileLearnedConfig, sidecar::{FileLearnedSidecar, FileLearnedSidecarFinish}},
        helpers::{processes::{FileProcessFailure, HelperRoundAdmission},
            learned_sockets::{processes::{FileLearnedProcessRound, LearnedProcessLaunch}}}},
}};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::{CommitteeContract, HelperContract, ReviewWindow,
    helper_processes::{HelperChildren, HelperProgram, ProcessFailure, ProcessStage},
    helper_client::{HelperClient, ClientPhase}, helper_workers::HelperLimits,
    human::HumanReviewPolicy, decoder_monitoring::LearnedDecoderBindingLimits,
    learned_source::LearnedSourceConfig, learned_host::sidecar::LearnedSidecarRequest,
    sidecar::{SidecarCongressBudget, SidecarIdentity}};
use fa_reference::action::consequence::activation::probe::learned::{KvGroup, KvRow};
use fa_reference::action::consequence::activation::tensor::kv::{experiment::KvSide,
    decoder::sampling::{SamplingPolicy, SamplingStart,
        monitored::{GenerationBudget, GenerationSpec, GenerationTelemetryBudget}}};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-learned-processes-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("process test cleanup: {error}"); } }
}
fn target() -> ResolvedTarget {
    ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 }
}
fn profile() -> FileOversightProfile {
    FileOversightProfile { delivery: FileDeliveryProfile {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: 100, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
            model_generation: 3, tokenizer_generation: 4, state_schema_generation: 1,
            grade: RestartGrade::AuditOnly }, Vec::new(), vec![0], vec![0], 0).unwrap(),
        suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::PayloadAtMost(1024)]).unwrap(),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".to_owned(),
            MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: vec![target()], target: target(), initial_payload: b"initial".to_vec(),
        retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
    }, committee: CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"process-v1".to_vec(),
            tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 7, b"approve?".to_vec(),
    ).unwrap())])).unwrap(), human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 } }
}
fn config() -> FileLearnedConfig {
    let model = model::model(); let policy = model::policy(&model, 0, 1);
    FileLearnedConfig::new(model, LearnedSourceConfig { stream: 21, evaluation_origin: 201, monitor_generation: 31,
        spec: GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
            policy: SamplingPolicy::new(1, 1, 3, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173,
        }).unwrap(), policy, budget: GenerationBudget::default(), telemetry: GenerationTelemetryBudget::default() },
        LearnedDecoderBindingLimits::default()).unwrap().with_required_sidecar().unwrap()
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn step(h: &mut Host) {
    let n = h.learned_generation_inspection().unwrap().numerical;
    h.advance_learned_generation(h.revision(), n.actor_revision, n.position).unwrap().unwrap();
}
fn setup() -> (Directory, Host, FileHumanReviewer, FrozenAction, FileLearnedSidecar) {
    setup_with_profile(profile())
}
fn setup_with_profile(selected_profile: FileOversightProfile) -> (Directory, Host, FileHumanReviewer, FrozenAction, FileLearnedSidecar) {
    let dir = Directory::new(); let (mut h, reviewer) = Host::create(dir.store(), selected_profile).unwrap();
    h.enable_learned_generation(h.revision(), config()).unwrap();
    h.observe_time(h.revision(), ElapsedTick(1)).unwrap(); step(&mut h); step(&mut h);
    let action = h.propose(h.revision(), 1, ActionSpec { version: VERSION, scope: profile().delivery.scope,
        target: Some(target()), payload: b"visible".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: h.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 }, snapshot()).unwrap();
    let n = h.learned_generation_inspection().unwrap().numerical;
    let sidecar = h.begin_learned_sidecar_plan(h.revision(), 1, n.actor_revision, LearnedSidecarRequest {
        identity: SidecarIdentity { object_id: 1001, generation: 1, transform_id: 7 },
        priority: vec![KvGroup { row: KvRow { layer: 1, side: KvSide::Value, position: 1 }, head: 0 }],
        budget: SidecarCongressBudget::default(),
    }).unwrap();
    (dir, h, reviewer, action, sidecar)
}
fn program(root: &Path, mode: &str) -> HelperProgram {
    HelperProgram::new(std::env::current_exe().unwrap(), root.to_path_buf(),
        vec![OsString::from("--exact"), OsString::from("process_worker"), OsString::from("--nocapture")],
        BTreeMap::from([(OsString::from("FA_PROCESS_TEST"), OsString::from(mode))])).unwrap()
}
fn launch(root: &Path, mode: &str) -> LearnedProcessLaunch {
    LearnedProcessLaunch { round: LearnedWorkerRound { round: 101, evidence_root: [9; 32],
        window: ReviewWindow { commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30) } },
        programs: BTreeMap::from([("reviewer".to_owned(), program(root, mode))]), limits: HelperLimits::default() }
}
fn pause() { std::thread::sleep(Duration::from_millis(1)); }
fn reaped(children: &mut HelperChildren) {
    let started = Instant::now();
    while !children.all_reaped() && started.elapsed() < Duration::from_secs(10) { children.reap(); pause(); }
    assert!(children.all_reaped(), "direct child cleanup not confirmed: {:?}", children.statuses());
}
fn cleanup(run: &mut FileLearnedProcessRound) {
    let started = Instant::now();
    while !run.all_reaped() && started.elapsed() < Duration::from_secs(10) { run.reap(); pause(); }
    assert!(run.all_reaped(), "direct child cleanup not confirmed: {:?}", run.process_statuses());
}
fn drive(run: &mut FileLearnedProcessRound, host: &mut Host) {
    let started = Instant::now();
    while !run.ready_to_finish() && started.elapsed() < Duration::from_secs(10) {
        run.pump(host, run.revision(), ElapsedTick(1)).unwrap(); pause();
    }
    assert!(run.ready_to_finish(), "original child protocol did not complete");
}

/// This test is also the executable child entrypoint. In the parent test run it
/// does nothing; only explicit launch configuration selects a synthetic ballot.
#[test]
fn process_worker() {
    let Ok(mode) = std::env::var("FA_PROCESS_TEST") else { return; };
    std::fs::write("started", mode.as_bytes()).unwrap();
    if mode == "exit" { return; }
    let expected = profile().committee.members()["reviewer"].profile_at(0);
    let mut client = HelperClient::<UnixStream>::from_process_stdin(expected).unwrap();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if client.step().is_err() { return; } // actual parent closure, no substitute vote
        if client.phase() == ClientPhase::NeedsInference {
            let input = client.input().unwrap().actual_input();
            let payload = input.part_bytes(1).unwrap();
            assert_eq!(&payload[..8], b"FASIDE\0\x01");
            std::fs::write("observed", payload).unwrap();
            if mode == "wait" { pause(); continue; }
            let verdict = if mode == "abstain" { Verdict::Abstain } else { Verdict::Allow };
            client.respond(verdict, &[11; 32]).unwrap();
        }
        if client.phase() == ClientPhase::ReplySent { return; }
        pause();
    }
    panic!("child exceeded its test-only protocol deadline");
}

use fa_reference::action::consequence::delivery::persistent::observed::helpers::learned_sockets::processes::sequence::{
    FileLearnedProcessReview, LearnedProcessLimits, LearnedProcessStatus,
};
use fa_reference::action::consequence::oversight::learned_host::sidecar::workers::{LearnedWorkerRound, LearnedWorkerSchedule};

fn schedule(polls: usize) -> LearnedWorkerSchedule {
    LearnedWorkerSchedule { rounds: vec![
        LearnedWorkerRound { round: 101, evidence_root: [9; 32], window: ReviewWindow {
            commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30) } },
        LearnedWorkerRound { round: 102, evidence_root: [10; 32], window: ReviewWindow {
            commit_by: ElapsedTick(40), reveal_by: ElapsedTick(50) } },
    ], helpers: HelperLimits::default(), polls }
}
fn programs(root: &Directory) -> BTreeMap<u64, BTreeMap<String, HelperProgram>> {
    let first = root.0.join("first"); let next = root.0.join("next");
    std::fs::create_dir(&first).unwrap(); std::fs::create_dir(&next).unwrap();
    BTreeMap::from([
        (101, BTreeMap::from([("reviewer".into(), program(&first, "abstain"))])),
        (102, BTreeMap::from([("reviewer".into(), program(&next, "allow"))])),
    ])
}
fn begin(root: &Directory, h: &mut Host, plan: FileLearnedSidecar, polls: usize) -> FileLearnedProcessReview {
    h.begin_learned_process_review(h.revision(), plan, schedule(polls), programs(root),
        LearnedProcessLimits { starts: 2 }, snapshot(), || ElapsedTick(1)).unwrap()
}
fn to_drain(run: &mut FileLearnedProcessReview, h: &mut Host) {
    let start = Instant::now();
    while run.status() == LearnedProcessStatus::Running && start.elapsed() < Duration::from_secs(10) {
        run.advance(h, run.revision(), ElapsedTick(1), snapshot()).unwrap(); pause();
    }
    assert_eq!(run.status(), LearnedProcessStatus::Draining);
    assert_eq!(run.history().len(), 1);
    assert!(matches!(&run.history()[0], FileLearnedSidecarFinish::Refined { .. }));
}
fn cleanup_review(run: &mut FileLearnedProcessReview) {
    let start = Instant::now();
    while !run.all_reaped() && start.elapsed() < Duration::from_secs(10) { run.reap(); pause(); }
    assert!(run.all_reaped(), "cleanup not confirmed: {:?}", run.records());
}
fn to_successor(run: &mut FileLearnedProcessReview, h: &mut Host) {
    let start = Instant::now();
    while run.status() == LearnedProcessStatus::Draining && start.elapsed() < Duration::from_secs(10) {
        run.advance(h, run.revision(), ElapsedTick(1), snapshot()).unwrap(); pause();
    }
    assert_eq!(run.status(), LearnedProcessStatus::Running); assert_eq!(run.round(), 102);
}

#[test]
fn original_refinement_is_followed_by_reaping_then_a_fresh_child_and_two_key_publication() {
    let (root, mut h, reviewer, action, plan) = setup();
    let original = h.learned_generation_inspection().unwrap().numerical;
    let mut run = begin(&root, &mut h, plan, 4096);
    assert_eq!(run.admitted_starts(), 2);
    assert_eq!(run.records()[&102].admission, None);
    assert!(!root.0.join("next/started").exists());
    to_drain(&mut run, &mut h);
    assert!(!root.0.join("next/started").exists());
    assert!(run.records()[&101].socket.completed);
    assert_eq!(h.learned_sidecar_outcome(101).unwrap().result.archive(), run.history()[0].archive());
    to_successor(&mut run, &mut h);
    assert!(run.records()[&101].processes["reviewer"].exit.is_some());
    assert_eq!(run.records()[&102].admission, Some(HelperRoundAdmission::Committed));
    assert_eq!(run.records()[&102].socket.connection_steps, 0);
    assert!(!root.0.join("next/observed").exists());
    let start = Instant::now();
    while run.status() == LearnedProcessStatus::Running && start.elapsed() < Duration::from_secs(10) {
        run.advance(&mut h, run.revision(), ElapsedTick(1), snapshot()).unwrap(); pause();
    }
    assert_eq!(run.status(), LearnedProcessStatus::Finished); cleanup_review(&mut run);
    assert_eq!(run.history().len(), 2);
    assert!(matches!(&run.history()[1], FileLearnedSidecarFinish::Applied { receipt: Ok(_), .. }));
    assert_ne!(std::fs::read(root.0.join("first/observed")).unwrap(), std::fs::read(root.0.join("next/observed")).unwrap());
    assert_eq!(h.learned_generation_inspection().unwrap().numerical, original);
    assert_eq!(h.learned_sidecar_outcome(102).unwrap().result.archive(), run.history()[1].archive());
    let input = run.input().clone();
    let automatic = h.authorize(h.revision(), 1, &input, snapshot()).unwrap();
    let request = h.request_human_approval(h.revision(), 1001, 1, &input, ElapsedTick(40)).unwrap();
    let revision = h.revision(); let human = reviewer.approve(&mut h, revision, &request).unwrap();
    h.dispatch(h.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
    h.publish_checked(h.revision(), 1, Some(&input), snapshot(), ElapsedTick(2)).unwrap();
    h.reconcile(h.revision(), 1).unwrap();
    assert_eq!(h.inspect().executions, 1); assert_eq!(h.inspect().control.ledger.charged, 16);
}

#[test]
fn cancellation_and_exhausted_polls_stop_only_launched_children_and_cleanup_stays_available() {
    for cancel in [false, true] {
        let (root, mut h, _, _, plan) = setup();
        let mut run = begin(&root, &mut h, plan, if cancel { 4096 } else { 1 });
        if cancel { run.cancel(0).unwrap(); }
        else {
            run.advance(&mut h, 0, ElapsedTick(1), snapshot()).unwrap();
            assert!(run.advance(&mut h, run.revision(), ElapsedTick(1), snapshot()).is_err());
        }
        cleanup_review(&mut run);
        assert_eq!(run.status(), if cancel { LearnedProcessStatus::Cancelled } else { LearnedProcessStatus::Failed });
        assert_eq!(run.records()[&102].admission, None); assert!(run.history().is_empty());
        assert!(run.records()[&101].processes["reviewer"].stop_requested);
        assert!(!root.0.join("next/started").exists()); assert_eq!(h.inspect().executions, 0);
        assert_eq!(h.inspect().control.ledger.stages[&1], ActionState::Reviewing);
    }
}

#[test]
fn late_source_or_window_loss_preserves_purchased_refinement_without_starting_the_successor() {
    for source_changed in [false, true] {
        let (root, mut h, _, _, plan) = setup(); let mut run = begin(&root, &mut h, plan, 4096);
        to_drain(&mut run, &mut h);
        let original = run.history()[0].archive().clone();
        if source_changed { step(&mut h); }
        let now = if source_changed { ElapsedTick(1) } else { ElapsedTick(40) };
        assert!(run.advance(&mut h, run.revision(), now, snapshot()).is_err());
        cleanup_review(&mut run);
        assert_eq!(run.status(), LearnedProcessStatus::Failed);
        assert_eq!(run.history()[0].archive(), &original);
        assert_eq!(h.learned_sidecar_outcome(101).unwrap().result.archive(), &original);
        assert_eq!(run.records()[&102].admission, None);
        assert!(!root.0.join("next/started").exists());
        assert!(h.authorize(h.revision(), 1, run.input(), snapshot()).is_err());
    }
}

#[test]
fn actual_successor_spawn_failure_retains_committed_admission_and_the_prior_outcome() {
    use std::os::unix::fs::PermissionsExt;
    let (root, mut h, _, _, plan) = setup(); let mut chosen = programs(&root);
    let bad = root.0.join("not-executable"); std::fs::write(&bad, b"not executable").unwrap();
    std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o600)).unwrap();
    chosen.get_mut(&102).unwrap().insert("reviewer".into(),
        HelperProgram::new(bad, root.0.join("next"), Vec::new(), BTreeMap::new()).unwrap());
    let mut run = h.begin_learned_process_review(h.revision(), plan, schedule(4096), chosen,
        LearnedProcessLimits { starts: 2 }, snapshot(), || ElapsedTick(1)).unwrap();
    to_drain(&mut run, &mut h);
    let start = Instant::now();
    while run.status() == LearnedProcessStatus::Draining && start.elapsed() < Duration::from_secs(10) {
        let _ = run.advance(&mut h, run.revision(), ElapsedTick(1), snapshot()); pause();
    }
    assert_eq!(run.status(), LearnedProcessStatus::Failed); cleanup_review(&mut run);
    assert_eq!(run.records()[&102].admission, Some(HelperRoundAdmission::Committed));
    assert!(matches!(run.failure(), Some(FileProcessFailure::Launch { failure: ProcessFailure::Io { stage: ProcessStage::Spawn, .. }, .. })));
    assert_eq!(run.history().len(), 1); assert!(h.learned_sidecar_outcome(101).is_ok());
    assert!(!root.0.join("next/observed").exists());
    assert!(h.authorize(h.revision(), 1, run.input(), snapshot()).is_err());
}

#[test]
fn schedule_roster_and_exact_start_allowance_are_admitted_before_any_spawn() {
    for defect in 0..3 {
        let (root, mut h, _, _, plan) = setup(); let before = h.revision(); let mut chosen = programs(&root);
        if defect == 1 { chosen.remove(&102); }
        if defect == 2 { chosen.get_mut(&102).unwrap().clear(); }
        let limit = if defect == 0 { 1 } else { 2 };
        let error = h.begin_learned_process_review(before, plan, schedule(4096), chosen,
            LearnedProcessLimits { starts: limit }, snapshot(), || ElapsedTick(1)).unwrap_err();
        assert_eq!(error.admission, HelperRoundAdmission::NotStarted); assert!(error.children.is_none());
        assert_eq!(h.revision(), before);
        assert!(!root.0.join("first/started").exists()); assert!(!root.0.join("next/started").exists());
    }
    // The exact two-start control proceeds through the real first child.
    let (root, mut h, _, _, plan) = setup(); let mut run = begin(&root, &mut h, plan, 4096);
    to_drain(&mut run, &mut h); run.cancel(run.revision()).unwrap(); cleanup_review(&mut run);
}

#[test]
fn stale_foreign_and_clock_unwind_never_spawn_a_replacement_roster() {
    let (root, mut h, _, _, plan) = setup(); let (_other_root, mut other, _, _, _) = setup();
    let mut run = begin(&root, &mut h, plan, 4096);
    assert!(run.advance(&mut other, 0, ElapsedTick(1), snapshot()).is_err());
    assert!(run.advance(&mut h, 1, ElapsedTick(1), snapshot()).is_err());
    assert!(run.advance(&mut h, 0, ElapsedTick(0), snapshot()).is_err());
    assert_eq!(run.revision(), 0); assert_eq!(run.polls(), 0);
    assert!(!run.records()[&101].processes["reviewer"].stop_requested);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = run.advance_with_clock(&mut h, 0, || panic!("clock interruption"), snapshot());
    })).is_err());
    assert_eq!(run.status(), LearnedProcessStatus::Failed);
    cleanup_review(&mut run); assert!(!root.0.join("next/started").exists());
    assert!(run.advance(&mut h, run.revision(), ElapsedTick(1), snapshot()).is_err());
}

#[test]
fn partial_multimember_launch_returns_the_actual_first_child_for_reaping() {
    use std::os::unix::fs::PermissionsExt;
    for partial_failure in [false, true] {
        let mut p = profile();
        let member = p.delivery.congress.members["reviewer"].clone();
        let contract = p.committee.members()["reviewer"].clone();
        p.delivery.congress.members = BTreeMap::from([("a".into(), member.clone()), ("b".into(), member)]);
        p.delivery.congress.minimum_members = 2;
        p.committee = CommitteeContract::new(BTreeMap::from([("a".into(), contract.clone()), ("b".into(), contract)])).unwrap();
        let (root, mut h, _, _, plan) = setup_with_profile(p);
        let good = program(&root.0, "allow");
        let bad = root.0.join("not-executable"); std::fs::write(&bad, b"invalid program").unwrap();
        std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o600)).unwrap();
        let second = if partial_failure {
            HelperProgram::new(bad, root.0.clone(), Vec::new(), BTreeMap::new()).unwrap()
        } else { good.clone() };
        let mut chosen = launch(&root.0, "allow"); chosen.programs = BTreeMap::from([("a".into(), good), ("b".into(), second)]);
        match h.begin_learned_process_round(h.revision(), plan, chosen, snapshot(), || ElapsedTick(1)) {
            Ok(mut run) => {
                assert!(!partial_failure); drive(&mut run, &mut h);
                assert!(matches!(run.finish(&mut h, run.revision(), snapshot()).unwrap(), FileLearnedSidecarFinish::Applied { receipt: Ok(_), .. }));
                cleanup(&mut run); assert_eq!(run.process_statuses().len(), 2);
            }
            Err(mut failure) => {
                assert!(partial_failure); assert_eq!(failure.admission, HelperRoundAdmission::Committed);
                let children = failure.children.as_mut().unwrap();
                assert_eq!(children.statuses().keys().map(String::as_str).collect::<Vec<_>>(), vec!["a"]);
                assert!(children.statuses()["a"].stop_requested); reaped(children);
                assert!(!root.0.join("observed").exists()); assert_eq!(h.inspect().executions, 0);
            }
        }
    }
}
