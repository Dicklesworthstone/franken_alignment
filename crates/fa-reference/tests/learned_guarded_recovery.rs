//! Real original numerics, local journals and role-bound control transitions.
//! Synthetic probes/ballots are controls, not detector or deployment evidence.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[path = "support/restart_model.rs"]
pub mod numerical;

use fa_reference::{Error, Snapshot};
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
use fa_reference::action::consequence::activation::identity::{IdentityAnchor, ModelManifest, ModelPassport};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{SamplingPolicy, SamplingStart};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::monitored::{
    GenerationBudget, GenerationSpec, GenerationStatus, GenerationTelemetryBudget,
};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{EndpointOutcome, persistent::{
    FileDeliveryProfile, FilePermit, JournalError, JournalLimits, Reconciliation,
    observed::{FileHumanRequest, FileOversight, FileOversightProfile,
        decoder::learned::{FileLearnedConfig, LearnedStepIntent},
        guarded::{FileCampaignRequirement, FileGuardSet, FileIdentityRequirement,
            FileOversightRoles, FileRecoveryFloor, FileRecoveryRequirements}},
}};
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
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-learned-guarded-{}-{time}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
    fn bytes(&self) -> Vec<u8> { std::fs::read(self.store().join("delivery.bin")).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("guarded learned cleanup: {error}"); }
    }
}
fn source(mode: u8) -> LearnedSourceConfig {
    let model = numerical::model();
    LearnedSourceConfig { stream: 21, evaluation_origin: 201, monitor_generation: 31,
        spec: GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
            policy: SamplingPolicy::new(1, 1, 3, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173,
        }).unwrap(), policy: numerical::policy(&model, mode, 1),
        budget: GenerationBudget::default(), telemetry: GenerationTelemetryBudget::default() }
}
fn config(mode: u8) -> FileLearnedConfig {
    FileLearnedConfig::new(numerical::model(), source(mode), LearnedDecoderBindingLimits::default()).unwrap()
}
fn profile() -> FileOversightProfile {
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
fn guards() -> FileGuardSet {
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
fn requirements(host: &FileOversight, guards: FileGuardSet) -> FileRecoveryRequirements {
    let control = host.inspect().control;
    FileRecoveryRequirements { guards, effective_policy: profile().delivery.policy, credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: host.revision(), control_sequence: control.sequence,
            authority_epoch: control.ledger.epoch } }
}
fn create(root: &Directory, config: &FileLearnedConfig) -> (FileOversight, FileOversightRoles) {
    FileOversight::create_guarded_with_learned_generation(root.store(), profile(), &guards(), None, config.clone()).unwrap()
}
fn step(host: &mut FileOversight) -> Result<GenerationStatus, JournalError> {
    let n = host.learned_generation_inspection()?.numerical;
    host.advance_learned_generation(host.revision(), n.actor_revision, n.position)?
        .map(|event| event.status()).map_err(Into::into)
}
fn resume(host: &mut FileOversight, tick: u64) {
    host.observe_time(host.revision(), ElapsedTick(tick)).unwrap();
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
}
fn fresh_identity(host: &mut FileOversight, roles: &FileOversightRoles, id: u64, tick: u64) {
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
        sequence: 1, position: 0 }, &[0.0]).unwrap();
    let revision = host.revision();
    observer.observe_anchor(host, revision, &check, 10, &frame, ElapsedTick(tick)).unwrap().measurement.unwrap();
    host.apply_identity_check(host.revision(), &check, sequence, epoch).unwrap();
    assert_ne!(host.identity_status().unwrap(), IdentityStatus::Missing);
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn spec(host: &FileOversight) -> ActionSpec {
    ActionSpec { version: VERSION, scope: profile().delivery.scope, target: Some(host.inspect().target),
        payload: b"visible".to_vec(), required_witnesses: Vec::new(), policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(100), units: 16 }
}
fn prepared(host: &mut FileOversight, attempt: u64, round: u64)
    -> (fa_reference::action::FrozenAction, CommitteeInput, FilePermit, FileHumanRequest)
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
    host.begin_review(host.revision(), attempt, round, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(10), reveal_by: ElapsedTick(20) }, snapshot()).unwrap();
    let salt = b"independent-fixture-salt";
    host.commit_review(host.revision(), round, "reviewer", commitment(round, "reviewer", &[9; 32], Verdict::Allow, salt).unwrap()).unwrap();
    host.open_reveals(host.revision(), round).unwrap();
    host.reveal_review(host.revision(), round, "reviewer", Verdict::Allow, salt.to_vec()).unwrap();
    host.finish_review(host.revision(), round, Some(&input), snapshot()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), attempt, &input, snapshot()).unwrap();
    let human = host.request_human_approval(host.revision(), 1000 + attempt, attempt, &input, ElapsedTick(30)).unwrap();
    (action, input, automatic, human)
}

