//! Recipe-owned joint qualification through the original learned and journal
//! owners. Synthetic weights, labels and ballots test enforcement, not detector
//! qualification; publication still needs original automatic and human keys.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[path = "support/learned_text_model.rs"]
#[allow(dead_code)]
mod numerical;
#[path = "support/file_mediation.rs"]
mod topology;

use fa_reference::{Error, Snapshot};
use fa_reference::action::{ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope};
use fa_reference::action::consequence::activation::{
    consistency::{BinaryForecast, ErrorBudget, ForecastRegistration},
    monitor::learned::LearnedMonitorBudget, probe::learned::MAX_CHECKED_KV_BYTES,
    tensor::kv::experiment::KvSide,
};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy,
    CredibilityBinding, CredibilityRequirements,
    credibility::{Campaign, CaseSpec, CredibilityLedger, EvaluationLabel,
        EvaluationScope, HelperGeneration, LabelSource, LabelVerdict, Observation}};
use fa_reference::action::consequence::delivery::{EndpointOutcome, stream::StreamProfile,
    persistent::{FileDeliveryProfile, FilePermit, JournalError, JournalLimits, RecoveryReserve,
        observed::{FileOversight, FileOversightProfile, FileHumanReviewer, FileHumanPermit,
            consistency::{FileConsistencyConfig, FileConsistencyParameters,
                learned::FileLearnedConsistencyConfig},
            credibility::held_out_joint::{HeldOutJointPolicy, HeldOutJointBudget},
            decoder::learned::{FileLearnedConfig, FileLearnedRecoveryStatus},
            guarded::{FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
                mediated::{FileMediatedRequirements, FileMediatedRoles, FileTopologyRequirement,
                    learned::FileMediatedLearnedRecovery,
                    predictive::{FileMediatedPredictor, FilePredictiveMediatedRequirements,
                        FilePredictiveMediatedLearnedRecovery}}},
            shutdown::recovery::FileAdmissionStoppedHistory}}};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile,
    session::policy::{Policy, Predicate,
        controller::credibility::CredibilityActivation}};
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract,
    ReviewWindow, action_frame, decoder_monitoring::LearnedDecoderBindingLimits,
    credibility::{EvaluationProtocol, Fraction}, human::HumanReviewPolicy,
    joint_credibility::{JointPromotionPolicy, JointReplayBudget},
    learned_host::text::LearnedTextTarget, learned_source::LearnedEvidenceLimits};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::{Verdict, commitment};
use std::{collections::{BTreeMap, BTreeSet}, fs, path::PathBuf,
    sync::atomic::{AtomicU64, Ordering}};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-learned-joint-{}-{time}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap(); Self(root)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
    fn bytes(&self) -> Vec<u8> { fs::read(self.store().join("delivery.bin")).unwrap() }
    fn stage(&self) { fs::write(self.store().join("delivery.pending"), b"unconfirmed candidate").unwrap(); }
    fn staged(&self) { assert_eq!(fs::read(self.store().join("delivery.pending")).unwrap(), b"unconfirmed candidate"); }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("learned joint cleanup: {error}"); }
    }
}
fn policy() -> HeldOutJointPolicy {
    HeldOutJointPolicy::new(71, 1, 1, 2, 0, 0, HeldOutJointBudget { cases: 16, member_outcomes: 64 }).unwrap()
}
fn alternatives() -> Vec<HeldOutJointPolicy> {
    let original = [71, 1, 1, 2, 0, 0, 16, 64];
    (0..8).map(|field| {
        let mut next = original; next[field] += 1;
        HeldOutJointPolicy::new(next[0], next[1], next[2], next[3], next[4], next[5],
            HeldOutJointBudget { cases: next[6] as usize, member_outcomes: next[7] as usize }).unwrap()
    }).collect()
}
fn profile(hold_maximum: u64) -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile { delivery: FileDeliveryProfile {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: 100, max_attempts: 16,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 3,
            tokenizer_generation: 4, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart },
            Vec::new(), vec![0], vec![0], 0).unwrap(),
        suspend_at_incident: 100, policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
        congress: CongressPolicy { generation: 1, members: ["a", "b"].into_iter().map(|name|
            (name.into(), MemberPolicy { cohort: name.into(), weight: 10 })).collect(),
            caps: Caps { per_member: 10, per_cohort: 10 }, continue_minimum: 5,
            continue_hold_maximum: hold_maximum, narrow_at: 20, suspend_at: 30,
            minimum_members: 2, minimum_cohorts: 2 },
        narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
        retention_ticks: 1000, max_deliveries: 16, clock_domain: 99, limits: JournalLimits::default(),
    }, committee: CommitteeContract::new(["a", "b"].into_iter().map(|name| (name.into(),
        HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: b"learned-joint".to_vec(),
            tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 7, b"approve?".to_vec()).unwrap())).collect()).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 16 } }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lane { Plain, Raw, Owned }
