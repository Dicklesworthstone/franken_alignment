//! Real direct children and journals. Test ballots are synthetic protocol controls.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[path = "support/restart_model.rs"]
#[allow(dead_code)]
mod model;
use fa_reference::{Error, Snapshot};
use fa_reference::action::{ActionSpec, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{EndpointOutcome, persistent::{
    FileDeliveryProfile, JournalError, JournalLimits,
    observed::{FileOversight as Host, FileOversightProfile, FileHumanReviewer,
        decoder::learned::{FileLearnedConfig, sidecar::{FileLearnedSidecar, FileLearnedSidecarFinish}},
        helpers::{processes::{FileProcessFailure, HelperRoundAdmission},
            learned_sockets::{LearnedSocketStatus, processes::{FileLearnedProcessRound, LearnedProcessLaunch}}}},
}};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, ReviewWindow,
    helper_client::{HelperClient, ClientPhase}, helper_workers::HelperLimits,
    helper_processes::{HelperProgram, HelperChildren, ProcessFailure, ProcessStage},
    human::HumanReviewPolicy, decoder_monitoring::LearnedDecoderBindingLimits,
    learned_source::LearnedSourceConfig, learned_host::sidecar::{LearnedSidecarRequest, workers::LearnedWorkerRound},
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
        let path = std::env::temp_dir().join(format!("fa-learned-process-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
    fn trace(&self) -> PathBuf { self.0.join("child") }
    fn bytes(&self) -> Vec<u8> { std::fs::read(self.store().join("delivery.bin")).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(e) = std::fs::remove_dir_all(&self.0) { eprintln!("process test cleanup: {e}"); } }
}
fn target() -> ResolvedTarget {
    ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 }
}
fn helper() -> HelperContract {
    HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: b"socket-v1".to_vec(),
        tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 7, b"approve?".to_vec()).unwrap()
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
    }, committee: CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(), helper())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 } }
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
fn setup() -> (Directory, Host, FileHumanReviewer, FrozenAction, FileLearnedSidecar) { setup_profile(profile()) }
fn setup_profile(p: FileOversightProfile) -> (Directory, Host, FileHumanReviewer, FrozenAction, FileLearnedSidecar) {
    let dir = Directory::new(); let (mut h, reviewer) = Host::create(dir.store(), p).unwrap();
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
fn program(trace: &Path, mode: &str) -> HelperProgram {
    HelperProgram::new(std::env::current_exe().unwrap(), std::env::current_dir().unwrap(),
        vec!["--exact".into(), "helper_child".into(), "--nocapture".into()],
        BTreeMap::from([(OsString::from("FA_TEST_LEARNED_CHILD"), OsString::from(mode)),
            (OsString::from("FA_TEST_LEARNED_TRACE"), trace.as_os_str().to_owned())])).unwrap()
}
fn launch(dir: &Directory, mode: &str) -> LearnedProcessLaunch {
    LearnedProcessLaunch { round: LearnedWorkerRound { round: 101, evidence_root: [9; 32],
        window: ReviewWindow { commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30) } },
        programs: BTreeMap::from([("reviewer".to_owned(), program(&dir.trace(), mode))]),
        limits: HelperLimits::default() }
}
fn eventually(mut condition: impl FnMut() -> bool) {
    let start = Instant::now();
    while !condition() {
        assert!(start.elapsed() < Duration::from_secs(15), "direct-child protocol/cleanup timeout");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn drain(round: &mut FileLearnedProcessRound) {
    eventually(|| { round.reap(); round.all_reaped() });
    assert!(round.process_statuses().values().all(|s| s.exit.is_some()));
}
fn drain_children(children: &mut HelperChildren) {
    eventually(|| { children.reap(); children.all_reaped() });
    assert!(children.statuses().values().all(|s| s.exit.is_some()));
}
fn drive(round: &mut FileLearnedProcessRound, h: &mut Host) {
    eventually(|| { round.pump(h, round.revision(), ElapsedTick(1)).unwrap(); round.ready_to_finish() });
}
fn publish(h: &mut Host, reviewer: &FileHumanReviewer, action: &FrozenAction, input: &CommitteeInput) {
    let automatic = h.authorize(h.revision(), 1, input, snapshot()).unwrap();
    let request = h.request_human_approval(h.revision(), 1001, 1, input, ElapsedTick(40)).unwrap();
    let revision = h.revision(); let human = reviewer.approve(h, revision, &request).unwrap();
    h.dispatch(h.revision(), &automatic, &human, action, input, snapshot()).unwrap();
    assert_eq!(h.publish_checked(h.revision(), 1, Some(input), snapshot(), ElapsedTick(2)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
    h.reconcile(h.revision(), 1).unwrap();
    assert_eq!(h.inspect().executions, 1); assert_eq!(h.inspect().payload, b"visible");
}

// A real separately launched executable entrypoint, not a mocked transport. The
// deliberately simple ballot controls framing/lifecycle, not detector quality.
#[test]
fn helper_child() {
    let Ok(mode) = std::env::var("FA_TEST_LEARNED_CHILD") else { return; };
    let trace = PathBuf::from(std::env::var_os("FA_TEST_LEARNED_TRACE").unwrap());
    std::fs::write(trace.with_extension("started"), b"started").unwrap();
    if mode == "exit" { return; }
    if mode == "hang" { std::thread::sleep(Duration::from_secs(20)); return; }
    let mut client = HelperClient::<UnixStream>::from_process_stdin(helper().profile_at(0)).unwrap();
    let start = Instant::now(); let mut responded = false;
    while start.elapsed() < Duration::from_secs(20) {
        if client.step().is_err() { return; }
        if client.phase() == ClientPhase::NeedsInference && !responded {
            let bytes = client.input().unwrap().actual_input().part_bytes(1).unwrap();
            assert!(bytes.starts_with(b"FASIDE\0\x01"));
            std::fs::write(trace.with_extension("packet"), bytes).unwrap();
            if mode == "gated" && !trace.with_extension("release").exists() {
                std::thread::sleep(Duration::from_millis(1)); continue;
            }
            client.respond(if mode == "abstain" { Verdict::Abstain } else { Verdict::Allow }, &[11; 32]).unwrap();
            responded = true;
        }
        if client.phase() == ClientPhase::AwaitingReveal {
            std::fs::write(trace.with_extension("committed"), b"commit sent, reveal not yet sent").unwrap();
        }
        if client.phase() == ClientPhase::ReplySent {
            std::fs::write(trace.with_extension("revealed"), b"reply sent").unwrap();
            // The supervisor must reap a child even if it does not exit after
            // its valid reply. This sleep ends early through original kill.
            if mode == "linger" { std::thread::sleep(Duration::from_secs(20)); }
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("worker did not receive original protocol completion");
}

#[test]
fn original_child_reply_reaches_two_keys_and_every_direct_child_is_reaped() {
    let (dir, mut h, reviewer, action, plan) = setup();
    let input = h.current_learned_sidecar(&plan).unwrap().clone();
    let packet = h.retained_learned_sidecar(1).unwrap().packet.payload().to_vec();
    let before = h.learned_generation_inspection().unwrap().numerical;
    let mut round = h.begin_learned_process_round(h.revision(), plan, launch(&dir, "linger"), snapshot(), || ElapsedTick(1)).unwrap();
    assert_eq!(round.connection_steps(), 0); assert!(round.progress().io.is_empty());
    assert_eq!(round.process_statuses().len(), 1);
    drive(&mut round, &mut h);
    assert_eq!(std::fs::read(dir.trace().with_extension("packet")).unwrap(), packet);
    assert!(matches!(round.finish(&mut h, round.revision(), snapshot()).unwrap(),
        FileLearnedSidecarFinish::Applied { receipt: Ok(_), .. }));
    drain(&mut round); assert!(round.process_statuses()["reviewer"].stop_requested);
    assert_eq!(h.learned_generation_inspection().unwrap().numerical, before);
    assert_eq!(h.learned_sidecar_outcome(101).unwrap().result.archive(), round.outcome().unwrap().archive());
    publish(&mut h, &reviewer, &action, &input);
    assert!(h.commit_review(h.revision(), 101, "reviewer", fa_reference::round::commitment(101,
        "reviewer", &[9; 32], Verdict::Allow, b"manual").unwrap()).is_err());
}

#[test]
fn preflight_calls_do_not_touch_children_and_source_loss_terminates_without_sending() {
    let (dir, mut h, _, _, plan) = setup(); let (_other_dir, mut other, _, _, _) = setup();
    let mut round = h.begin_learned_process_round(h.revision(), plan, launch(&dir, "hang"), snapshot(), || ElapsedTick(1)).unwrap();
    let children = round.process_statuses(); let revision = h.revision();
    assert_eq!(round.pump(&mut other, 0, ElapsedTick(1)).unwrap_err().error, JournalError::Contract(Error::Binding));
    assert_eq!(round.pump(&mut h, 1, ElapsedTick(1)).unwrap_err().error, JournalError::Contract(Error::Stale));
    assert_eq!(round.finish(&mut h, 0, snapshot()).err(), Some(JournalError::Contract(Error::Incomplete)));
    // finish may inspect process liveness but cannot terminate a pending worker.
    assert_eq!(round.process_statuses()["reviewer"].pid, children["reviewer"].pid);
    assert!(!round.process_statuses()["reviewer"].stop_requested);
    assert_eq!(h.revision(), revision); assert_eq!(round.connection_steps(), 0);
    step(&mut h);
    assert!(round.pump(&mut h, 0, ElapsedTick(1)).is_err());
    assert_eq!(round.status(), LearnedSocketStatus::Failed);
    assert_eq!(round.connection_steps(), 0); assert!(!dir.trace().with_extension("packet").exists());
    drain(&mut round); assert!(round.outcome().is_none());
}

#[test]
fn post_launch_expiry_returns_cleanup_and_keeps_the_original_round_consumed() {
    let (dir, mut h, _, _, plan) = setup(); let mut calls = 0;
    let mut error = h.begin_learned_process_round(h.revision(), plan, launch(&dir, "hang"), snapshot(), || {
        calls += 1; ElapsedTick(if calls == 1 { 1 } else { 20 })
    }).unwrap_err();
    assert_eq!(error.admission, HelperRoundAdmission::Committed);
    assert_eq!(error.failure, FileProcessFailure::Journal(JournalError::Contract(Error::Stale)));
    let children = error.children.as_mut().unwrap(); assert_eq!(children.statuses().len(), 1);
    assert!(children.statuses()["reviewer"].stop_requested); drain_children(children);
    assert!(!dir.trace().with_extension("packet").exists());
    assert!(h.begin_review(h.revision(), 1, 101, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(40), reveal_by: ElapsedTick(50) }, snapshot()).is_err());
    assert!(h.commit_review(h.revision(), 101, "reviewer", fa_reference::round::commitment(101,
        "reviewer", &[9; 32], Verdict::Allow, b"manual").unwrap()).is_err());
}

#[test]
fn failed_begin_storage_starts_no_children_and_reports_ambiguous_admission() {
    let (dir, mut h, _, _, plan) = setup(); let before = dir.bytes();
    std::fs::write(dir.store().join("delivery.pending"), b"occupied stage").unwrap();
    let error = h.begin_learned_process_round(h.revision(), plan, launch(&dir, "allow"), snapshot(), || ElapsedTick(1)).unwrap_err();
    assert_eq!(error.admission, HelperRoundAdmission::Unknown); assert!(error.children.is_none());
    assert!(h.storage_failure().is_some()); assert_eq!(dir.bytes(), before);
    assert!(!dir.trace().with_extension("started").exists());
}

#[test]
fn partial_spawn_returns_every_started_child_without_a_reduced_roster() {
    let mut p = profile();
    p.committee = CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(), helper()), ("z-last".to_owned(), helper())])).unwrap();
    p.delivery.congress.members.insert("z-last".to_owned(), MemberPolicy { cohort: "second".to_owned(), weight: 1 });
    let (dir, mut h, _, _, plan) = setup_profile(p);
    let bad = dir.0.join("not-executable"); std::fs::write(&bad, b"not a program").unwrap();
    let mut spec = launch(&dir, "hang");
    spec.programs.insert("z-last".to_owned(), HelperProgram::new(bad, dir.0.clone(), Vec::new(), BTreeMap::new()).unwrap());
    let mut error = h.begin_learned_process_round(h.revision(), plan, spec, snapshot(), || ElapsedTick(1)).unwrap_err();
    assert_eq!(error.admission, HelperRoundAdmission::Committed);
    assert!(matches!(&error.failure, FileProcessFailure::Launch { member: Some(m),
        failure: ProcessFailure::Io { stage: ProcessStage::Spawn, .. } } if m == "z-last"));
    let children = error.children.as_mut().unwrap();
    assert_eq!(children.statuses().keys().map(String::as_str).collect::<Vec<_>>(), vec!["reviewer"]);
    drain_children(children); assert!(!dir.trace().with_extension("packet").exists());
    assert!(h.learned_sidecar_outcome(101).is_err()); assert_eq!(h.inspect().executions, 0);
}

#[test]
fn a_successful_exit_is_missing_evidence_not_an_allow_vote() {
    let (dir, mut h, _, _, plan) = setup();
    let input = h.current_learned_sidecar(&plan).unwrap().clone();
    let mut round = h.begin_learned_process_round(h.revision(), plan, launch(&dir, "exit"), snapshot(), || ElapsedTick(1)).unwrap();
    eventually(|| { round.reap(); round.all_reaped() });
    assert!(round.process_statuses()["reviewer"].exit.unwrap().success);
    assert!(!round.ready_to_finish()); assert!(!round.progress().workers["reviewer"].revealed);
    round.pump(&mut h, round.revision(), ElapsedTick(30)).unwrap();
    round.finish(&mut h, round.revision(), snapshot()).unwrap();
    assert!(!round.progress().workers["reviewer"].revealed);
    assert!(h.authorize(h.revision(), 1, &input, snapshot()).is_err());
    assert_eq!(h.inspect().executions, 0); drain(&mut round);
}

#[test]
fn a_caught_clock_unwind_closes_transport_and_retains_reapable_children() {
    let (dir, mut h, _, _, plan) = setup();
    let mut round = h.begin_learned_process_round(h.revision(), plan, launch(&dir, "hang"), snapshot(), || ElapsedTick(1)).unwrap();
    let revision = round.revision();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = round.pump_with_clock(&mut h, revision, || panic!("injected clock unwind"));
    })).is_err());
    assert_eq!(round.status(), LearnedSocketStatus::Failed);
    assert!(round.process_statuses()["reviewer"].stop_requested); drain(&mut round);
    assert!(round.pump(&mut h, round.revision(), ElapsedTick(1)).is_err());
    assert!(round.outcome().is_none());
}