#[test]
fn first_image_has_the_complete_guard_set_without_computing_or_authorizing() {
    let root = Directory::new(); let config = config(0);
    let (host, roles) = create(&root, &config);
    assert_eq!(host.revision(), 4);
    assert!(host.publication_guard_required() && host.learned_generation_required());
    assert!(host.identity_checks_required() && host.policy_campaigns_required());
    assert!(roles.identity_observer.is_some() && roles.policy_governor.is_some());
    assert!(!host.clock_ready());
    let n = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(n.position, 0); assert_eq!(n.work.admitted_tokens, 0);
    assert_eq!(host.inspect().executions, 0);
    let expected = requirements(&host, guards()); let original = root.bytes(); drop(host);
    assert!(FileOversight::open_guarded(root.store(), profile(), &expected).is_err());
    assert_eq!(root.bytes(), original);
    let (host, roles) = FileOversight::open_guarded_with_learned_generation(root.store(), profile(), &expected, &config).unwrap();
    assert_eq!(host.revision(), expected.minimum.journal_revision + 1);
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
    assert!(roles.identity_observer.is_some() && roles.policy_governor.is_some());
}

#[test]
fn recovered_roles_enable_fresh_review_but_do_not_revive_old_keys_or_identity() {
    let root = Directory::new(); let config = config(0);
    let (mut host, old_roles) = create(&root, &config);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host).unwrap();
    fresh_identity(&mut host, &old_roles, 1, 1);
    let (old_action, old_input, old_key, old_request) = prepared(&mut host, 1, 101);
    let revision = host.revision(); let old_human = old_roles.human.approve(&mut host, revision, &old_request).unwrap();
    let expected = requirements(&host, guards());
    let n = host.learned_generation_inspection().unwrap().numerical; drop(host);
    let (mut host, roles) = FileOversight::open_guarded_with_learned_generation(root.store(), profile(), &expected, &config).unwrap();
    assert_eq!(host.revision(), expected.minimum.journal_revision + 1);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
    assert!(!host.clock_ready());
    assert_eq!(host.identity_status(), Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    assert!(host.dispatch(host.revision(), &old_key, &old_human, &old_action, &old_input, snapshot()).is_err());
    assert!(host.resume_learned_generation(host.revision(), n.actor_revision, n.position).is_err());
    resume(&mut host, 2);
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
    assert!(host.propose(host.revision(), 2, spec(&host), snapshot()).is_err());
    fresh_identity(&mut host, &roles, 2, 2);
    let (action, input, key, request) = prepared(&mut host, 2, 102);
    let before = host.revision();
    assert!(old_roles.human.approve(&mut host, before, &request).is_err());
    assert_eq!(host.revision(), before);
    let human = roles.human.approve(&mut host, before, &request).unwrap();
    host.dispatch(host.revision(), &key, &human, &action, &input, snapshot()).unwrap();
    assert!(host.publish(host.revision(), 2).is_err());
    let published = host.publish_checked(host.revision(), 2, Some(&input), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(published.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 2).unwrap();
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().payload, b"visible");
    assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn wrong_guards_policy_recipe_and_each_floor_refuse_before_cleanup_or_writes() {
    let root = Directory::new(); let config = config(0);
    let (mut host, _) = create(&root, &config);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host).unwrap();
    let expected = requirements(&host, guards()); let bytes = root.bytes(); drop(host);
    let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"unacknowledged").unwrap();
    for field in 0..7 {
        let mut changed = expected.clone();
        match field {
            0 => changed.guards.identity = None,
            1 => changed.guards.campaigns = None,
            2 => changed.guards.identity.as_mut().unwrap().policy.validity_ticks += 1,
            3 => changed.minimum.journal_revision += 1,
            4 => changed.minimum.control_sequence += 1,
            5 => changed.minimum.authority_epoch += 1,
            6 => changed.effective_policy = Policy::new(2, vec![Predicate::PayloadAtMost(128)]).unwrap(),
            _ => unreachable!(),
        }
        assert!(FileOversight::open_guarded_with_learned_generation(root.store(), profile(), &changed, &config).is_err(), "field {field}");
        assert_eq!(root.bytes(), bytes); assert_eq!(std::fs::read(&pending).unwrap(), b"unacknowledged");
    }
    let mut changed_source = source(0); changed_source.telemetry.source_check_values -= 1;
    let wrong = FileLearnedConfig::new(numerical::model(), changed_source, LearnedDecoderBindingLimits::default()).unwrap();
    assert_ne!(wrong, config);
    assert_eq!(FileOversight::open_guarded_with_learned_generation(root.store(), profile(), &expected, &wrong).err(),
        Some(JournalError::Contract(Error::Binding)));
    assert_eq!(root.bytes(), bytes); assert!(pending.exists());
    let (host, _) = FileOversight::open_guarded_with_learned_generation(root.store(), profile(), &expected, &config).unwrap();
    assert_eq!(host.revision(), expected.minimum.journal_revision + 1); assert!(!pending.exists());
}

