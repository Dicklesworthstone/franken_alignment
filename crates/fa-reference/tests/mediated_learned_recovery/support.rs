//! Real original numerics, local journals and role-bound control transitions.
//! Synthetic probes/ballots are controls, not detector or deployment evidence.
#![allow(dead_code)]
#[path = "../support/restart_model.rs"]
pub mod numerical;

use fa_reference::{Error, Snapshot};
use fa_reference::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
use fa_reference::action::consequence::activation::identity::{IdentityAnchor, ModelManifest, ModelPassport};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{SamplingPolicy, SamplingStart};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::monitored::{
    GenerationBudget, GenerationSpec, GenerationTelemetryBudget,
};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::persistent::{
    FileDeliveryProfile, FilePermit, JournalError, JournalLimits,
    observed::{FileHumanRequest, FileOversight, FileOversightProfile,
        decoder::learned::FileLearnedConfig,
        guarded::{FileCampaignRequirement, FileGuardSet, FileIdentityRequirement,
            FileOversightRoles, FileRecoveryFloor, FileRecoveryRequirements}},
};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract,
    ReviewWindow, action_frame, decoder_monitoring::LearnedDecoderBindingLimits,
    human::HumanReviewPolicy, identity::{IdentityPolicy, IdentityStatus}, learned_source::LearnedSourceConfig};
