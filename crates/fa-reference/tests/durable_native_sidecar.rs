//! Original Rust model/probe engines and real journals. These tiny model controls
//! test plumbing and failure boundaries, not detector quality or independence.
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
        helpers::learned::native::{FileNativeSidecarReview, NativeReviewMember, NativeReviewCost,
            NativeReviewLimits, NativeReviewStatus, NativeMemberFailure}, guarded::FileGuardSet}}};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::{CommitteeContract, HelperContract, ReviewWindow,
    decoder_monitoring::LearnedDecoderBindingLimits, human::HumanReviewPolicy,
    learned_source::LearnedSourceConfig, learned_host::sidecar::{LearnedSidecarRequest, workers::LearnedWorkerRound},
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
        let path = std::env::temp_dir().join(format!("fa-native-congress-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
    fn bytes(&self) -> Vec<u8> { std::fs::read(self.store().join("delivery.bin")).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("native congress cleanup: {error}"); } }
}
fn profile(deny_question: bool) -> FileOversightProfile {
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
                if id == "beta" && deny_question { b"review this!".to_vec() } else { b"review this?".to_vec() }).unwrap())).collect()).unwrap(),
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
fn round(id: u64) -> LearnedWorkerRound {
    LearnedWorkerRound { round: id, evidence_root: [9; 32], window: ReviewWindow { commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30) } }
}
struct Case {
    root: Directory, config: FileLearnedConfig, profile: FileOversightProfile,
    host: Host, human: FileHumanReviewer, action: FrozenAction, sidecar: Option<FileLearnedSidecar>, position: u64,
}
impl Case {
    fn new(positions: u64, deny_question: bool) -> Self {
        let root = Directory::new(); let config = config(); let profile = profile(deny_question);
        let guards = FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
            identity: None, campaigns: None, credential: None };
        let (mut host, roles) = Host::create_guarded_with_learned_generation(root.store(), profile.clone(), &guards, None, config.clone()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        for _ in 0..positions { step(&mut host); }
        let action = host.propose(host.revision(), 1, ActionSpec { version: VERSION, scope: profile.delivery.scope,
            target: Some(host.inspect().target), payload: b"visible".to_vec(), required_witnesses: Vec::new(),
            policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 }, snapshot()).unwrap();
        let n = host.learned_generation_inspection().unwrap().numerical;
        let request = LearnedSidecarRequest { identity: SidecarIdentity { object_id: 1001, generation: 1, transform_id: 7 },
            priority: vec![KvGroup { row: KvRow { layer: 1, side: KvSide::Value, position: positions - 1 }, head: 0 }],
            budget: SidecarCongressBudget::default() };
        let sidecar = host.begin_learned_sidecar_plan(host.revision(), 1, n.actor_revision, request).unwrap();
        Self { root, config, profile, host, human: roles.human, action, sidecar: Some(sidecar), position: positions - 1 }
    }
    fn members(&self, spelling: &[u8], mode: u8, alarm: bool) -> BTreeMap<String, NativeReviewMember> {
        let model = actor_model::model(); let mut queries = Vec::new();
        for (layer, contract) in model.cache_profile().layers() {
            for (side, tensor) in [(KvSide::Key, contract.keys()), (KvSide::Value, contract.values())] {
                let mut weights = vec![0.0; tensor.dimensions()];
                let threshold = if mode > 0 && *layer == 1 && side == KvSide::Value { weights[usize::from(mode == 2)] = 1.0; 0.5 } else { 1.0 };
                queries.push(SidecarProbeQuery { row: KvRow { layer: *layer, side, position: self.position },
                    probe: LinearProbe::new(1, 1, tensor.profile(), &weights, 0.0, threshold).unwrap() });
            }
        }
        self.profile.committee.members().iter().enumerate().map(|(i, (name, helper))| (name.clone(), NativeReviewMember {
            evaluator: native_model::helper(helper.profile_at(self.action.spec().policy_epoch), spelling, alarm),
            queries: queries.clone(), salt: vec![16 + i as u8; 32],
        })).collect()
    }
    fn start(&mut self, members: BTreeMap<String, NativeReviewMember>, limits: NativeReviewLimits) -> Result<FileNativeSidecarReview, JournalError> {
        let sidecar = self.sidecar.take().unwrap();
        self.host.begin_native_sidecar_review(self.host.revision(), sidecar, round(101), members, limits, snapshot())
    }
}
fn poll(case: &mut Case, run: &mut FileNativeSidecarReview, now: u64) -> Result<NativeReviewStatus, JournalError> {
    run.advance(&mut case.host, run.revision(), ElapsedTick(now), snapshot())
}
fn drive(case: &mut Case, run: &mut FileNativeSidecarReview) {
    for _ in 0..8192 {
        if run.status() != NativeReviewStatus::Running { return; }
        poll(case, run, 1).unwrap();
    }
    panic!("native round exceeded the declared test envelope");
}
fn publish(case: &mut Case, run: &FileNativeSidecarReview) {
    let input = run.input();
    let automatic = case.host.authorize(case.host.revision(), 1, input, snapshot()).unwrap();
    let request = case.host.request_human_approval(case.host.revision(), 1001, 1, input, ElapsedTick(40)).unwrap();
    let revision = case.host.revision(); let human = case.human.approve(&mut case.host, revision, &request).unwrap();
    case.host.dispatch(case.host.revision(), &automatic, &human, &case.action, input, snapshot()).unwrap();
    assert!(case.host.publish(case.host.revision(), 1).is_err());
    let receipt = case.host.publish_checked(case.host.revision(), 1, Some(input), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(receipt.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    case.host.reconcile(case.host.revision(), 1).unwrap();
    assert_eq!(case.host.inspect().payload, b"visible"); assert_eq!(case.host.inspect().executions, 1);
}

#[test]
fn actual_native_roster_yields_between_tokens_and_needs_both_original_publication_keys() {
    let mut case = Case::new(1, false); let members = case.members(b"allow", 0, false);
    let cost = NativeReviewCost::required(&members).unwrap();
    let mut run = case.start(members, NativeReviewLimits { native: cost, ..NativeReviewLimits::default() }).unwrap();
    let numerical = case.host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(run.reservation(), cost);
    assert!(case.host.authorize(case.host.revision(), 1, run.input(), snapshot()).is_err());
    assert_eq!(case.host.commit_review(case.host.revision(), 101, "alpha", 0).err(), Some(JournalError::Contract(Error::WrongState)));
    poll(&mut case, &mut run, 1).unwrap();
    assert!(run.records().values().all(|r| r.progress.completed_probes == 0 && !r.commitment_queued));
    for completed in 1..=4 {
        poll(&mut case, &mut run, 1).unwrap();
        for record in run.records().values() {
            assert_eq!(record.progress.completed_probes, completed);
            assert_eq!(record.progress.native.work.position, 0);
            assert!(!record.commitment_queued);
        }
    }
    assert!(run.records().values().all(|r| r.progress.native_started));
    for position in 1..=3 {
        poll(&mut case, &mut run, 1).unwrap();
        assert!(run.records().values().all(|r| r.progress.native.work.position == position && !r.commitment_queued));
    }
    drive(&mut case, &mut run);
    assert_eq!(run.status(), NativeReviewStatus::Finished);
    for record in run.records().values() {
        assert_eq!(record.progress.basis, Some(SidecarDecisionBasis::NativeModel));
        assert_eq!(record.progress.status, SidecarEvaluationStatus::Judged(Verdict::Allow));
        assert!(record.commitment_queued && record.reveal_queued && record.failure.is_none());
        assert_eq!(record.progress.native.reviewed_prompt_tokens, record.progress.native.requested_prompt_tokens);
        assert_eq!(record.progress.native.work.sampled_draws, 2);
    }
    assert!(matches!(run.outcome(), Some(FileLearnedSidecarFinish::Applied { receipt: Ok(_), .. })));
    assert_eq!(case.host.learned_generation_inspection().unwrap().numerical, numerical);
    let archived = Host::read_learned_sidecar_outcome(case.root.store(), &case.profile, &case.config, 101).unwrap();
    assert_eq!(archived.result.archive(), run.outcome().unwrap().archive());
    publish(&mut case, &run);
    assert_eq!(case.host.inspect().control.ledger.charged, 16);
}

#[test]
fn quiet_activation_probes_do_not_overrule_a_real_native_denial_from_changed_question_bytes() {
    for denied in [false, true] {
        let mut case = Case::new(1, denied); let members = case.members(b"allow", 0, false);
        let mut run = case.start(members, NativeReviewLimits::default()).unwrap(); drive(&mut case, &mut run);
        assert_eq!(run.records()["alpha"].progress.status, SidecarEvaluationStatus::Judged(Verdict::Allow));
        assert_eq!(run.records()["beta"].progress.status, SidecarEvaluationStatus::Judged(if denied { Verdict::Deny } else { Verdict::Allow }));
        assert!(run.records().values().all(|r| r.progress.basis == Some(SidecarDecisionBasis::NativeModel)));
        assert_eq!(case.host.authorize(case.host.revision(), 1, run.input(), snapshot()).is_ok(), !denied);
        assert_eq!(case.host.inspect().executions, 0);
    }
}

#[test]
fn numerical_alarm_or_uncertainty_prevents_native_inference_and_cannot_become_allow() {
    for (mode, verdict) in [(1, Verdict::Abstain), (2, Verdict::Hold)] {
        let mut case = Case::new(2, false); let members = case.members(b"allow", mode, false);
        let mut run = case.start(members, NativeReviewLimits::default()).unwrap(); drive(&mut case, &mut run);
        assert!(run.records().values().all(|r| r.progress.status == SidecarEvaluationStatus::Judged(verdict)
            && !r.progress.native_started && r.progress.native.work.position == 0));
        assert!(case.host.authorize(case.host.revision(), 1, run.input(), snapshot()).is_err());
        assert_eq!(case.host.retained_learned_sidecar(1).unwrap().packet.work().rounds, 1);
        assert_eq!(case.host.inspect().executions, 0);
    }
}

#[test]
fn malformed_native_output_and_native_monitor_holds_remain_missing_not_synthetic_votes() {
    for (spelling, alarm) in [(b"bad".as_slice(), false), (b"allow".as_slice(), true)] {
        let mut case = Case::new(1, false); let members = case.members(spelling, 0, alarm);
        let mut run = case.start(members, NativeReviewLimits::default()).unwrap();
        for _ in 0..8192 {
            if run.records().values().all(|r| r.failure.is_some()) { break; }
            poll(&mut case, &mut run, 1).unwrap();
        }
        assert!(run.records().values().all(|r| matches!(r.failure, Some(NativeMemberFailure::Evaluation(_)))
            && r.progress.native.work.position > 0 && !r.commitment_queued && !r.reveal_queued));
        let records = run.records().clone();
        poll(&mut case, &mut run, 30).unwrap();
        assert_eq!(run.status(), NativeReviewStatus::Finished); assert_eq!(run.records(), &records);
        assert_eq!(run.outcome().unwrap().archive().reveal_times.len(), 0);
        assert!(case.host.authorize(case.host.revision(), 1, run.input(), snapshot()).is_err());
        assert_eq!(case.host.inspect().executions, 0);
    }
}

#[test]
fn entire_native_allowance_and_complete_profiles_are_admitted_before_durable_begin() {
    for change in 0..7 {
        let mut case = Case::new(1, false); let mut members = case.members(b"allow", 0, false);
        let cost = NativeReviewCost::required(&members).unwrap();
        let mut limits = NativeReviewLimits { native: cost, ..NativeReviewLimits::default() };
        match change {
            0 => limits.native.evaluations -= 1,
            1 => limits.native.scalar_products -= 1,
            2 => limits.native.sampling_entries -= 1,
            3 => { members.get_mut("beta").unwrap().queries.pop(); }
            4 => { members.get_mut("beta").unwrap().salt.truncate(15); }
            5 => {
                let mut wrong = members["beta"].evaluator.policy().input_profile.clone(); wrong.policy_epoch += 1;
                members.get_mut("beta").unwrap().evaluator = native_model::helper(wrong, b"allow", false);
            }
            6 => limits.receive.request_bytes = 1,
            _ => unreachable!(),
        }
        let before = case.root.bytes(); let revision = case.host.revision();
        assert!(case.start(members, limits).is_err(), "change {change}");
        assert_eq!(case.host.revision(), revision); assert_eq!(case.root.bytes(), before);
    }
    let mut case = Case::new(1, false); let members = case.members(b"allow", 0, false);
    let native = NativeReviewCost::required(&members).unwrap();
    let mut control = case.start(members, NativeReviewLimits { native, ..NativeReviewLimits::default() }).unwrap();
    drive(&mut case, &mut control); assert_eq!(control.status(), NativeReviewStatus::Finished);
}

#[test]
fn cancellation_and_source_change_preserve_partial_model_work_and_stop_the_owned_round() {
    for change in [false, true] {
        let mut case = Case::new(1, false); let members = case.members(b"allow", 0, false);
        let mut run = case.start(members, NativeReviewLimits::default()).unwrap();
        for _ in 0..8 { poll(&mut case, &mut run, 1).unwrap(); }
        let before = run.records().clone(); assert_eq!(before["alpha"].progress.native.work.position, 3);
        if change {
            step(&mut case.host); assert!(poll(&mut case, &mut run, 1).is_err());
            assert_eq!(run.status(), NativeReviewStatus::Failed);
        } else {
            run.cancel(run.revision()).unwrap(); assert_eq!(run.status(), NativeReviewStatus::Cancelled);
        }
        for (member, record) in run.records() {
            assert_eq!(record.progress.native.work, before[member].progress.native.work);
            assert!(!record.commitment_queued && !record.reveal_queued);
        }
        let records = run.records().clone(); let revision = case.host.revision();
        assert!(poll(&mut case, &mut run, 1).is_err()); assert_eq!(run.records(), &records);
        assert_eq!(case.host.revision(), revision);
        assert_eq!(case.host.commit_review(case.host.revision(), 101, "alpha", 0).err(), Some(JournalError::Contract(Error::WrongState)));
        assert_eq!(case.host.inspect().executions, 0);
    }
}

#[test]
fn stale_and_foreign_calls_do_not_consume_work_and_expired_members_never_start_models() {
    let mut case = Case::new(1, false); let mut foreign = Case::new(1, false);
    let members = case.members(b"allow", 0, false);
    let mut run = case.start(members, NativeReviewLimits::default()).unwrap();
    let before = run.records().clone();
    assert_eq!(run.advance(&mut foreign.host, 0, ElapsedTick(1), snapshot()).err(), Some(JournalError::Contract(Error::Binding)));
    assert_eq!(run.advance(&mut case.host, 1, ElapsedTick(1), snapshot()).err(), Some(JournalError::Contract(Error::Stale)));
    assert_eq!(run.records(), &before); assert_eq!(run.revision(), 0); assert_eq!(run.polls(), 0);
    poll(&mut case, &mut run, 30).unwrap(); assert_eq!(run.status(), NativeReviewStatus::Finished);
    assert!(run.records().values().all(|r| !r.progress.native_started && r.progress.native.work.position == 0));
    assert_eq!(run.outcome().unwrap().archive().commit_times.len(), 0);
    assert!(case.host.authorize(case.host.revision(), 1, run.input(), snapshot()).is_err());
}

#[test]
fn poll_exhaustion_and_journal_failure_never_release_a_candidate_review() {
    for disk_failure in [false, true] {
        let mut case = Case::new(1, false); let members = case.members(b"allow", 0, false);
        let limits = NativeReviewLimits { polls: if disk_failure { 65536 } else { 1 }, ..NativeReviewLimits::default() };
        let mut run = case.start(members, limits).unwrap();
        if disk_failure {
            for _ in 0..8192 {
                if run.records().values().all(|r| matches!(r.progress.status, SidecarEvaluationStatus::Judged(_))) { break; }
                poll(&mut case, &mut run, 1).unwrap();
            }
            assert!(run.records().values().all(|r| !r.commitment_queued));
            std::fs::write(case.root.store().join("delivery.pending"), b"unacknowledged").unwrap();
        } else { poll(&mut case, &mut run, 1).unwrap(); }
        let before = case.root.bytes();
        assert!(poll(&mut case, &mut run, 1).is_err()); assert_eq!(run.status(), NativeReviewStatus::Failed);
        assert!(run.outcome().is_none()); assert_eq!(case.root.bytes(), before);
        assert!(poll(&mut case, &mut run, 1).is_err()); assert_eq!(case.host.inspect().executions, 0);
    }
}