const LANES: [Lane; 3] = [Lane::Plain, Lane::Raw, Lane::Owned];
fn base(stream: bool) -> FileLearnedConfig {
    let model = numerical::model(&[u32::from(b'O'), u32::from(b'K'), numerical::END]);
    if stream {
        FileLearnedConfig::new_text_stream(model.clone(), numerical::tokenizer(&model), numerical::config(&model),
            LearnedDecoderBindingLimits::default(), StreamProfile::new(1, 1, 4, 64, 128).unwrap()).unwrap()
    } else {
        FileLearnedConfig::new_text(model.clone(), numerical::tokenizer(&model), numerical::config(&model),
            LearnedDecoderBindingLimits::default()).unwrap()
    }
}
fn configure(mut config: FileLearnedConfig, lane: Lane) -> FileLearnedConfig {
    config = config.with_required_sidecar().unwrap();
    if lane == Lane::Plain { return config; }
    let model = numerical::model(&[u32::from(b'O'), u32::from(b'K'), numerical::END]);
    let neutral = BinaryForecast::new(32_768, 32_768).unwrap();
    let capture = if lane == Lane::Raw { model.residual_contract(1).unwrap().profile() }
        else { model.cache_profile().layers()[&1].values().profile() };
    let prediction = FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 91, probe_generation: 1, profile: capture, weights: vec![1.0, 0.0, 0.0, 0.0],
        bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"OK".to_vec(), negative: neutral, at_threshold: neutral, positive: neutral },
        alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21, max_predictions: 8, max_prediction_age_ticks: 50,
    }).unwrap();
    match lane {
        Lane::Raw => config.with_required_pre_output_forecast(prediction.with_hosted_residual(1)
            .unwrap().with_pre_output_forecast().unwrap()).unwrap(),
        Lane::Owned => config.with_required_owned_pre_output_forecast(FileLearnedConsistencyConfig::new(
            prediction, 1, KvSide::Value, LearnedMonitorBudget::default(), LearnedMonitorBudget::default(),
            MAX_CHECKED_KV_BYTES).unwrap().with_owned_generation().unwrap()
            .with_pre_output_forecast().unwrap()).unwrap(),
        Lane::Plain => unreachable!(),
    }
}
fn guards(config: &FileLearnedConfig) -> FileGuardSet {
    FileGuardSet { stream: config.text_stream_profile(), decoder: None, decoder_stop: None,
        source: config.required_policy_source(), identity: None, campaigns: None, credential: None }
}
fn predictor(config: &FileLearnedConfig) -> FileMediatedPredictor {
    match config.required_pre_output_forecast() {
        Some(raw) => FileMediatedPredictor::Raw(raw.clone()),
        None => FileMediatedPredictor::Owned(config.required_owned_pre_output_forecast().unwrap().clone()),
    }
}
fn create(root: &Directory, lane: Lane, config: &FileLearnedConfig, evaluation: Option<EvaluationProtocol>)
    -> Result<(FileOversight, FileMediatedRoles), JournalError>
{
    let p = profile(0); let graph = topology::graph(p.delivery.scope, p.delivery.target, 1, false);
    if lane == Lane::Plain {
        FileOversight::create_mediated_guarded_with_learned_generation_reserved(root.store(), p,
            &guards(config), None, graph, evaluation, config.clone(), RecoveryReserve::terminal())
    } else {
        FileOversight::create_predictive_mediated_guarded_with_learned_generation_reserved(root.store(), p,
            &guards(config), None, graph, predictor(config), evaluation, config.clone(), RecoveryReserve::terminal())
    }
}
fn expected(host: &FileOversight, config: &FileLearnedConfig) -> FileMediatedRequirements {
    let control = host.inspect().control; let p = profile(0); let current = host.mediation_snapshot().unwrap();
    FileMediatedRequirements { oversight: FileRecoveryRequirements { guards: guards(config),
        effective_policy: p.delivery.policy, credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: host.revision(), control_sequence: control.sequence,
            authority_epoch: control.ledger.epoch } },
        topology: FileTopologyRequirement { initial: topology::graph(p.delivery.scope, p.delivery.target, 1, false),
            current: current.graph, available: current.available }, prediction: None, evaluation: None }
}
enum Recovery { Plain(FileMediatedLearnedRecovery), Predictive(FilePredictiveMediatedLearnedRecovery) }
impl Recovery {
    fn drive(&mut self) {
        loop {
            let before = match self { Self::Plain(run) => run.progress(), Self::Predictive(run) => run.progress() };
            if before.status != FileLearnedRecoveryStatus::Replaying { assert_eq!(before.status, FileLearnedRecoveryStatus::Ready); break; }
            let after = match self { Self::Plain(run) => run.advance(before.replayed_events, 1),
                Self::Predictive(run) => run.advance(before.replayed_events, 1) }.unwrap();
            assert_eq!(after.replayed_events, before.replayed_events + 1);
        }
    }
    fn finish(self) -> (FileOversight, FileMediatedRoles) {
        match self { Self::Plain(run) => run.finish(), Self::Predictive(run) => run.finish() }.unwrap()
    }
    fn stop(self) -> FileAdmissionStoppedHistory {
        match self {
            Self::Plain(run) => { let request = run.stop_request(901).unwrap(); run.finish_admission_stop(request) }
            Self::Predictive(run) => { let request = run.stop_request(901).unwrap(); run.finish_admission_stop(request) }
        }.unwrap()
    }
}
fn begin(root: &Directory, lane: Lane, config: &FileLearnedConfig, expected: &FileMediatedRequirements)
    -> Result<Recovery, JournalError>
{
    if lane == Lane::Plain {
        FileOversight::begin_open_mediated_guarded_with_learned_generation(root.store(), profile(0), expected, config)
            .map(Recovery::Plain)
    } else {
        let expected = FilePredictiveMediatedRequirements { oversight: expected.oversight.clone(),
            topology: expected.topology.clone(), prediction: predictor(config), evaluation: expected.evaluation.clone() };
        FileOversight::begin_open_predictive_mediated_guarded_with_learned_generation(root.store(), profile(0), &expected, config)
            .map(Recovery::Predictive)
    }
}

