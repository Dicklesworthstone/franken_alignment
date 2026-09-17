//! Original replacement barriers; no new storage seam or synthetic outcomes.
use super::*;
use super::super::{FileRecoveryFloor, FileGuardSet};
use crate::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalIo, JournalLimits};
use crate::action::consequence::delivery::persistent::observed::credibility::FileCredibilityUpdate;
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, ReviewWindow, action_frame};
use crate::action::consequence::oversight::credibility::{Assessment, Fraction, GroundTruth};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use crate::reducer::Caps;
use crate::round::{Verdict, commitment};
use crate::Snapshot;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync];
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let p = std::env::temp_dir().join(format!("fa-evaluation-{}-{now}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&p).unwrap(); Self(p)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory { fn drop(&mut self) { if let Err(e) = std::fs::remove_dir_all(&self.0) { eprintln!("evaluation cleanup: {e}"); } } }
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile { delivery: FileDeliveryProfile {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: 100, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1,
            tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".into(), MemberPolicy { cohort: "one".into(), weight: 1 })]),
            caps: Caps { per_member: 2, per_cohort: 2 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(), retention_ticks: 1000,
        max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
    }, committee: CommitteeContract::new(BTreeMap::from([("reviewer".into(), HelperContract::new(InputProfileBinding {
        profile_id: 1, profile_bytes: b"evaluation-test".to_vec(), tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1,
    }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 } }
}
fn guards() -> FileGuardSet { FileGuardSet { stream: None, decoder: None, decoder_stop: None,
    source: None, identity: None, campaigns: None, credential: None } }
fn protocol() -> EvaluationProtocol {
    EvaluationProtocol { domain: 1, stratum: 2, period: 3, minimum_violation_origins: 1, minimum_benign_origins: 1,
        precision_floor: Fraction { numerator: 1, denominator: 2 }, recall_floor: Fraction { numerator: 1, denominator: 2 },
        false_positive_ceiling: Fraction { numerator: 1, denominator: 2 }, false_stop_budget: 10 }
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn requirements(host: &FileOversight) -> FileRecoveryRequirements {
    let c = host.inspect().control;
    FileRecoveryRequirements { guards: guards(), effective_policy: profile().delivery.policy, credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: host.revision(), control_sequence: c.sequence, authority_epoch: c.ledger.epoch } }
}
fn case(host: &mut FileOversight, id: u64, verdict: Verdict) {
    let p = profile(); let round = 100 + id;
    let action = host.propose(host.revision(), id, ActionSpec { version: VERSION, scope: p.delivery.scope,
        target: Some(host.inspect().target), payload: b"reviewed".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 }, snapshot()).unwrap();
    let helper = &p.committee.members()["reviewer"];
    let mut bytes = action_frame(&action); let start = bytes.len(); bytes.extend_from_slice(helper.question()); let end = bytes.len();
    let actual = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: start } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start, end } }], Vec::new()).unwrap();
    let view = EvidenceViewManifest::new(actual, AuthorizationProjection { projection_id: 7,
        policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new() }, Vec::new()).unwrap();
    let inputs = CommitteeInput::capture(&action, &p.committee, BTreeMap::from([("reviewer".into(), view)])).unwrap();
    host.record_inputs(host.revision(), id, 0, inputs.clone()).unwrap();
    host.begin_review(host.revision(), id, round, [9; 32], ReviewWindow { commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) }, snapshot()).unwrap();
    let digest = commitment(round, "reviewer", &[9; 32], verdict, b"salt").unwrap();
    host.commit_review(host.revision(), round, "reviewer", digest).unwrap(); host.open_reveals(host.revision(), round).unwrap();
    host.reveal_review(host.revision(), round, "reviewer", verdict, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), round, Some(&inputs), snapshot()).unwrap().unwrap();
}
fn assess(host: &mut FileOversight, evaluator: &FileIndependentEvaluator, id: u64, truth: GroundTruth) -> Result<bool, JournalError> {
    let ticket = host.evaluation_ticket(id + 100)?; let revision = host.revision();
    evaluator.assess(host, revision, &ticket, Assessment { origin: id, evidence_id: [17; 32], truth })
}
fn owner(root: &Directory) -> (FileOversight, FileEvaluatedOversightRoles) {
    let (mut host, roles) = FileOversight::create_evaluated_guarded(root.store(), profile(), &guards(), None, protocol()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    case(&mut host, 1, Verdict::Hold); assess(&mut host, &roles.evaluator, 1, GroundTruth::Violation).unwrap();
    case(&mut host, 2, Verdict::Allow);
    (host, roles)
}
fn update(host: &FileOversight) -> FileCredibilityUpdate {
    let c = host.inspect().control;
    FileCredibilityUpdate { operation: 7, expected_control_sequence: c.sequence, expected_authority_epoch: c.ledger.epoch,
        expected_evaluation_revision: host.credibility_report().unwrap().revision }
}
fn fault(error: JournalError, stage: JournalIo) {
    let JournalError::Io(error) = error else { panic!("expected original replacement failure"); };
    assert_eq!(error.operation, stage);
    assert_eq!(error.replacement_may_be_visible, matches!(stage, JournalIo::Rename | JournalIo::DirectorySync));
}

#[test]
fn atomic_evaluation_bootstrap_never_returns_partial_roles_or_promotes_staging() {
    for stage in BARRIERS {
        let root = Directory::new();
        let prepared = PreparedGuardedBootstrap::prepare(profile(), &guards(), None).unwrap().evaluated(protocol()).unwrap();
        let store = storage::Store::create(&root.store()).unwrap(); store.fail_once(stage);
        fault(prepared.publish(store).unwrap_err(), stage);
        let expected = FileRecoveryRequirements { guards: guards(), effective_policy: profile().delivery.policy,
            credential_epoch: None, minimum: FileRecoveryFloor { journal_revision: 2, control_sequence: 0, authority_epoch: 0 } };
        let recovered = FileOversight::open_evaluated_guarded(root.store(), profile(), &expected, &protocol());
        if stage == JournalIo::DirectorySync {
            let (host, _) = recovered.unwrap(); assert_eq!(host.credibility_report().unwrap().retained_cases, 0); assert!(!host.clock_ready());
        } else { assert!(recovered.is_err()); assert!(!root.store().join(storage::CANONICAL).exists()); }
    }
}

#[test]
fn evaluation_recovery_fence_failure_never_returns_a_role_or_drops_pending_labels() {
    for stage in BARRIERS {
        let root = Directory::new(); let (host, _) = owner(&root);
        let expected = requirements(&host); let report = host.credibility_report().unwrap(); drop(host);
        let store = storage::Store::open(&root.store()).unwrap(); store.fail_once(stage);
        fault(FileOversight::open_evaluated_guarded_store(store, profile(), &expected, &protocol()).unwrap_err(), stage);
        assert_eq!(FileOversight::read_credibility(root.store(), &profile(), &expected, &protocol()).unwrap().report, report);
        let (mut host, roles) = FileOversight::open_evaluated_guarded(root.store(), profile(), &expected, &protocol()).unwrap();
        assert!(assess(&mut host, &roles.evaluator, 2, GroundTruth::Benign).unwrap());
        assert!(!host.clock_ready()); assert!(host.credibility_report().unwrap().qualified());
    }
}

#[test]
fn label_and_promotion_faults_follow_canonical_outcomes_not_lost_acknowledgments() {
    for promoting in [false, true] {
        for stage in BARRIERS {
            let root = Directory::new(); let (mut host, roles) = owner(&root);
            if promoting { assess(&mut host, &roles.evaluator, 2, GroundTruth::Benign).unwrap(); }
            let expected = requirements(&host); let request = update(&host); let before = host.inspect();
            host.store.fail_once(stage);
            let error = if promoting { host.promote_credibility(host.revision(), &request).unwrap_err() }
                else { assess(&mut host, &roles.evaluator, 2, GroundTruth::Benign).unwrap_err() };
            fault(error, stage); assert_eq!(host.inspect(), before);
            assert_eq!(host.credibility_report(), Err(JournalError::Unavailable));
            let canonical = FileOversight::read_credibility(root.store(), &profile(), &expected, &protocol()).unwrap();
            let visible = stage == JournalIo::DirectorySync;
            assert_eq!(canonical.promotions.len(), usize::from(promoting && visible));
            assert_eq!(canonical.report.pending_cases, usize::from(!promoting && !visible));
            drop(host);
            let (mut host, fresh) = FileOversight::open_evaluated_guarded(root.store(), profile(), &expected, &protocol()).unwrap();
            assert_eq!(assess(&mut host, &roles.evaluator, 2, GroundTruth::Benign), Err(JournalError::Contract(Error::Binding)));
            let before_retry = host.revision();
            if promoting && visible {
                let report = host.credibility_report().unwrap();
                assert_eq!(host.promote_credibility(0, &request).unwrap(), canonical.promotions[0]);
                assert_eq!(host.credibility_report().unwrap(), report); assert_eq!(host.revision(), before_retry);
            } else if !promoting {
                assert_eq!(assess(&mut host, &fresh.evaluator, 2, GroundTruth::Benign).unwrap(), !visible);
                assert_eq!(host.revision(), before_retry + u64::from(!visible));
            }
            assert!(!host.clock_ready());
        }
    }
}

#[test]
fn assessments_preserve_interruption_and_corrupt_or_duplicate_histories_refuse() {
    let root = Directory::new(); let (mut host, roles) = owner(&root);
    host.source_interrupted = true; // The actual process latch set before a reader is entered.
    assert!(assess(&mut host, &roles.evaluator, 2, GroundTruth::Benign).unwrap());
    assert!(host.source_interrupted);
    let request = update(&host); assert_eq!(host.promote_credibility(host.revision(), &request), Err(JournalError::Contract(Error::Incomplete)));
    let expected = requirements(&host);
    let mut duplicate = host.events.clone(); duplicate.push(Event::Credibility(CredibilityEvent::Enable(protocol())));
    assert_eq!(check_protocol(&duplicate, &protocol()), Err(Error::Binding));
    let mut invalid = host.events.clone();
    invalid.push(Event::Credibility(CredibilityEvent::Assess(999, Assessment { origin: 8, evidence_id: [17; 32], truth: GroundTruth::Benign })));
    let bytes = journal::encode(&profile(), host.store.identity(), &invalid).unwrap();
    let path = root.store().join(storage::CANONICAL); let original = std::fs::read(&path).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    assert!(FileOversight::read_credibility(root.store(), &profile(), &expected, &protocol()).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    std::fs::write(&path, original).unwrap();
    assert!(FileOversight::read_credibility(root.store(), &profile(), &expected, &protocol()).is_ok());
}
