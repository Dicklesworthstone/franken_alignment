//! Real canonical files, original role custody and causal storage barriers.
use super::*;
use super::super::super::FileRecoveryFloor;
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalIo, JournalLimits, RecoveryReserve};
use crate::action::consequence::delivery::persistent::observed::credibility::FileCredibilityUpdate;
use crate::action::consequence::delivery::persistent::observed::source::FileSourcePolicy;
use crate::action::consequence::delivery::EndpointOutcome;
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, ReviewWindow};
use crate::action::consequence::oversight::credibility::{Assessment, Fraction, GroundTruth};
use crate::action::consequence::oversight::policy_state::{StateFreshness, StateLimits, StateSource};
use crate::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot};
use crate::action::consequence::oversight::human::{HumanDisposition, HumanReviewPolicy};
use crate::action::consequence::oversight::joint_credibility::JointReplayBudget;
use crate::action::{ActionSpec, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::round::{Verdict, commitment};
use crate::Snapshot;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync];
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        Self(std::env::temp_dir().join(format!("fa-joint-roles-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))))
    }
    fn bytes(&self) -> Vec<u8> { std::fs::read(self.0.join(storage::CANONICAL)).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if self.0.exists() && let Err(error) = std::fs::remove_dir_all(&self.0) {
            eprintln!("joint role fixture cleanup: {error}");
        }
    }
}
fn fraction(numerator: u64, denominator: u64) -> Fraction { Fraction { numerator, denominator } }
fn protocol() -> EvaluationProtocol {
    EvaluationProtocol { domain: 1, stratum: 2, period: 3,
        minimum_violation_origins: 1, minimum_benign_origins: 1,
        precision_floor: fraction(1, 1), recall_floor: fraction(1, 1),
        false_positive_ceiling: fraction(0, 1), false_stop_budget: 10 }
}
fn joint() -> JointPromotionPolicy {
    JointPromotionPolicy::new(71, 1, fraction(0, 1), fraction(0, 1), JointReplayBudget::default()).unwrap()
}
fn guards() -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
        identity: None, campaigns: None, credential: None }
}
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    let scope = Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect };
    let actor = ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
        model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
        grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap();
    FileOversightProfile {
        delivery: FileDeliveryProfile { scope, total: 100, max_attempts: 16, actor,
            suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
            congress: CongressPolicy { generation: 1,
                members: BTreeMap::from([("a".to_owned(), MemberPolicy { cohort: "a".into(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1,
                continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 16, clock_domain: 99, limits: JournalLimits::default() },
        committee: CommitteeContract::new(BTreeMap::from([("a".to_owned(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: vec![], model_epoch: 1, tokenizer_epoch: 1, policy_epoch: 0 },
            1, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 16 },
    }
}
fn expected(host: &FileOversight) -> FileRecoveryRequirements {
    let state = host.inspect();
    FileRecoveryRequirements { guards: guards(), effective_policy: profile().delivery.policy,
        credential_epoch: None, minimum: FileRecoveryFloor { journal_revision: state.revision,
            control_sequence: state.control.sequence, authority_epoch: state.control.ledger.epoch } }
}
fn create(root: &Directory) -> (FileOversight, FileEvaluatedOversightRoles) {
    FileOversight::create_jointly_evaluated_guarded(&root.0, profile(), &guards(), None, protocol(), joint()).unwrap()
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn review(host: &mut FileOversight, id: u64, verdict: Verdict) -> (FrozenAction, CommitteeInput) {
    host.observe_time(host.revision(), ElapsedTick(id * 4)).unwrap();
    let p = profile();
    let action = host.propose(host.revision(), id, ActionSpec { version: VERSION,
        scope: p.delivery.scope, target: Some(host.inspect().target), payload: b"visible".to_vec(),
        required_witnesses: vec![], policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(1000), units: 16 }, snapshot()).unwrap();
    let source = EvidenceSnapshot::new(EvidenceIdentity { source: 1, generation: id, scope: p.delivery.scope },
        snapshot(), BTreeMap::from([("a".into(), b"context".to_vec())])).unwrap();
    let inputs = source.inputs_for(&action, &p.committee).unwrap();
    host.record_inputs(host.revision(), id, 0, inputs.clone()).unwrap();
    host.begin_review(host.revision(), id, id + 100, [id as u8; 32], ReviewWindow {
        commit_by: ElapsedTick(id * 4 + 1), reveal_by: ElapsedTick(id * 4 + 2) }, snapshot()).unwrap();
    host.commit_review(host.revision(), id + 100, "a", commitment(id + 100, "a", &[id as u8; 32], verdict, b"salt").unwrap()).unwrap();
    host.open_reveals(host.revision(), id + 100).unwrap();
    host.reveal_review(host.revision(), id + 100, "a", verdict, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), id + 100, Some(&inputs), snapshot()).unwrap().unwrap();
    (action, inputs)
}
fn label(origin: u64, truth: GroundTruth) -> Assessment { Assessment { origin, evidence_id: [2; 32], truth } }

#[test]
fn first_image_is_fully_guarded_and_read_only_inspection_needs_no_owner_lock() {
    let root = Directory::new();
    let (mut host, _) = create(&root);
    assert_eq!(host.revision(), 2);
    assert!(host.publication_guard_required());
    assert_eq!(host.joint_credibility_policy().unwrap(), Some(joint()));
    let bytes = root.bytes();
    let image = FileOversight::read_joint_credibility(&root.0, &profile(), &expected(&host), &protocol(), joint()).unwrap();
    assert_eq!(image.credibility.journal, host.inspect());
    assert!(!image.credibility.report.qualified());
    assert!(image.promotions.is_empty());
    assert_eq!(root.bytes(), bytes);
    // Explicit setup remains bootstrap, not work stealing terminal capacity.
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    assert_eq!(host.journal_capacity().unwrap().reserve(), Some(RecoveryReserve::terminal()));
}

#[test]
fn recovered_evaluator_resolves_censoring_without_restoring_old_roles_or_keys() {
    let root = Directory::new();
    let (mut host, roles) = create(&root);
    review(&mut host, 1, Verdict::Hold);
    let old_ticket = host.evaluation_ticket(101).unwrap();
    let revision = host.revision();
    roles.evaluator.assess(&mut host, revision, &old_ticket, label(1, GroundTruth::Censored)).unwrap();
    let (_, inputs) = review(&mut host, 2, Verdict::Allow);
    let ticket = host.evaluation_ticket(102).unwrap();
    let revision = host.revision();
    roles.evaluator.assess(&mut host, revision, &ticket, label(2, GroundTruth::Benign)).unwrap();
    let old_automatic = host.authorize(host.revision(), 2, &inputs, snapshot()).unwrap();
    let old_request = host.request_human_approval(host.revision(), 1002, 2, &inputs, ElapsedTick(30)).unwrap();
    let revision = host.revision();
    let old_human = roles.oversight.human.approve(&mut host, revision, &old_request).unwrap();
    let requirements = expected(&host);
    drop(host);
    let (mut host, fresh) = FileOversight::open_jointly_evaluated_guarded(
        &root.0, profile(), &requirements, &protocol(), joint()).unwrap();
    assert!(!host.clock_ready());
    assert_eq!(host.human_status(1002).unwrap().disposition, HumanDisposition::Revoked);
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(host.credibility_report().unwrap().censored_cases, 1);
    let revision = host.revision();
    assert_eq!(roles.evaluator.assess(&mut host, revision, &old_ticket, label(1, GroundTruth::Violation)), Err(Error::Binding.into()));
    assert_eq!(fresh.evaluator.assess(&mut host, revision, &old_ticket, label(1, GroundTruth::Violation)), Err(Error::Binding.into()));
    assert!(host.dispatch(host.revision(), &old_automatic, &old_human, inputs.action(), &inputs, snapshot()).is_err());
    let ticket = host.evaluation_ticket(101).unwrap();
    let revision = host.revision();
    fresh.evaluator.assess(&mut host, revision, &ticket, label(1, GroundTruth::Violation)).unwrap();
    assert_eq!(host.evaluation_history(101).unwrap(), &[label(1, GroundTruth::Censored), label(1, GroundTruth::Violation)]);
    assert!(host.credibility_report().unwrap().qualified());
    host.observe_time(host.revision(), ElapsedTick(10)).unwrap();
    let control = host.inspect().control;
    let update = FileCredibilityUpdate { operation: 41, expected_control_sequence: control.sequence,
        expected_authority_epoch: control.ledger.epoch, expected_evaluation_revision: host.credibility_report().unwrap().revision };
    host.promote_credibility(host.revision(), &update).unwrap();
    assert!(host.joint_credibility_promotion(41).unwrap().qualified());
    let (action, inputs) = review(&mut host, 3, Verdict::Allow);
    let automatic = host.authorize(host.revision(), 3, &inputs, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), 1003, 3, &inputs, ElapsedTick(30)).unwrap();
    let revision = host.revision();
    let human = fresh.oversight.human.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    assert_eq!(host.publish_checked(host.revision(), 3, Some(&inputs), snapshot(), ElapsedTick(13)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
}

#[test]
fn incorrect_policy_protocol_guards_or_floors_refuse_before_cleanup_and_fencing() {
    let root = Directory::new();
    let (host, _) = create(&root);
    let requirements = expected(&host);
    let bytes = root.bytes();
    drop(host);
    let pending = root.0.join("delivery.pending");
    std::fs::write(&pending, b"unacknowledged stage").unwrap();
    let changed = [
        JointPromotionPolicy::new(72, 1, fraction(0, 1), fraction(0, 1), JointReplayBudget::default()).unwrap(),
        JointPromotionPolicy::new(71, 2, fraction(0, 1), fraction(0, 1), JointReplayBudget::default()).unwrap(),
        JointPromotionPolicy::new(71, 1, fraction(1, 1), fraction(0, 1), JointReplayBudget::default()).unwrap(),
        JointPromotionPolicy::new(71, 1, fraction(0, 1), fraction(1, 1), JointReplayBudget::default()).unwrap(),
        JointPromotionPolicy::new(71, 1, fraction(0, 1), fraction(0, 1), JointReplayBudget { cases: 1, member_outcomes: 2 }).unwrap(),
    ];
    for policy in changed {
        assert!(FileOversight::open_jointly_evaluated_guarded(&root.0, profile(), &requirements, &protocol(), policy).is_err());
        assert_eq!(root.bytes(), bytes);
        assert_eq!(std::fs::read(&pending).unwrap(), b"unacknowledged stage");
    }
    let mut p = protocol(); p.false_stop_budget += 1;
    assert!(FileOversight::open_jointly_evaluated_guarded(&root.0, profile(), &requirements, &p, joint()).is_err());
    let mut newer = requirements.clone(); newer.minimum.authority_epoch += 1;
    assert!(matches!(FileOversight::open_jointly_evaluated_guarded(&root.0, profile(), &newer, &protocol(), joint()),
        Err(JournalError::Contract(Error::Stale))));
    let mut wrong = requirements.clone(); wrong.effective_policy = Policy::new(2, vec![Predicate::PayloadAtMost(128)]).unwrap();
    assert!(FileOversight::open_jointly_evaluated_guarded(&root.0, profile(), &wrong, &protocol(), joint()).is_err());
    let mut wrong_guard = requirements.clone();
    wrong_guard.guards.source = Some(FileSourcePolicy {
        source: StateSource { scope: profile().delivery.scope, source: 51, generation: 1 },
        limits: StateLimits { events: 8, retained_bytes: 4096 },
        freshness: StateFreshness::new(100).unwrap(),
    });
    assert!(FileOversight::open_jointly_evaluated_guarded(&root.0, profile(), &wrong_guard, &protocol(), joint()).is_err());
    assert_eq!(root.bytes(), bytes);
    assert!(pending.exists());
    let (host, _) = FileOversight::open_jointly_evaluated_guarded(&root.0, profile(), &requirements, &protocol(), joint()).unwrap();
    assert_eq!(host.revision(), requirements.minimum.journal_revision + 1);
    assert!(!pending.exists());
}

#[test]
fn marginal_and_joint_pinned_openers_never_substitute_for_each_other() {
    let joint_root = Directory::new();
    let (host, _) = create(&joint_root);
    let requirements = expected(&host);
    let bytes = joint_root.bytes();
    drop(host);
    assert!(FileOversight::open_evaluated_guarded(&joint_root.0, profile(), &requirements, &protocol()).is_err());
    assert_eq!(joint_root.bytes(), bytes);
    let legacy = Directory::new();
    let (host, _) = FileOversight::create_evaluated_guarded(&legacy.0, profile(), &guards(), None, protocol()).unwrap();
    let requirements = expected(&host);
    let bytes = legacy.bytes();
    drop(host);
    assert!(FileOversight::open_jointly_evaluated_guarded(&legacy.0, profile(), &requirements, &protocol(), joint()).is_err());
    assert_eq!(legacy.bytes(), bytes);
    let (host, _) = FileOversight::open_evaluated_guarded(&legacy.0, profile(), &requirements, &protocol()).unwrap();
    assert_eq!(host.joint_credibility_policy().unwrap(), None);
}

#[test]
fn invalid_protocol_is_refused_before_creating_any_storage_or_role() {
    let root = Directory::new();
    let mut invalid = protocol(); invalid.domain = 0;
    assert!(FileOversight::create_jointly_evaluated_guarded(&root.0, profile(), &guards(), None, invalid, joint()).is_err());
    assert!(!root.0.exists());
    let (host, _) = create(&root);
    assert_eq!(host.joint_credibility_policy().unwrap(), Some(joint()));
}

#[test]
fn creation_and_recovery_storage_failures_return_no_owner_or_evaluator() {
    for stage in BARRIERS {
        let root = Directory::new();
        let prepared = PreparedGuardedBootstrap::prepare(profile(), &guards(), None).unwrap()
            .jointly_evaluated(protocol(), joint()).unwrap();
        let store = storage::Store::create(&root.0).unwrap();
        store.fail_once(stage);
        assert!(matches!(prepared.publish(store), Err(JournalError::Io(_))));
        assert_eq!(root.0.join(storage::CANONICAL).exists(), stage == JournalIo::DirectorySync);

        let root = Directory::new();
        let (host, _) = create(&root);
        let requirements = expected(&host);
        let before = host.inspect();
        drop(host);
        let store = storage::Store::open(&root.0).unwrap();
        store.fail_once(stage);
        assert!(matches!(FileOversight::open_jointly_evaluated_guarded_store(
            store, profile(), &requirements, &protocol(), joint()), Err(JournalError::Io(_))));
        let historical = FileOversight::read_joint_credibility(&root.0, &profile(), &requirements, &protocol(), joint()).unwrap();
        assert_eq!(historical.credibility.journal.revision,
            before.revision + u64::from(stage == JournalIo::DirectorySync));
        let (host, _) = FileOversight::open_jointly_evaluated_guarded(&root.0, profile(), &requirements, &protocol(), joint()).unwrap();
        assert!(!host.clock_ready());
        assert_eq!(host.joint_credibility_policy().unwrap(), Some(joint()));
        assert_eq!(host.inspect().control.ledger.available, 100);
        assert_eq!(host.inspect().executions, 0);
    }
}