#[test]
fn actual_commit_storage_failure_terminates_children_without_a_reveal() {
    let (dir, mut h, _, _, plan) = setup();
    let mut round = h.begin_learned_process_round(h.revision(), plan, launch(&dir, "gated"), snapshot(), || ElapsedTick(1)).unwrap();
    eventually(|| { round.pump(&mut h, round.revision(), ElapsedTick(1)).unwrap(); dir.trace().with_extension("packet").exists() });
    let before = dir.bytes(); let revision = h.revision();
    std::fs::write(dir.store().join("delivery.pending"), b"occupied stage").unwrap();
    std::fs::write(dir.trace().with_extension("release"), b"send commitment").unwrap();
    eventually(|| dir.trace().with_extension("committed").exists());
    eventually(|| round.pump(&mut h, round.revision(), ElapsedTick(1)).is_err());
    assert_eq!(round.status(), LearnedSocketStatus::Failed); assert_eq!(h.revision(), revision);
    assert_eq!(dir.bytes(), before); assert!(h.storage_failure().is_some());
    assert!(!round.progress().workers["reviewer"].revealed); assert!(!dir.trace().with_extension("revealed").exists());
    assert!(!round.progress().io.is_empty()); drain(&mut round);
    assert!(round.outcome().is_none());
}
