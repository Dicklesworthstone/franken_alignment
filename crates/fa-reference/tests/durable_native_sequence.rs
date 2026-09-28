//! Original native models, exact learned probes and actual durable publication.
//! Small fixed models are plumbing controls, not trained safety or independence.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[path = "support/restart_model.rs"]
mod actor_model;
#[path = "support/sidecar_native_model.rs"]
mod native_model;
use fa_reference::{Error, Snapshot};
use fa_reference::action::{ActionSpec, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::probe::learned::{KvGroup, KvRow};
use fa_reference::action::consequence::activation::tensor::kv::{experiment::KvSide,
    decoder::sampling::{SamplingPolicy, SamplingStart,
        monitored::{GenerationBudget, GenerationSpec, GenerationTelemetryBudget}}};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{EndpointOutcome, persistent::{FileDeliveryProfile, JournalError, JournalLimits,
    observed::{FileOversight as Host, FileOversightProfile, FileHumanReviewer,
        decoder::learned::{FileLearnedConfig, sidecar::{FileLearnedSidecar, FileLearnedSidecarFinish}},
        helpers::learned::native::{NativeReviewMember, NativeReviewCost, NativeReviewLimits, NativeReviewStatus,
            sequence::{FileNativeSidecarSequence, NativeSequenceRosters}}, guarded::FileGuardSet}}};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::{CommitteeContract, HelperContract, ReviewWindow,
    decoder_monitoring::LearnedDecoderBindingLimits, human::HumanReviewPolicy,
    learned_source::LearnedSourceConfig, learned_host::sidecar::{LearnedSidecarRequest, workers::LearnedWorkerRound},
    helper_workers::{coordinator::Coordinator, wire},
    sidecar::{SidecarCongressBudget, SidecarIdentity, receiver::native::{SidecarProbeQuery,
        SidecarEvaluationStatus, SidecarDecisionBasis}}};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-native-sequence-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
    fn bytes(&self) -> Vec<u8> { std::fs::read(self.store().join("delivery.bin")).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("native sequence cleanup: {error}"); } }
}
fn profile(denial: bool) -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 3,
                tokenizer_generation: 4, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart },
                Vec::new(), vec![0], vec![0], 0).unwrap(), suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::PayloadAtMost(1024)]).unwrap(),
            congress: CongressPolicy { generation: 1, members: ["alpha", "beta"].into_iter().map(|id| (id.into(),
                MemberPolicy { cohort: id.into(), weight: 1 })).collect(),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 2, continue_hold_maximum: 0,
                narrow_at: 2, suspend_at: 3, minimum_members: 2, minimum_cohorts: 2 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(["alpha", "beta"].into_iter().map(|id| (id.into(),
            HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: b"native-sidecar".to_vec(),
                model_epoch: 3, tokenizer_epoch: 4, policy_epoch: 0 }, 7,
                if id == "beta" && denial { b"review this!".to_vec() } else { b"review this?".to_vec() }).unwrap())).collect()).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn config() -> FileLearnedConfig {
    let model = actor_model::model();
    let source = LearnedSourceConfig { stream: 21, evaluation_origin: 201, monitor_generation: 31,
        spec: GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
            policy: SamplingPolicy::new(1, 1, 3, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173 }).unwrap(),
        policy: actor_model::policy(&model, 0, 1), budget: GenerationBudget::default(), telemetry: GenerationTelemetryBudget::default() };
    FileLearnedConfig::new(model, source, LearnedDecoderBindingLimits::default()).unwrap().with_required_sidecar().unwrap()
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn step(host: &mut Host) {
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.advance_learned_generation(host.revision(), n.actor_revision, n.position).unwrap().unwrap();
}
fn rounds(count: usize) -> Vec<LearnedWorkerRound> {
    (0..count).map(|i| LearnedWorkerRound { round: 101 + i as u64, evidence_root: [9 - i as u8; 32],
        window: ReviewWindow { commit_by: ElapsedTick(20 + 20 * i as u64), reveal_by: ElapsedTick(30 + 20 * i as u64) } }).collect()
}
struct Case {
    root: Directory, config: FileLearnedConfig, profile: FileOversightProfile,
    host: Host, human: FileHumanReviewer, action: FrozenAction, sidecar: Option<FileLearnedSidecar>,
}
impl Case {
    fn new(denial: bool) -> Self {
        let root = Directory::new(); let config = config(); let profile = profile(denial);
        let guards = FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
            identity: None, campaigns: None, credential: None };
        let (mut host, roles) = Host::create_guarded_with_learned_generation(root.store(), profile.clone(), &guards, None, config.clone()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host); step(&mut host);
        let action = host.propose(host.revision(), 1, ActionSpec { version: VERSION, scope: profile.delivery.scope,
            target: Some(host.inspect().target), payload: b"visible".to_vec(), required_witnesses: Vec::new(),
            policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 }, snapshot()).unwrap();
        let n = host.learned_generation_inspection().unwrap().numerical;
        let request = LearnedSidecarRequest { identity: SidecarIdentity { object_id: 1001, generation: 1, transform_id: 7 },
            priority: vec![KvGroup { row: KvRow { layer: 1, side: KvSide::Value, position: 1 }, head: 0 }],
            budget: SidecarCongressBudget::default() };
        let sidecar = host.begin_learned_sidecar_plan(host.revision(), 1, n.actor_revision, request).unwrap();
        Self { root, config, profile, host, human: roles.human, action, sidecar: Some(sidecar) }
    }
    fn members(&self, spelling: &[u8], uncertainty: bool) -> BTreeMap<String, NativeReviewMember> {
        let model = actor_model::model(); let mut queries = Vec::new();
        for (layer, contract) in model.cache_profile().layers() {
            for (side, tensor) in [(KvSide::Key, contract.keys()), (KvSide::Value, contract.values())] {
                let mut weights = vec![0.0; tensor.dimensions()];
                let threshold = if uncertainty && *layer == 1 && side == KvSide::Value { weights[0] = 1.0; 0.5 } else { 1.0 };
                queries.push(SidecarProbeQuery { row: KvRow { layer: *layer, side, position: 1 },
                    probe: LinearProbe::new(1, 1, tensor.profile(), &weights, 0.0, threshold).unwrap() });
            }
        }
        self.profile.committee.members().iter().enumerate().map(|(i, (name, helper))| (name.clone(), NativeReviewMember {
            evaluator: native_model::helper(helper.profile_at(self.action.spec().policy_epoch), spelling, false),
            queries: queries.clone(), salt: vec![16 + i as u8; 32],
        })).collect()
    }
    fn rosters(&self, count: usize, uncertainty: bool) -> NativeSequenceRosters {
        rounds(count).iter().map(|r| (r.round, self.members(b"allow", uncertainty))).collect()
    }
    fn start(&mut self, selected: Vec<LearnedWorkerRound>, rosters: NativeSequenceRosters,
        limits: NativeReviewLimits) -> Result<FileNativeSidecarSequence, JournalError> {
        self.host.begin_native_sidecar_sequence(self.host.revision(), self.sidecar.take().unwrap(), selected, rosters, limits, snapshot())
    }
}
fn advance(case: &mut Case, run: &mut FileNativeSidecarSequence) -> Result<NativeReviewStatus, JournalError> {
    run.advance(&mut case.host, run.revision(), ElapsedTick(1), snapshot())
}
fn until_next(case: &mut Case, run: &mut FileNativeSidecarSequence) {
    let first = run.round().round;
    for _ in 0..8192 {
        advance(case, run).unwrap();
        if run.status() != NativeReviewStatus::Running || run.round().round != first { return; }
    }
    panic!("declared native fixture did not finish its round");
}

