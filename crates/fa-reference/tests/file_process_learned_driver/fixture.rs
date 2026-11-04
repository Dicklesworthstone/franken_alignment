//! Public-API fixture: original numerical source, actor gateway and real children.
use super::*;
use fa_reference::action::{Purpose, ResolvedTarget, Scope};
use fa_reference::action::consequence::activation::probe::learned::{KvGroup, KvRow};
use fa_reference::action::consequence::activation::tensor::kv::{experiment::KvSide,
    decoder::sampling::{SamplingPolicy, SamplingStart,
        monitored::{GenerationBudget, GenerationSpec, GenerationTelemetryBudget}}};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits,
    requests::{FileRequestDisposition, actor::{FileActorPort, FileActorTicket}}};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::{CommitteeContract, HelperContract, ReviewWindow,
    decoder_monitoring::LearnedDecoderBindingLimits, human::HumanReviewPolicy,
    learned_host::sidecar::LearnedSidecarRequest, learned_source::LearnedSourceConfig,
    sidecar::{SidecarCongressBudget, SidecarIdentity},
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES}};
use fa_reference::action::consequence::oversight::learned_host::sidecar::workers::{LearnedWorkerRound, LearnedWorkerSchedule};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::reducer::Caps;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub struct Directory(pub PathBuf);
impl Directory {
    pub fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-process-driver-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    pub fn store(&self) -> PathBuf { self.0.join("publication") }
    pub fn round(&self, round: u64) -> PathBuf { self.0.join(round.to_string()) }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("process driver fixture cleanup: {error}"); }
    }
}
pub fn target() -> ResolvedTarget {
    ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 }
}
pub fn profile() -> FileOversightProfile {
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
        InputProfileBinding { profile_id: 1, profile_bytes: b"process-driver-v1".to_vec(),
            tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 7, b"approve?".to_vec(),
    ).unwrap())])).unwrap(), human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 } }
}
pub fn config() -> FileLearnedConfig {
    let model = model::model();
    let policy = model::policy(&model, 0, 1);
    FileLearnedConfig::new(model, LearnedSourceConfig { stream: 21, evaluation_origin: 201, monitor_generation: 31,
        spec: GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
            policy: SamplingPolicy::new(1, 1, 3, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173,
        }).unwrap(), policy, budget: GenerationBudget::default(), telemetry: GenerationTelemetryBudget::default() },
        LearnedDecoderBindingLimits::default()).unwrap().with_required_sidecar().unwrap()
}
pub fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
pub fn step_source(host: &mut Host) {
    let state = host.learned_generation_inspection().unwrap().numerical;
    host.advance_learned_generation(host.revision(), state.actor_revision, state.position).unwrap().unwrap();
}
pub fn program(root: &Path, mode: &str) -> HelperProgram {
    HelperProgram::new(std::env::current_exe().unwrap(), root.to_path_buf(),
        vec![OsString::from("--exact"), OsString::from("process_worker"), OsString::from("--nocapture")],
        BTreeMap::from([(OsString::from("FA_PROCESS_DRIVER_TEST"), OsString::from(mode))])).unwrap()
}
pub fn setup(modes: &[&str]) -> (Directory, FileActorPort<Host>, FileActorTicket<Host>,
    FileHumanReviewer, FileSupervisedDriver, FileProcessDriverLaunch)
{
    let root = Directory::new();
    let (mut host, reviewer) = Host::create(root.store(), profile()).unwrap();
    host.enable_learned_generation(host.revision(), config()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    step_source(&mut host); step_source(&mut host);
    let (port, mut driver) = host.into_supervised_driver();
    let revision = driver.supervisor().host().unwrap().revision();
    driver.supervisor_mut().set_snapshot(revision, Some(snapshot())).unwrap();
    let proposal = ActorProposal { target: target(), payload: b"visible".to_vec(), units: 16,
        deadline: ElapsedTick(100), expected_policy_epoch: 0 };
    let ticket = port.submit(41, &proposal).unwrap();
    let launch = {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let FileRequestDisposition::Admitted { attempt, stage: ActionState::Reviewing } =
            host.request_status(41).unwrap().disposition else { panic!("original request was not admitted"); };
        let actor_revision = host.learned_generation_inspection().unwrap().numerical.actor_revision;
        let revision = host.revision();
        let sidecar = host.begin_learned_sidecar_plan(revision, attempt, actor_revision, LearnedSidecarRequest {
            identity: SidecarIdentity { object_id: 1001, generation: 1, transform_id: 7 },
            priority: vec![KvGroup { row: KvRow { layer: 1, side: KvSide::Value, position: 1 }, head: 0 }],
            budget: SidecarCongressBudget::default(),
        }).unwrap();
        let mut rounds = Vec::new();
        let mut programs = BTreeMap::new();
        for (index, mode) in modes.iter().enumerate() {
            let round = 101 + index as u64;
            let directory = root.round(round);
            std::fs::create_dir(&directory).unwrap();
            rounds.push(LearnedWorkerRound { round, evidence_root: [9 + index as u8; 32],
                window: ReviewWindow { commit_by: ElapsedTick(20 + index as u64 * 20),
                    reveal_by: ElapsedTick(30 + index as u64 * 20) } });
            programs.insert(round, BTreeMap::from([("reviewer".into(), program(&directory, mode))]));
        }
        FileProcessDriverLaunch { journal_revision: host.revision(), request: 41, sidecar,
            schedule: LearnedWorkerSchedule { rounds, helpers: HelperLimits::default(), polls: 4096 },
            programs, limits: LearnedProcessLimits { starts: modes.len() } }
    };
    (root, port, ticket, reviewer, driver, launch)
}
pub fn policy_source(root: &Directory) -> FileEvidenceSource {
    let path = root.0.join("policy.json");
    let scope = profile().delivery.scope;
    let snapshot = EvidenceSnapshot::new(EvidenceIdentity { source: 91, generation: 1, scope },
        snapshot(), BTreeMap::from([("reviewer".into(), Vec::new())])).unwrap();
    std::fs::write(&path, snapshot.encode()).unwrap();
    FileEvidenceSource::new(path, 91, scope, MAX_EVIDENCE_FILE_BYTES).unwrap()
}
pub fn active(run: &FileProcessSupervisedDriver) -> bool {
    matches!(run.review().status(), LearnedProcessStatus::Running | LearnedProcessStatus::Draining)
}
pub fn pause() { std::thread::sleep(Duration::from_millis(1)); }
pub fn until(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !predicate() {
        assert!(Instant::now() < deadline, "public process fixture exceeded its finite wall-clock bound");
        pause();
    }
}
pub fn cleanup(run: &mut FileProcessSupervisedDriver) {
    until(|| { run.reap_helpers(); run.all_helpers_reaped() });
}
pub fn cleanup_children(children: &mut HelperChildren) {
    until(|| { children.reap(); children.all_reaped() });
}
pub fn finish(run: &mut FileProcessSupervisedDriver, source: &mut FileEvidenceSource, tick: u64) {
    until(|| {
        if !active(run) { return true; }
        assert!(matches!(run.step_from_policy_file(source, || ElapsedTick(tick), None).result.unwrap(),
            FileProcessDriverEvent::Review(_)));
        !active(run)
    });
    assert_eq!(run.review().status(), LearnedProcessStatus::Finished);
    cleanup(run);
}