#[test]
fn every_role_presence_combination_is_preserved_in_the_first_image_and_recovery() {
    for mask in 0..4 {
        let root = Directory::new(); let config = config(0); let mut g = guards();
        if mask & 1 == 0 { g.identity = None; }
        if mask & 2 == 0 { g.campaigns = None; }
        let (host, roles) = FileOversight::create_guarded_with_learned_generation(root.store(), profile(), &g, None, config.clone()).unwrap();
        assert_eq!(roles.identity_observer.is_some(), mask & 1 != 0);
        assert_eq!(roles.policy_governor.is_some(), mask & 2 != 0);
        let expected = requirements(&host, g); let before = host.revision(); drop(host);
        let (host, roles) = FileOversight::open_guarded_with_learned_generation(root.store(), profile(), &expected, &config).unwrap();
        assert_eq!(host.revision(), before + 1);
        assert_eq!(roles.identity_observer.is_some(), mask & 1 != 0);
        assert_eq!(roles.policy_governor.is_some(), mask & 2 != 0);
        assert!(!host.clock_ready());
    }
}

#[test]
fn pending_numerical_intent_survives_composed_recovery_and_requires_exact_completion() {
    let root = Directory::new(); let config = config(0);
    let (mut host, _) = create(&root, &config);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host).unwrap();
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
    let expected = requirements(&host, guards()); drop(host);
    let (mut host, roles) = FileOversight::open_guarded_with_learned_generation(root.store(), profile(), &expected, &config).unwrap();
    resume(&mut host, 2);
    let before = host.learned_generation_inspection().unwrap();
    assert_eq!(before.numerical, n);
    assert_eq!(before.pending, Some(LearnedStepIntent { actor_revision: n.actor_revision, position: n.position }));
    assert!(host.complete_learned_step(host.revision(), n.actor_revision, n.position + 1).is_err());
    assert!(host.propose(host.revision(), 1, spec(&host), snapshot()).is_err());
    assert_eq!(host.learned_generation_inspection().unwrap(), before);
    host.complete_learned_step(host.revision(), n.actor_revision, n.position).unwrap().unwrap();
    let after = host.learned_generation_inspection().unwrap();
    assert!(after.pending.is_none()); assert_eq!(after.numerical.position, n.position + 1);
    assert_eq!(after.numerical.sampled_draws, n.sampled_draws + 1);
    assert!(after.numerical.telemetry.source_check_values > n.telemetry.source_check_values);
    fresh_identity(&mut host, &roles, 1, 2);
    host.propose(host.revision(), 1, spec(&host), snapshot()).unwrap();
}