#[test]
fn complete_policy_identity_composes_without_changing_legacy_recipe_bytes() {
    let old = base(false); let length = old.encoded_bytes();
    assert_eq!(old.required_held_out_joint(), None); assert_eq!(old, base(false));
    let selected = old.clone().with_required_held_out_joint(policy()).unwrap();
    assert_eq!(selected.required_held_out_joint(), Some(policy())); assert_ne!(selected, old);
    assert_eq!(old.encoded_bytes(), length); assert_eq!(selected.encoded_bytes(), length + 96);
    assert_eq!(selected.clone().with_required_held_out_joint(policy()).err(), Some(Error::Duplicate));
    for changed in alternatives() { assert_ne!(old.clone().with_required_held_out_joint(changed).unwrap(), selected); }
    for lane in LANES {
        let early = configure(selected.clone(), lane);
        let late = configure(old.clone(), lane).with_required_held_out_joint(policy()).unwrap();
        for config in [&early, &late] {
            assert_eq!(config.required_held_out_joint(), Some(policy())); assert!(config.requires_sidecar());
            assert_eq!(config.required_pre_output_forecast().is_some(), lane == Lane::Raw);
            assert_eq!(config.required_owned_pre_output_forecast().is_some(), lane == Lane::Owned);
        }
        assert_ne!(early, late); // Full wrapper order remains original identity.
    }
    for config in [base(true).with_required_held_out_joint(policy()).unwrap().with_text_stream_continuation().unwrap(),
        base(true).with_text_stream_continuation().unwrap().with_required_held_out_joint(policy()).unwrap()] {
        assert_eq!(config.required_held_out_joint(), Some(policy())); assert!(config.text_stream_profile().is_some());
    }
}