use fa_reference::action::consequence::policy_campaign::ReplayLimits;
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::{Verdict, commitment};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Directory(PathBuf);
impl Directory {
    pub(super) fn new() -> Self {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-mediated-learned-{}-{time}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    pub(super) fn store(&self) -> PathBuf { self.0.join("publication") }
    pub(super) fn bytes(&self) -> Vec<u8> { std::fs::read(self.store().join("delivery.bin")).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("mediated learned cleanup: {error}"); }
    }
}
pub(super) fn source(mode: u8) -> LearnedSourceConfig {
    let model = numerical::model();
    LearnedSourceConfig { stream: 21, evaluation_origin: 201, monitor_generation: 31,
        spec: GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
            policy: SamplingPolicy::new(1, 1, 3, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173,
        }).unwrap(), policy: numerical::policy(&model, mode, 1),
        budget: GenerationBudget::default(), telemetry: GenerationTelemetryBudget::default() }
}
pub(super) fn config(mode: u8) -> FileLearnedConfig {
    FileLearnedConfig::new(numerical::model(), source(mode), LearnedDecoderBindingLimits::default()).unwrap()
}
pub(super) fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 3, tokenizer_generation: 4, state_schema_generation: 1,
                grade: RestartGrade::FunctionalRestart }, Vec::new(), vec![0], vec![0], 0).unwrap(),
            suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
            congress: CongressPolicy { generation: 1,
                members: BTreeMap::from([("reviewer".into(), MemberPolicy { cohort: "one".into(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
                narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("reviewer".into(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"guarded-learned-v1".to_vec(),
                tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
pub(super) fn guards() -> FileGuardSet {
    let manifest = ModelManifest { tenant: 1, model: 2, model_generation: 3, host_generation: 1, tokenizer_generation: 4,
        weights: [1; 32], adapters: [2; 32], tokenizer: [3; 32], architecture: [4; 32], numeric_profile: [5; 32] };
    let anchor = IdentityAnchor::new(10, CaptureProfile { tenant: 1, model: 2, model_generation: 3,
        tap: 4, layout_generation: 1 }, 5, vec![7], &[[-1.0, 1.0]]).unwrap();
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None, credential: None,
        identity: Some(FileIdentityRequirement {
            passport: ModelPassport::new(51, 1, manifest, vec![anchor]).unwrap(),
            policy: IdentityPolicy { observer_id: 99, timeout_ticks: 10, validity_ticks: 20, max_checks: 16 },
        }),
        campaigns: Some(FileCampaignRequirement { limits: ReplayLimits { cases: 16, input_bytes: 1_048_576 }, max_campaigns: 8 }),
    }
}
pub(super) fn oversight_requirements(host: &FileOversight, guards: FileGuardSet) -> FileRecoveryRequirements {
    let control = host.inspect().control;
    FileRecoveryRequirements { guards, effective_policy: profile().delivery.policy, credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: host.revision(), control_sequence: control.sequence,
            authority_epoch: control.ledger.epoch } }
}
pub(super) fn step(host: &mut FileOversight)
    -> std::rc::Rc<fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::monitored::GenerationEvent>
{
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.advance_learned_generation(host.revision(), n.actor_revision, n.position).unwrap().unwrap()
}
pub(super) fn resume(host: &mut FileOversight, tick: u64) {
    host.observe_time(host.revision(), ElapsedTick(tick)).unwrap();
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
}
pub(super) fn fresh_identity(host: &mut FileOversight, roles: &FileOversightRoles, id: u64, tick: u64) {
    let g = guards(); let expected = g.identity.unwrap();
    let n = host.learned_generation_inspection().unwrap().numerical;
    let control = host.inspect().control;
    let sequence = control.sequence; let epoch = control.ledger.epoch;
    let check = host.begin_identity_check(host.revision(), id, sequence, n.actor_revision).unwrap().unwrap();
    let observer = roles.identity_observer.as_ref().unwrap();
    let revision = host.revision();
    observer.observe_manifest(host, revision, &check, expected.passport.manifest().clone(), ElapsedTick(tick))
        .unwrap().measurement.unwrap();
    let anchor = &expected.passport.anchors()[&10];
    let frame = SourceFrame::capture(FrameIdentity { profile: anchor.profile(), stream: anchor.stream(),
        sequence: id, position: 0 }, &[0.0]).unwrap();
    let revision = host.revision();
    observer.observe_anchor(host, revision, &check, 10, &frame, ElapsedTick(tick)).unwrap().measurement.unwrap();
    host.apply_identity_check(host.revision(), &check, sequence, epoch).unwrap();
    assert_ne!(host.identity_status().unwrap(), IdentityStatus::Missing);
}
pub(super) fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
pub(super) fn spec(host: &FileOversight) -> ActionSpec {
    ActionSpec { version: VERSION, scope: profile().delivery.scope, target: Some(host.inspect().target),
        payload: b"visible".to_vec(), required_witnesses: Vec::new(), policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(100), units: 16 }
}
pub(super) fn staged(host: &mut FileOversight, attempt: u64)
    -> (fa_reference::action::FrozenAction, CommitteeInput)
{
    let action = host.propose(host.revision(), attempt, spec(host), snapshot()).unwrap();
    let contracts = profile().committee; let helper = &contracts.members()["reviewer"];
    let mut bytes = action_frame(&action); let boundary = bytes.len();
    bytes.extend_from_slice(helper.question()); let end = bytes.len();
    let actual = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
    ], Vec::new()).unwrap();
    let view = EvidenceViewManifest::new(actual, AuthorizationProjection { projection_id: 7,
        policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new() }, Vec::new()).unwrap();
    let input = CommitteeInput::capture(&action, &contracts, BTreeMap::from([("reviewer".into(), view)])).unwrap();
    host.record_inputs(host.revision(), attempt, 0, input.clone()).unwrap();
    (action, input)
}
pub(super) fn reviewed(host: &mut FileOversight, attempt: u64, round: u64, input: &CommitteeInput,
    verdict: Verdict) -> Result<fa_reference::action::consequence::oversight::ObservedReceipt, JournalError>
{
    host.begin_review(host.revision(), attempt, round, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(10), reveal_by: ElapsedTick(20) }, snapshot()).unwrap();
    let salt = b"independent-fixture-salt";
    host.commit_review(host.revision(), round, "reviewer", commitment(round, "reviewer", &[9; 32], verdict, salt).unwrap()).unwrap();
    host.open_reveals(host.revision(), round).unwrap();
    host.reveal_review(host.revision(), round, "reviewer", verdict, salt.to_vec()).unwrap();
    host.finish_review(host.revision(), round, Some(input), snapshot())?.map_err(Into::into)
}
pub(super) fn prepared(host: &mut FileOversight, attempt: u64, round: u64)
    -> (fa_reference::action::FrozenAction, CommitteeInput, FilePermit, FileHumanRequest)
{
    let (action, input) = staged(host, attempt);
    reviewed(host, attempt, round, &input, Verdict::Allow).unwrap();
    let automatic = host.authorize(host.revision(), attempt, &input, snapshot()).unwrap();
    let human = host.request_human_approval(host.revision(), 1000 + attempt, attempt, &input, ElapsedTick(30)).unwrap();
    (action, input, automatic, human)
}

#[path = "../support/file_mediation.rs"]
mod topology;
use fa_reference::action::consequence::delivery::persistent::observed::{
    containment::FileResetRequest,
    decoder::learned::{FileLearnedRecoveryStatus,
        checkpoint::{FileLearnedCheckpoint, FileLearnedResetIntent}},
    guarded::mediated::{FileMediatedRequirements, FileMediatedRoles, FileTopologyRequirement,
        learned::FileMediatedLearnedRecovery},
    mediation::{FileMediationObserver, FileMediationUpdate},
};
use fa_reference::action::consequence::activation::tensor::kv::{
    model::MAX_MODEL_KV_VALUES, decoder::monitoring::restart::KvRestartBudget,
};
use fa_reference::action::consequence::gate::ReviewBinding;
use fa_reference::action::consequence::mediation::{AuthorityGraph, CutCheck, MAX_CHECK_EDGE_VISITS};
use fa_reference::action::consequence::oversight::credibility::{
    Assessment, EvaluationProtocol, Fraction, GroundTruth,
};

pub(super) fn graph(generation: u64, bypass: bool) -> AuthorityGraph {
    topology::graph(profile().delivery.scope, profile().delivery.target, generation, bypass)
}
pub(super) fn protocol() -> EvaluationProtocol {
    EvaluationProtocol { domain: 31, stratum: 2, period: 3,
        minimum_violation_origins: 1, minimum_benign_origins: 1,
        precision_floor: Fraction { numerator: 1, denominator: 2 },
        recall_floor: Fraction { numerator: 1, denominator: 2 },
        false_positive_ceiling: Fraction { numerator: 1, denominator: 2 }, false_stop_budget: 10 }
}
pub(super) fn assessment(truth: GroundTruth) -> Assessment {
    Assessment { origin: 201, evidence_id: [19; 32], truth }
}
pub(super) fn requirements(host: &FileOversight, g: FileGuardSet,
    evaluation: Option<EvaluationProtocol>) -> FileMediatedRequirements
{
    let topology = host.mediation_snapshot().unwrap();
    FileMediatedRequirements { oversight: oversight_requirements(host, g),
        topology: FileTopologyRequirement { initial: graph(1, false),
            current: topology.graph, available: topology.available }, prediction: None, evaluation }
}
pub(super) fn create(root: &Directory, config: &FileLearnedConfig,
    evaluation: Option<EvaluationProtocol>) -> (FileOversight, FileMediatedRoles)
{
    FileOversight::create_mediated_guarded_with_learned_generation(root.store(), profile(),
        &guards(), None, graph(1, false), evaluation, config.clone()).unwrap()
}
pub(super) fn certify(host: &mut FileOversight, role: &FileMediationObserver)
    -> Result<CutCheck, Error>
{
    let proposal = host.mediation_snapshot().unwrap().graph.propose_cut(&[2]).unwrap();
    let revision = host.revision(); let epoch = host.inspect().control.ledger.epoch;
    role.certify(host, revision, epoch, &proposal, MAX_CHECK_EDGE_VISITS).unwrap()
}
pub(super) fn replacement(host: &FileOversight, operation: u64,
    next: Option<AuthorityGraph>) -> FileMediationUpdate
{
    FileMediationUpdate { operation, expected_generation: host.mediation_snapshot().unwrap().graph.spec().generation,
        expected_authority_epoch: host.inspect().control.ledger.epoch, next }
}
pub(super) fn update(host: &mut FileOversight, role: &FileMediationObserver,
    operation: u64, next: Option<AuthorityGraph>)
{
    let request = replacement(host, operation, next); let revision = host.revision();
    role.update(host, revision, &request).unwrap();
}
pub(super) fn capture(host: &mut FileOversight) -> FileLearnedCheckpoint {
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.capture_learned_checkpoint(host.revision(), 1, n.actor_revision,
        host.inspect().control.ledger.epoch).unwrap()
}
pub(super) fn begin_reset(host: &mut FileOversight, saved: &FileLearnedCheckpoint) -> FileLearnedResetIntent {
    let c = host.inspect().control;
    let intent = FileLearnedResetIntent::for_recovery(saved.id(), FileResetRequest {
        operation: 900, expected_control_sequence: c.sequence,
        expected_actor_revision: host.actor_snapshot().unwrap().actor_revision,
        expected_authority_epoch: c.ledger.epoch,
        binding: ReviewBinding { round: 900, reducer_generation: 1, evidence_root: [8; 32] },
        retained_targets: vec![host.inspect().target],
    }, KvRestartBudget { cache_values: MAX_MODEL_KV_VALUES,
        audit: source(0).policy.allowance() }).unwrap();
    host.begin_learned_reset(host.revision(), saved, intent.control().clone(), intent.budget()).unwrap();
    assert_eq!(&host.pending_learned_reset().unwrap().unwrap().intent, &intent);
    intent
}
pub(super) fn drive(run: &mut FileMediatedLearnedRecovery, quantum: usize) -> Result<(), JournalError> {
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        let before = run.progress(); let after = run.advance(before.replayed_events, quantum)?;
        assert!(after.replayed_events - before.replayed_events <= quantum);
    }
    Ok(())
}