#[test]
fn original_uncertainty_refines_then_fresh_native_models_and_both_keys_publish() {
    let mut case = Case::new(false); let rosters = case.rosters(2, true);
    let cost = NativeReviewCost::required_sequence(&rosters).unwrap();
    let mut run = case.start(rounds(2), rosters, NativeReviewLimits { native: cost, ..NativeReviewLimits::default() }).unwrap();
    let coarse = run.input().clone(); let numerical = case.host.learned_generation_inspection().unwrap().numerical;
    until_next(&mut case, &mut run);
    assert_eq!(run.round().round, 102); assert_eq!(run.history().len(), 1);
    assert!(matches!(run.history()[0], FileLearnedSidecarFinish::Refined { .. }));
    assert!(run.input().logical_bytes() > coarse.logical_bytes());
    assert!(case.host.authorize(case.host.revision(), 1, &coarse, snapshot()).is_err());
    for record in run.records()[&101].members.values().flatten() {
        assert_eq!(record.progress.status, SidecarEvaluationStatus::Judged(Verdict::Abstain));
        assert!(!record.progress.native_started); assert_eq!(record.progress.native.work.position, 0);
    }
    assert_eq!(run.records()[&102].polls, 0);
    assert!(run.records()[&102].members.values().flatten().all(|r| r.progress.completed_probes == 0 && !r.progress.native_started));
    until_next(&mut case, &mut run);
    assert_eq!(run.status(), NativeReviewStatus::Finished); assert_eq!(run.reservation(), cost);
    assert_eq!(run.history().len(), 2);
    for record in run.records()[&102].members.values().flatten() {
        assert_eq!(record.progress.basis, Some(SidecarDecisionBasis::NativeModel));
        assert_eq!(record.progress.status, SidecarEvaluationStatus::Judged(Verdict::Allow));
        assert_eq!(record.progress.native.requested_prompt_tokens, record.progress.native.reviewed_prompt_tokens);
        assert!(record.commitment_queued && record.reveal_queued);
    }
    assert_eq!(case.host.learned_generation_inspection().unwrap().numerical, numerical);
    for (index, id) in [101, 102].into_iter().enumerate() {
        let saved = Host::read_learned_sidecar_outcome(case.root.store(), &case.profile, &case.config, id).unwrap();
        assert_eq!(saved.result.archive(), run.history()[index].archive());
    }
    let input = run.input(); let automatic = case.host.authorize(case.host.revision(), 1, input, snapshot()).unwrap();
    assert!(case.host.publish(case.host.revision(), 1).is_err());
    let request = case.host.request_human_approval(case.host.revision(), 1001, 1, input, ElapsedTick(40)).unwrap();
    let revision = case.host.revision(); let human = case.human.approve(&mut case.host, revision, &request).unwrap();
    case.host.dispatch(case.host.revision(), &automatic, &human, &case.action, input, snapshot()).unwrap();
    let receipt = case.host.publish_checked(case.host.revision(), 1, Some(input), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(receipt.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    case.host.reconcile(case.host.revision(), 1).unwrap();
    assert_eq!(case.host.inspect().payload, b"visible"); assert_eq!(case.host.inspect().executions, 1);
    assert_eq!(case.host.inspect().control.ledger.charged, 16);
}

#[test]
fn native_denial_after_refinement_is_terminal_and_never_tries_the_unused_roster() {
    let mut case = Case::new(true); let rosters = case.rosters(3, true);
    let reservation = NativeReviewCost::required_sequence(&rosters).unwrap();
    let mut run = case.start(rounds(3), rosters, NativeReviewLimits::default()).unwrap();
    until_next(&mut case, &mut run); assert_eq!(run.round().round, 102);
    until_next(&mut case, &mut run); assert_eq!(run.status(), NativeReviewStatus::Finished);
    assert_eq!(run.records()[&102].members["alpha"].unwrap().progress.status, SidecarEvaluationStatus::Judged(Verdict::Allow));
    assert_eq!(run.records()[&102].members["beta"].unwrap().progress.status, SidecarEvaluationStatus::Judged(Verdict::Deny));
    assert!(!run.records()[&103].started); assert!(run.records()[&103].members.values().all(Option::is_none));
    assert_eq!(run.history().len(), 2); assert_eq!(run.reservation(), reservation);
    assert!(case.host.authorize(case.host.revision(), 1, run.input(), snapshot()).is_err());
    assert_eq!(case.host.inspect().executions, 0);
}

#[test]
fn all_future_profiles_queries_and_aggregate_native_costs_are_checked_before_begin() {
    for change in 0..8 {
        let mut case = Case::new(false); let mut rosters = case.rosters(2, true);
        let cost = NativeReviewCost::required_sequence(&rosters).unwrap();
        let mut limits = NativeReviewLimits { native: cost, ..NativeReviewLimits::default() };
        let mut selected = rounds(2);
        match change {
            0 => limits.native.evaluations -= 1,
            1 => limits.native.scalar_products -= 1,
            2 => limits.native.sampling_entries -= 1,
            3 => { rosters.get_mut(&102).unwrap().get_mut("beta").unwrap().queries.pop(); }
            4 => { rosters.get_mut(&102).unwrap().get_mut("beta").unwrap().salt.clear(); }
            5 => { rosters.get_mut(&102).unwrap().remove("beta"); }
            6 => selected[1].round = selected[0].round,
            7 => {
                let member = rosters.get_mut(&102).unwrap().get_mut("beta").unwrap();
                let mut profile = member.evaluator.policy().input_profile.clone(); profile.model_epoch += 1;
                member.evaluator = native_model::helper(profile, b"allow", false);
            }
            _ => unreachable!(),
        }
        let before = case.root.bytes(); let revision = case.host.revision();
        assert!(case.start(selected, rosters, limits).is_err(), "change {change}");
        assert_eq!(case.host.revision(), revision); assert_eq!(case.root.bytes(), before);
    }
}

#[test]
fn cancellation_preserves_first_archive_and_partial_second_model_work_and_all_round_leases() {
    let mut case = Case::new(false); let rosters = case.rosters(3, true);
    let mut run = case.start(rounds(3), rosters, NativeReviewLimits::default()).unwrap();
    until_next(&mut case, &mut run); assert_eq!(run.round().round, 102);
    for _ in 0..8 { advance(&mut case, &mut run).unwrap(); }
    let before = run.records()[&102].members.clone(); let reservation = run.reservation(); let bytes = case.root.bytes();
    assert!(before.values().flatten().all(|r| r.progress.native.work.position > 0 && !r.commitment_queued));
    run.cancel(run.revision()).unwrap(); assert_eq!(run.status(), NativeReviewStatus::Cancelled);
    for (name, record) in &run.records()[&102].members {
        assert_eq!(record.unwrap().progress.native.work, before[name].unwrap().progress.native.work);
    }
    assert_eq!(run.history().len(), 1); assert_eq!(run.reservation(), reservation); assert_eq!(case.root.bytes(), bytes);
    assert!(!run.records()[&103].started);
    for round in [101, 102, 103] {
        assert_eq!(case.host.commit_review(case.host.revision(), round, "alpha", 0).err(), Some(JournalError::Contract(Error::WrongState)));
    }
    assert!(advance(&mut case, &mut run).is_err());
}

#[test]
fn a_lifetime_poll_limit_does_not_restart_at_the_refinement_boundary() {
    let mut control = Case::new(false); let rosters = control.rosters(2, true);
    let mut measured = control.start(rounds(2), rosters, NativeReviewLimits::default()).unwrap();
    until_next(&mut control, &mut measured); assert_eq!(measured.round().round, 102);
    let first_polls = measured.polls();
    let mut case = Case::new(false); let rosters = case.rosters(2, true);
    let maximum = first_polls + 2;
    let mut run = case.start(rounds(2), rosters, NativeReviewLimits { polls: maximum, ..NativeReviewLimits::default() }).unwrap();
    for _ in 0..maximum { advance(&mut case, &mut run).unwrap(); }
    assert_eq!(run.round().round, 102); assert_eq!(run.polls(), maximum);
    assert_eq!(run.records()[&102].polls, 2); assert_eq!(run.history().len(), 1);
    let before = case.root.bytes();
    assert_eq!(advance(&mut case, &mut run).err(), Some(JournalError::Contract(Error::Limit)));
    assert_eq!(run.status(), NativeReviewStatus::Failed); assert_eq!(run.polls(), maximum);
    assert_eq!(run.records().values().map(|r| r.polls).sum::<usize>(), maximum);
    assert_eq!(case.root.bytes(), before);
}

#[test]
fn source_change_or_pending_original_token_blocks_the_next_roster_before_work() {
    for pending in [false, true] {
        let mut case = Case::new(false); let rosters = case.rosters(2, true);
        let mut run = case.start(rounds(2), rosters, NativeReviewLimits::default()).unwrap();
        until_next(&mut case, &mut run); assert_eq!(run.round().round, 102);
        if pending {
            let n = case.host.learned_generation_inspection().unwrap().numerical;
            case.host.begin_learned_step(case.host.revision(), n.actor_revision, n.position).unwrap();
        } else { step(&mut case.host); }
        assert!(advance(&mut case, &mut run).is_err()); assert_eq!(run.status(), NativeReviewStatus::Failed);
        assert_eq!(run.history().len(), 1);
        assert!(run.records()[&102].members.values().flatten().all(|r| r.progress.completed_probes == 0 && !r.progress.native_started));
        assert_eq!(run.history()[0].archive(), case.host.learned_sidecar_outcome(101).unwrap().result.archive());
        assert_eq!(case.host.inspect().executions, 0);
    }
}

#[test]
fn richer_receiver_limit_failure_keeps_the_original_acknowledged_refinement() {
    let mut case = Case::new(false);
    let coarse = case.host.current_learned_sidecar(case.sidecar.as_ref().unwrap()).unwrap().clone();
    let first = rounds(2)[0]; let mut limits = NativeReviewLimits::default();
    let (coordinator, ports) = Coordinator::new(first.round, first.evidence_root, &coarse, first.window, ElapsedTick(1), limits.helpers).unwrap();
    limits.receive.request_bytes = ports.values().map(|port| wire::encode_request(port).unwrap().len()).max().unwrap();
    drop(ports); drop(coordinator);
    let rosters = case.rosters(2, true);
    let mut run = case.start(rounds(2), rosters, limits).unwrap();
    let mut failure = None;
    for _ in 0..64 {
        if let Err(error) = advance(&mut case, &mut run) { failure = Some(error); break; }
    }
    assert_eq!(failure, Some(JournalError::Contract(Error::Limit)));
    assert_eq!(run.status(), NativeReviewStatus::Failed); assert_eq!(run.history().len(), 1);
    assert!(matches!(run.history()[0], FileLearnedSidecarFinish::Refined { .. }));
    assert_eq!(case.host.input_revision(1).unwrap(), 2); assert!(!run.records()[&102].started);
    assert!(case.host.authorize(case.host.revision(), 1, &coarse, snapshot()).is_err());
    assert_eq!(case.host.inspect().executions, 0);
}

#[test]
fn malformed_native_output_remains_missing_and_stale_calls_do_not_consume_work() {
    let mut case = Case::new(false); let mut foreign = Case::new(false);
    let rosters = BTreeMap::from([(101, case.members(b"bad", false)), (102, case.members(b"allow", false))]);
    let mut run = case.start(rounds(2), rosters, NativeReviewLimits::default()).unwrap();
    let before = run.records().clone(); let bytes = case.root.bytes();
    assert_eq!(run.advance(&mut foreign.host, 0, ElapsedTick(1), snapshot()).err(), Some(JournalError::Contract(Error::Binding)));
    assert_eq!(run.advance(&mut case.host, 1, ElapsedTick(1), snapshot()).err(), Some(JournalError::Contract(Error::Stale)));
    assert_eq!(run.advance(&mut case.host, 0, ElapsedTick(0), snapshot()).err(), Some(JournalError::Contract(Error::Stale)));
    assert_eq!(run.records(), &before); assert_eq!(run.polls(), 0); assert_eq!(case.root.bytes(), bytes);
    for _ in 0..8192 {
        if run.records()[&101].members.values().flatten().all(|r| r.failure.is_some()) { break; }
        advance(&mut case, &mut run).unwrap();
    }
    assert!(run.records()[&101].members.values().flatten().all(|r| r.failure.is_some() && !r.commitment_queued));
    run.advance(&mut case.host, run.revision(), ElapsedTick(30), snapshot()).unwrap();
    assert_eq!(run.status(), NativeReviewStatus::Finished); assert_eq!(run.history().len(), 1);
    assert!(!run.records()[&102].started); assert_eq!(case.host.input_revision(1).unwrap(), 1);
    assert!(case.host.authorize(case.host.revision(), 1, run.input(), snapshot()).is_err());
}