#[test]
fn all_predictor_lanes_bind_policy_before_cleanup_and_keep_native_terminal_recovery() {
    for lane in LANES {
        let root = Directory::new(); let legacy = configure(base(false), lane);
        let config = legacy.clone().with_required_held_out_joint(policy()).unwrap();
        let (mut host, roles) = create(&root, lane, &config, None).unwrap();
        assert_eq!(host.held_out_joint_policy().unwrap(), Some(policy()));
        assert_eq!(host.journal_capacity().unwrap().reserve(), Some(RecoveryReserve::terminal()));
        assert!(roles.evaluator.is_none()); assert_eq!(roles.consistency_observer.is_some(), lane != Lane::Plain);
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        host.advance_learned_generation(host.revision(), numerical.actor_revision, numerical.position).unwrap().unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let required = expected(&host, &config); drop(host); root.stage(); let bytes = root.bytes();
        let mut wrong = vec![legacy.clone()];
        wrong.extend(alternatives().into_iter().map(|changed| legacy.clone().with_required_held_out_joint(changed).unwrap()));
        for other in wrong {
            assert_eq!(begin(&root, lane, &other, &required).err(), Some(Error::Binding.into()));
            assert_eq!(root.bytes(), bytes); root.staged();
        }
        let mut run = begin(&root, lane, &config, &required).unwrap(); run.drive();
        assert_eq!(root.bytes(), bytes); root.staged();
        let (host, roles) = run.finish();
        assert_eq!(host.held_out_joint_policy().unwrap(), Some(policy()));
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert!(roles.evaluator.is_none()); assert!(!root.store().join("delivery.pending").exists());
        let required = expected(&host, &config); drop(host);
        let mut run = begin(&root, lane, &config, &required).unwrap(); run.drive();
        let stopped = run.stop(); assert_eq!(stopped.snapshot().stop.as_ref().unwrap().request().operation, 901);
        assert_eq!(stopped.snapshot().executions, 0);
    }
}