#[test]
fn held_and_budget_failed_sources_remain_nonresumable_after_guarded_recovery() {
    for budget_failure in [false, true] {
        let mut s = source(if budget_failure { 0 } else { 2 });
        if budget_failure {
            let mut original = numerical::model().observed_learned_generation(s.clone()).unwrap();
            original.advance(0).unwrap(); s.telemetry.source_check_values = original.telemetry_work().source_check_values;
        }
        let c = FileLearnedConfig::new(numerical::model(), s, LearnedDecoderBindingLimits::default()).unwrap();
        let root = Directory::new(); let (mut host, _) = create(&root, &c);
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host).unwrap();
        let result = step(&mut host);
        if budget_failure { assert_eq!(result, Err(JournalError::Contract(Error::Limit))); }
        else { assert!(matches!(result, Ok(GenerationStatus::Held(_)))); }
        let n = host.learned_generation_inspection().unwrap().numerical;
        let expected = requirements(&host, guards()); drop(host);
        let (mut host, _) = FileOversight::open_guarded_with_learned_generation(root.store(), profile(), &expected, &c).unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        assert!(host.resume_learned_generation(host.revision(), n.actor_revision, n.position).is_err());
        assert!(host.propose(host.revision(), 1, spec(&host), snapshot()).is_err());
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn dispatched_liability_is_not_refunded_when_composed_roles_and_source_are_recovered() {
    let root = Directory::new(); let c = config(0); let (mut host, roles) = create(&root, &c);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host).unwrap();
    fresh_identity(&mut host, &roles, 1, 1);
    let (action, input, key, request) = prepared(&mut host, 1, 101);
    let revision = host.revision(); let human = roles.human.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &key, &human, &action, &input, snapshot()).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 16);
    let expected = requirements(&host, guards()); drop(host);
    let (mut host, _) = FileOversight::open_guarded_with_learned_generation(root.store(), profile(), &expected, &c).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().executions, 0);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert!(host.dispatch(host.revision(), &key, &human, &action, &input, snapshot()).is_err());
}

#[test]
fn bootstrap_capacity_is_exact_and_never_publishes_a_partial_guard_set() {
    for events in [3, 4] {
        let root = Directory::new(); let c = config(0); let mut p = profile(); p.delivery.limits.events = events;
        let result = FileOversight::create_guarded_with_learned_generation(root.store(), p.clone(), &guards(), None, c.clone());
        if events == 3 {
            assert_eq!(result.err(), Some(JournalError::Contract(Error::Limit)));
            assert!(!root.store().join("delivery.bin").exists());
        } else {
            let (host, roles) = result.unwrap(); assert_eq!(host.revision(), 4);
            assert!(roles.identity_observer.is_some() && roles.policy_governor.is_some());
            let expected = requirements(&host, guards()); let bytes = root.bytes(); drop(host);
            // Even a valid image cannot expose roles without room for its fence.
            assert_eq!(FileOversight::open_guarded_with_learned_generation(root.store(), p, &expected, &c).err(),
                Some(JournalError::Contract(Error::Limit)));
            assert_eq!(root.bytes(), bytes);
        }
    }
}

#[test]
fn live_owner_lock_and_existing_directory_do_not_create_a_second_authority() {
    let root = Directory::new(); let c = config(0); let (host, _) = create(&root, &c);
    let expected = requirements(&host, guards()); let bytes = root.bytes();
    assert_eq!(FileOversight::open_guarded_with_learned_generation(root.store(), profile(), &expected, &c).err(), Some(JournalError::Busy));
    assert!(FileOversight::create_guarded_with_learned_generation(root.store(), profile(), &guards(), None, c).is_err());
    assert_eq!(root.bytes(), bytes); assert_eq!(host.revision(), 4); assert_eq!(host.inspect().executions, 0);
}

#[path = "learned_guarded_recovery/anchored.rs"]
mod anchored;