fn protocol() -> EvaluationProtocol {
    EvaluationProtocol { domain: 1, stratum: 1, period: 1, minimum_violation_origins: 2,
        minimum_benign_origins: 1, precision_floor: Fraction { numerator: 1, denominator: 2 },
        recall_floor: Fraction { numerator: 1, denominator: 2 },
        false_positive_ceiling: Fraction { numerator: 1, denominator: 2 }, false_stop_budget: 10 }
}
fn owned_policy() -> JointPromotionPolicy {
    JointPromotionPolicy::new(81, 1, Fraction { numerator: 0, denominator: 1 },
        Fraction { numerator: 0, denominator: 1 }, JointReplayBudget::default()).unwrap()
}
#[test]
fn conflicting_evaluation_lanes_refuse_before_first_storage_or_acknowledgment() {
    for lane in LANES {
        let root = Directory::new();
        let config = configure(base(false), lane).with_required_held_out_joint(policy()).unwrap();
        assert!(create(&root, lane, &config, Some(protocol())).is_err());
        assert!(!root.store().exists());
    }
    for existing in 0..3 {
        let root = Directory::new(); let (mut host, _) = FileOversight::create(root.store(), profile(0)).unwrap();
        match existing {
            0 => { host.enable_held_out_joint(host.revision(), policy()).unwrap(); }
            1 => { host.enable_credibility(host.revision(), protocol()).unwrap(); }
            _ => { host.enable_joint_credibility(host.revision(), protocol(), owned_policy()).unwrap(); }
        }
        let before = host.inspect(); let bytes = root.bytes();
        assert!(host.enable_learned_generation(host.revision(), base(false).with_required_held_out_joint(policy()).unwrap()).is_err());
        assert_eq!(host.inspect(), before); assert_eq!(root.bytes(), bytes);
        assert!(host.learned_generation_inspection().is_err());
    }
    let root = Directory::new();
    let (mut host, _) = FileOversight::create_with_learned_text(root.store(), profile(0),
        base(false).with_required_held_out_joint(policy()).unwrap()).unwrap();
    let before = host.inspect(); let bytes = root.bytes();
    assert!(host.enable_held_out_joint(host.revision(), policy()).is_err());
    assert!(host.enable_credibility(host.revision(), protocol()).is_err());
    assert!(host.enable_joint_credibility(host.revision(), protocol(), owned_policy()).is_err());
    assert_eq!(host.inspect(), before); assert_eq!(root.bytes(), bytes);

    // This independently valid recipe needs an actual stream bootstrap. Its
    // learned installation fails AFTER the nested joint enable in the candidate;
    // neither the policy nor a partial numerical owner may leak out of it.
    let missing_stream = base(true).with_required_held_out_joint(policy()).unwrap();
    let empty = Directory::new();
    assert!(FileOversight::create_with_learned_text(empty.store(), profile(0), missing_stream.clone()).is_err());
    assert!(!empty.store().exists());
    let (mut host, _) = FileOversight::create(empty.store(), profile(0)).unwrap();
    let before = host.inspect(); let bytes = empty.bytes();
    assert!(host.enable_learned_generation(host.revision(), missing_stream).is_err());
    assert_eq!(host.held_out_joint_policy().unwrap(), None);
    assert!(host.learned_generation_inspection().is_err());
    assert_eq!(host.inspect(), before); assert_eq!(empty.bytes(), bytes);
}

fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn reviewed(host: &mut FileOversight, p: &FileOversightProfile, id: u64) -> (FrozenAction, CommitteeInput) {
    let target = LearnedTextTarget { target: host.inspect().target, required_witnesses: Vec::new(),
        policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 7 };
    let action = host.propose_learned_text(host.revision(), id, target, snapshot()).unwrap();
    assert_eq!(action.spec().payload, b"OK");
    let views = p.committee.members().iter().map(|(name, helper)| {
        let mut bytes = action_frame(&action); let boundary = bytes.len();
        bytes.extend_from_slice(helper.question()); let end = bytes.len();
        let actual = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
            SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
            SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
        ], Vec::new()).unwrap();
        (name.clone(), EvidenceViewManifest::new(actual, AuthorizationProjection { projection_id: 7,
            policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new() }, Vec::new()).unwrap())
    }).collect();
    let inputs = CommitteeInput::capture(&action, &p.committee, views).unwrap();
    host.record_inputs(host.revision(), id, 0, inputs.clone()).unwrap();
    let round = 100 + id;
    host.begin_review(host.revision(), id, round, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) }, snapshot()).unwrap();
    for member in ["a", "b"] {
        host.commit_review(host.revision(), round, member, commitment(round, member, &[9; 32], Verdict::Allow, b"salt").unwrap()).unwrap();
    }
    host.open_reveals(host.revision(), round).unwrap();
    for member in ["a", "b"] { host.reveal_review(host.revision(), round, member, Verdict::Allow, b"salt".to_vec()).unwrap(); }
    host.finish_review(host.revision(), round, Some(&inputs), snapshot()).unwrap().unwrap();
    (action, inputs)
}
fn ready(root: &Directory, p: &FileOversightProfile, selected: bool) -> (FileOversight, FileHumanReviewer, FileLearnedConfig) {
    let config = if selected { base(false).with_required_held_out_joint(policy()).unwrap() } else { base(false) };
    let (mut host, human) = FileOversight::create_with_learned_text(root.store(), p.clone(), config.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    while host.learned_generation_inspection().unwrap().numerical.status.is_active() {
        let n = host.learned_generation_inspection().unwrap().numerical;
        host.advance_learned_generation(host.revision(), n.actor_revision, n.position).unwrap().unwrap();
    }
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"OK");
    for id in 1..=2 { reviewed(&mut host, p, id); host.cancel(host.revision(), id).unwrap(); }
    (host, human, config)
}
fn activation(host: &FileOversight, p: &FileOversightProfile, operation: u64, generation: u64) -> CredibilityActivation {
    let mut ledger = CredibilityLedger::new(Campaign {
        scope: EvaluationScope { campaign: 1, model_generation: 3, evaluator_generation: 1, held_out_manifest: [8; 32] },
        label_owner: "independent".into(), helpers: ["a", "b"].into_iter().map(|name|
            (name.into(), HelperGeneration { generation: 1, cohort: name.into() })).collect(),
        strata: BTreeSet::from(["effect".into()]), cases: (1..=3).map(|id| CaseSpec {
            id, stratum: "effect".into(), evidence_root: [id as u8; 32], dispatch_sequence: 2 }).collect(),
    }).unwrap();
    for id in 1..=3 {
        let votes = ["a", "b"].into_iter().enumerate().map(|(index, name)| (name.into(),
            if id == index as u64 + 1 { Observation::Hold { first_sequence: 1 } } else { Observation::Clear })).collect();
        ledger.record_observations(id, votes).unwrap();
        ledger.record_label(id, EvaluationLabel { owner: "independent".into(), evaluator_generation: 1,
            source: LabelSource::IndependentEvaluation, evidence_root: [id as u8; 32], recorded_sequence: 3,
            verdict: if id == 3 { LabelVerdict::Safe } else { LabelVerdict::Violation } }).unwrap();
    }
    let evidence = ledger.seal(3).unwrap();
    CredibilityActivation { operation, expected_control_sequence: host.inspect().control.sequence,
        expected_epoch: host.inspect().control.ledger.epoch, scope: p.delivery.scope,
        policy_generation: p.delivery.policy.generation(), actor_profile: p.delivery.actor.profile(),
        binding: CredibilityBinding { scope: evidence.scope().clone(), label_owner: evidence.label_owner().into(),
            helpers: evidence.helpers().clone(), strata: evidence.strata().clone(), reducer_generation: generation },
        stratum: "effect".into(), requirements: CredibilityRequirements {
            minimum_safe_cases: 1, minimum_violation_cases: 2, minimum_precision_ppm: 1_000_000,
            minimum_timely_recall_ppm: 500_000, maximum_false_positive_ppm: 0, base_weight: 10,
            lead_bonus_weight: 0, lead_saturation_sequences: 0, maximum_evidence_age: 100,
            maximum_member_share_ppm: 500_000, maximum_cohort_share_ppm: 500_000 }, snapshot: evidence }
}
fn prepared(host: &mut FileOversight, human: &FileHumanReviewer, p: &FileOversightProfile, id: u64)
    -> (FrozenAction, CommitteeInput, FilePermit, FileHumanPermit)
{
    let (action, inputs) = reviewed(host, p, id);
    let automatic = host.authorize(host.revision(), id, &inputs, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), 1000 + id, id, &inputs, ElapsedTick(30)).unwrap();
    let revision = host.revision(); let key = human.approve(host, revision, &request).unwrap();
    (action, inputs, automatic, key)
}

#[test]
fn joint_regression_is_causal_and_accepted_learned_output_uses_both_original_keys() {
    for (hold_maximum, selected) in [(5, false), (5, true), (0, true)] {
        let root = Directory::new(); let p = profile(hold_maximum);
        let (mut host, human, _) = ready(&root, &p, selected);
        let request = activation(&host, &p, 10, 2); let before = host.inspect(); let bytes = root.bytes();
        let result = host.activate_credibility(host.revision(), request, &p.committee);
        if hold_maximum == 5 && selected {
            assert_eq!(result, Err(Error::Incomplete.into()));
            assert_eq!(host.inspect(), before); assert_eq!(root.bytes(), bytes);
            assert!(host.held_out_joint_report(10).is_err()); continue;
        }
        result.unwrap();
        if selected {
            let report = host.held_out_joint_report(10).unwrap().unwrap();
            assert!(report.qualified()); assert_eq!(report.candidate.members["a"].weight, 5);
            assert_eq!(report.work, HeldOutJointBudget { cases: 3, member_outcomes: 12 });
        } else { assert!(host.held_out_joint_report(10).unwrap().is_none()); }
        let (action, inputs, automatic, human) = prepared(&mut host, &human, &p, 3);
        host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
        let result = host.publish_checked(host.revision(), 3, Some(&inputs), snapshot(), ElapsedTick(2)).unwrap();
        assert!(matches!(result.outcome, EndpointOutcome::Executed { .. }));
        host.reconcile(host.revision(), 3).unwrap();
        assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().control.ledger.charged, 7);
        assert_eq!(host.inspect().payload, b"OK");
    }
}

#[test]
fn learned_recovery_retains_joint_report_without_reviving_qualification_or_old_keys() {
    let root = Directory::new(); let p = profile(0);
    let (mut host, human, config) = ready(&root, &p, true);
    let first = activation(&host, &p, 10, 2);
    let receipt = host.activate_credibility(host.revision(), first.clone(), &p.committee).unwrap();
    let (action, inputs, old_auto, old_human) = prepared(&mut host, &human, &p, 3);
    let report = host.held_out_joint_report(10).unwrap().unwrap().clone(); drop(host);
    let (mut host, human) = FileOversight::open_with_learned_generation(root.store(), p.clone(), &config).unwrap();
    assert_eq!(host.held_out_joint_policy().unwrap(), Some(policy()));
    assert_eq!(host.held_out_joint_report(10).unwrap(), Some(&report));
    assert_eq!(host.check_credibility(), Err(Error::Stale.into()));
    let before = host.inspect(); let bytes = root.bytes();
    assert_eq!(host.activate_credibility(0, first.clone(), &p.committee).unwrap(), receipt);
    assert_eq!(host.inspect(), before); assert_eq!(root.bytes(), bytes);
    assert_eq!(host.check_credibility(), Err(Error::Stale.into()));
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
    assert!(host.dispatch(host.revision(), &old_auto, &old_human, &action, &inputs, snapshot()).is_err());
    let next = activation(&host, &p, 11, 3);
    host.activate_credibility(host.revision(), next, &p.committee).unwrap();
    let (action, inputs, automatic, human) = prepared(&mut host, &human, &p, 4);
    let before = host.inspect();
    assert_eq!(host.activate_credibility(0, first, &p.committee).unwrap(), receipt);
    assert_eq!(host.inspect(), before);
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    let result = host.publish_checked(host.revision(), 4, Some(&inputs), snapshot(), ElapsedTick(3)).unwrap();
    assert!(matches!(result.outcome, EndpointOutcome::Executed { .. }));
    assert_eq!(host.inspect().executions, 1);
}
