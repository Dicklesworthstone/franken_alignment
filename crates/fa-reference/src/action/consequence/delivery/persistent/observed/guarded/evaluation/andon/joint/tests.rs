//! Original native reviews, independent labels, joint replay and real Store writes.
use super::*;
use crate::action::{ActionSpec, ElapsedTick, VERSION};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{JournalIo, MAX_JOURNAL_EVENTS};
use crate::action::consequence::delivery::persistent::observed::{
    credibility::{FileCredibilityUpdate, FileIndependentEvaluator, andon::tests as original},
    guarded::{FileRecoveryFloor, FileRecoveryRequirements, recovery::FileGuardedRecoveryStatus},
};
use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput, ReviewWindow, action_frame};
use crate::action::consequence::oversight::credibility::{Assessment, Fraction, GroundTruth};
use crate::action::consequence::oversight::joint_credibility::{JointFailure, JointReplayBudget};
use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use crate::full_input::{ActualHelperInput, ByteSpan, PartKind, SubmittedPart};
use crate::reducer::Caps;
use crate::round::{Verdict, commitment};
use crate::Snapshot;
use std::{collections::BTreeMap, fs};
use original::{Directory, BARRIERS};

fn policy() -> JointPromotionPolicy {
    JointPromotionPolicy::new(71, 1, Fraction { numerator: 0, denominator: 1 },
        Fraction { numerator: 1, denominator: 2 }, JointReplayBudget::default()).unwrap()
}
fn protocol() -> EvaluationProtocol {
    let mut p = original::protocol(); p.minimum_violation_origins = 2;
    p.false_stop_budget = 100; p
}
fn guards() -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
        identity: None, campaigns: None, credential: None }
}
fn profile(hold: u64) -> FileOversightProfile {
    let mut p = original::profile();
    p.delivery.total = 1000; p.delivery.max_attempts = 16;
    p.delivery.congress = CongressPolicy {
        generation: 1, members: ["a", "b"].into_iter().map(|name| (name.to_owned(),
            MemberPolicy { cohort: name.into(), weight: 10 })).collect(),
        caps: Caps { per_member: 10, per_cohort: 10 }, continue_minimum: 5,
        continue_hold_maximum: hold, narrow_at: 20, suspend_at: 30,
        minimum_members: 2, minimum_cohorts: 2,
    };
    let helper = p.committee.members()["reviewer"].clone();
    p.committee = CommitteeContract::new(["a", "b"].into_iter()
        .map(|name| (name.into(), helper.clone())).collect()).unwrap();
    p
}
fn config() -> FileAndonConfig {
    let mut budget = original::budget(); budget.false_stops_allowed = 1;
    FileAndonConfig::new(original::config().identity(), vec![budget]).unwrap()
}
fn requirements(h: &FileOversight) -> FileAndonRecoveryRequirements {
    let c = h.inspect().control;
    FileAndonRecoveryRequirements::new(FileRecoveryRequirements {
        guards: guards(), effective_policy: h.profile.delivery.policy.clone(), credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: h.revision(), control_sequence: c.sequence,
            authority_epoch: c.ledger.epoch },
    }, config(), config().windows().to_vec(), h.credibility_report().unwrap().revision).unwrap()
}
fn case(h: &mut FileOversight, id: u64, a: Verdict, b: Verdict) {
    let p = h.profile.clone(); let round = 100 + id;
    let snapshot = Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() };
    let action = h.propose(h.revision(), id, ActionSpec { version: VERSION, scope: p.delivery.scope,
        target: Some(h.inspect().target), payload: b"native".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: h.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 }, snapshot.clone()).unwrap();
    let views = p.committee.members().iter().map(|(name, helper)| {
        let mut bytes = action_frame(&action); let start = bytes.len();
        bytes.extend_from_slice(helper.question()); let end = bytes.len();
        let input = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
            SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: start } },
            SubmittedPart { kind: PartKind::Question, span: ByteSpan { start, end } },
        ], Vec::new()).unwrap();
        (name.clone(), EvidenceViewManifest::new(input, AuthorizationProjection {
            projection_id: helper.projection_id(), policy_epoch: action.spec().policy_epoch,
            projected_originals: Vec::new(),
        }, Vec::new()).unwrap())
    }).collect();
    let inputs = CommitteeInput::capture(&action, &p.committee, views).unwrap();
    h.record_inputs(h.revision(), id, 0, inputs.clone()).unwrap();
    h.begin_review(h.revision(), id, round, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) }, snapshot.clone()).unwrap();
    for (name, verdict) in [("a", a), ("b", b)] {
        h.commit_review(h.revision(), round, name, commitment(round, name, &[9; 32], verdict, b"s").unwrap()).unwrap();
    }
    h.open_reveals(h.revision(), round).unwrap();
    for (name, verdict) in [("a", a), ("b", b)] {
        h.reveal_review(h.revision(), round, name, verdict, b"s".to_vec()).unwrap();
    }
    h.finish_review(h.revision(), round, Some(&inputs), snapshot).unwrap().unwrap();
}
fn label(h: &mut FileOversight, e: &FileIndependentEvaluator, id: u64, truth: GroundTruth) -> Result<bool, JournalError> {
    let ticket = h.evaluation_ticket(100 + id)?; let revision = h.revision();
    e.assess(h, revision, &ticket, Assessment { origin: id, evidence_id: [17; 32], truth })
}
fn calibrated(root: &Directory, hold: u64, reserved: bool) -> (FileOversight, FileEvaluatedOversightRoles) {
    let (mut h, roles) = if reserved {
        FileOversight::create_joint_andon_guarded_reserved(root.store(), profile(hold), &guards(), None,
            protocol(), policy(), &config(), RecoveryReserve::terminal()).unwrap()
    } else {
        FileOversight::create_joint_andon_guarded(root.store(), profile(hold), &guards(), None,
            protocol(), policy(), &config()).unwrap()
    };
    h.observe_time(h.revision(), ElapsedTick(1)).unwrap();
    for (id, a, b, truth) in [
        (1, Verdict::Hold, Verdict::Allow, GroundTruth::Violation),
        (2, Verdict::Allow, Verdict::Hold, GroundTruth::Violation),
        (3, Verdict::Allow, Verdict::Allow, GroundTruth::Benign),
        (4, Verdict::Hold, Verdict::Allow, GroundTruth::Benign),
    ] {
        case(&mut h, id, a, b); label(&mut h, &roles.evaluator, id, truth).unwrap();
    }
    assert!(h.credibility_report().unwrap().qualified());
    assert_eq!(h.andon_ledger().unwrap().windows()[&1].false_stops, 1);
    assert_eq!(h.andon_ledger().unwrap().windows()[&1].incident.as_ref().unwrap().opened_by_round, 104);
    (h, roles)
}
fn update(h: &FileOversight) -> FileCredibilityUpdate {
    let c = h.inspect().control;
    FileCredibilityUpdate { operation: 7, expected_control_sequence: c.sequence,
        expected_authority_epoch: c.ledger.epoch, expected_evaluation_revision: h.credibility_report().unwrap().revision }
}

#[test]
fn qualified_joint_promotion_and_andon_incident_survive_anchored_restart() {
    for reserved in [false, true] {
        let root = Directory::new(); let (mut h, roles) = calibrated(&root, 0, reserved);
        assert_eq!(h.journal_capacity().unwrap().reserve(), reserved.then_some(RecoveryReserve::terminal()));
        let accounting = h.andon_ledger().unwrap().clone(); let receipt = h.andon_receipt(104).unwrap().clone();
        let request = update(&h); let preview = h.joint_credibility_report(request.expected_evaluation_revision).unwrap();
        assert!(preview.qualified());
        let promotion = h.promote_credibility(h.revision(), &request).unwrap();
        assert_eq!(promotion.change.current.members["a"].weight, 3);
        assert_eq!(promotion.change.current.members["b"].weight, 5);
        assert_eq!(h.joint_credibility_promotion(7).unwrap(), &preview);
        assert_eq!(h.andon_ledger().unwrap(), &accounting);
        case(&mut h, 5, Verdict::Allow, Verdict::Allow);
        label(&mut h, &roles.evaluator, 5, GroundTruth::Censored).unwrap();
        let expected = requirements(&h).require_joint(policy()).unwrap();
        let anchor = h.history_anchor().unwrap(); let report = h.credibility_report().unwrap(); drop(h);
        let (mut h, fresh) = FileOversight::open_andon_guarded(root.store(), profile(0), &expected, &protocol(), &anchor).unwrap();
        assert_eq!(h.joint_credibility_policy().unwrap(), Some(policy()));
        assert_eq!(h.joint_credibility_promotion(7).unwrap(), &preview);
        assert_eq!(h.credibility_report().unwrap(), report); assert_eq!(h.andon_receipt(104).unwrap(), &receipt);
        assert_eq!(h.andon_ledger().unwrap(), &accounting); assert!(!h.clock_ready());
        assert_eq!(label(&mut h, &roles.evaluator, 5, GroundTruth::Benign), Err(Error::Binding.into()));
        assert_eq!(label(&mut h, &fresh.evaluator, 5, GroundTruth::Benign), Err(Error::Incomplete.into()));
        h.observe_time(h.revision(), ElapsedTick(2)).unwrap();
        assert!(label(&mut h, &fresh.evaluator, 5, GroundTruth::Benign).unwrap());
        assert_eq!(h.andon_ledger().unwrap(), &accounting); assert_eq!(h.inspect().executions, 0);
        let before = root.bytes();
        assert_eq!(h.promote_credibility(0, &request).unwrap(), promotion); assert_eq!(root.bytes(), before);
    }
}

#[test]
fn common_mode_regression_is_refused_before_and_after_recovery_without_accounting_drift() {
    let root = Directory::new(); let (mut h, _) = calibrated(&root, 5, true);
    for recovered in [false, true] {
        if recovered {
            let expected = requirements(&h).require_joint(policy()).unwrap(); let anchor = h.history_anchor().unwrap(); drop(h);
            (h, _) = FileOversight::open_andon_guarded(root.store(), profile(5), &expected, &protocol(), &anchor).unwrap();
            h.observe_time(h.revision(), ElapsedTick(2)).unwrap();
        }
        let request = update(&h); let preview = h.joint_credibility_report(request.expected_evaluation_revision).unwrap();
        assert!(preview.failures.contains(&JointFailure::RegressedOrigin(1)));
        let bytes = root.bytes(); let control = h.inspect(); let accounting = h.andon_ledger().unwrap().clone();
        assert_eq!(h.promote_credibility(h.revision(), &request), Err(Error::Incomplete.into()));
        assert_eq!(root.bytes(), bytes); assert_eq!(h.inspect(), control); assert_eq!(h.andon_ledger().unwrap(), &accounting);
        assert_eq!(h.joint_credibility_promotion(7).err(), Some(Error::Missing.into()));
    }
}

#[test]
fn absent_or_changed_joint_requirements_refuse_before_replay_or_staging_cleanup() {
    let root = Directory::new(); let (h, _) = calibrated(&root, 0, true);
    let plain = requirements(&h); let exact = plain.clone().require_joint(policy()).unwrap();
    assert_eq!(plain.joint_policy(), None); assert_eq!(exact.joint_policy(), Some(policy()));
    assert_eq!(exact.clone().require_joint(policy()), Err(Error::Duplicate));
    let anchor = h.history_anchor().unwrap(); let bytes = root.bytes(); drop(h);
    let pending = root.store().join("delivery.pending"); fs::write(&pending, b"retain staging").unwrap();
    let mut variants = vec![plain.clone()];
    for field in 0..6 {
        let p = JointPromotionPolicy::new(if field == 0 { 72 } else { 71 }, if field == 1 { 2 } else { 1 },
            Fraction { numerator: u64::from(field == 2), denominator: 1 },
            Fraction { numerator: 1, denominator: if field == 3 { 1 } else { 2 } }, JointReplayBudget {
                cases: if field == 4 { 4 } else { JointReplayBudget::default().cases },
                member_outcomes: if field == 5 { 16 } else { JointReplayBudget::default().member_outcomes },
            }).unwrap();
        variants.push(plain.clone().require_joint(p).unwrap());
    }
    for expected in variants {
        assert_eq!(FileOversight::begin_open_andon_guarded(root.store(), profile(0), &expected, &protocol(), &anchor).err(), Some(Error::Binding.into()));
        assert!(FileOversight::read_andon_guarded(root.store(), &profile(0), &expected, &protocol(), &anchor).is_err());
        assert_eq!(root.bytes(), bytes); assert_eq!(fs::read(&pending).unwrap(), b"retain staging");
    }
    let mut run = FileOversight::begin_open_andon_guarded(root.store(), profile(0), &exact, &protocol(), &anchor).unwrap();
    assert_eq!(run.progress().attempted_events, 0);
    run.advance(0, MAX_JOURNAL_EVENTS).unwrap(); assert_eq!(run.progress().status, FileGuardedRecoveryStatus::Ready);
    let (h, _, _) = run.finish_reconciled(|| ElapsedTick(2)).unwrap();
    assert_eq!(h.joint_credibility_policy().unwrap(), Some(policy())); assert!(!pending.exists());
}

#[test]
fn marginal_only_and_mixed_activation_histories_cannot_be_reinterpreted_as_joint() {
    let root = Directory::new();
    let (h, _) = FileOversight::create_andon_guarded(root.store(), profile(0), &guards(), None, protocol(), &config()).unwrap();
    let plain = requirements(&h); let joint = plain.clone().require_joint(policy()).unwrap();
    let anchor = h.history_anchor().unwrap(); let mut events = h.events.clone(); let bytes = root.bytes(); drop(h);
    assert!(FileOversight::begin_open_andon_guarded(root.store(), profile(0), &joint, &protocol(), &anchor).is_err());
    assert!(FileOversight::read_andon_guarded(root.store(), &profile(0), &plain, &protocol(), &anchor).is_ok());
    events.push(Event::Credibility(CredibilityEvent::EnableJoint(protocol(), policy())));
    for required in [None, Some(policy())] { assert_eq!(check_protocol(&events, &protocol(), required), Err(Error::Binding)); }
    assert_eq!(root.bytes(), bytes);
}

#[test]
fn joint_promotion_barriers_preserve_accounting_and_joint_history_together() {
    for barrier in BARRIERS {
        let root = Directory::new(); let (mut h, _) = calibrated(&root, 0, true);
        let expected = requirements(&h).require_joint(policy()).unwrap(); let anchor = h.history_anchor().unwrap();
        let accounting = h.andon_ledger().unwrap().clone(); let receipt = h.andon_receipt(104).unwrap().clone();
        let request = update(&h); h.store.fail_once(barrier);
        assert!(h.promote_credibility(h.revision(), &request).is_err());
        assert!(h.storage_failure().is_some()); assert!(h.andon_source().is_err());
        let saved = FileOversight::read_andon_guarded(root.store(), &profile(0), &expected, &protocol(), &anchor).unwrap();
        assert_eq!(saved.accounting, accounting); assert_eq!(saved.receipts, vec![receipt]);
        let visible = usize::from(barrier == JournalIo::DirectorySync);
        assert_eq!(saved.credibility.promotions.len(), visible); drop(h);
        let (h, _) = FileOversight::open_andon_guarded(root.store(), profile(0), &expected, &protocol(), &anchor).unwrap();
        assert_eq!(usize::from(h.joint_credibility_promotion(7).is_ok()), visible);
        assert_eq!(h.andon_ledger().unwrap(), &accounting);
    }
}

#[test]
fn reserved_joint_first_image_uses_original_storage_barriers_and_no_partial_roles() {
    for barrier in BARRIERS {
        let root = Directory::new();
        let prepared = PreparedGuardedBootstrap::prepare_reserved(profile(0), &guards(), None, RecoveryReserve::terminal()).unwrap()
            .jointly_evaluated(protocol(), policy()).unwrap().with_andon(&config()).unwrap();
        let store = storage::Store::create(&root.store()).unwrap(); store.fail_once(barrier);
        assert!(prepared.publish(store).is_err());
        if barrier == JournalIo::DirectorySync {
            let (h, _) = FileOversight::open(root.store(), profile(0)).unwrap();
            assert_eq!(h.joint_credibility_policy().unwrap(), Some(policy()));
            assert_eq!(h.andon_config().unwrap(), Some(&config()));
            assert_eq!(h.journal_capacity().unwrap().reserve(), Some(RecoveryReserve::terminal()));
        } else { assert!(!root.store().join("delivery.bin").exists()); }
    }
    let root = Directory::new();
    assert!(FileOversight::create_joint_andon_guarded_reserved(root.store(), profile(0), &guards(), None,
        protocol(), policy(), &config(), RecoveryReserve { events: 0, bytes: 0 }).is_err());
    assert!(!root.store().exists());
}

#[test]
fn joint_work_exhaustion_remains_a_refusal_not_a_marginal_only_promotion() {
    let root = Directory::new();
    let limited = JointPromotionPolicy::new(71, 1, Fraction { numerator: 0, denominator: 1 },
        Fraction { numerator: 0, denominator: 1 }, JointReplayBudget { cases: 1, member_outcomes: 4 }).unwrap();
    let (mut h, roles) = FileOversight::create_joint_andon_guarded(root.store(), profile(0), &guards(), None,
        protocol(), limited, &config()).unwrap();
    h.observe_time(h.revision(), ElapsedTick(1)).unwrap();
    for (id, a, b, truth) in [(1, Verdict::Hold, Verdict::Allow, GroundTruth::Violation),
        (2, Verdict::Allow, Verdict::Hold, GroundTruth::Violation),
        (3, Verdict::Allow, Verdict::Allow, GroundTruth::Benign)] {
        case(&mut h, id, a, b); label(&mut h, &roles.evaluator, id, truth).unwrap();
    }
    assert!(h.credibility_report().unwrap().qualified()); let request = update(&h); let bytes = root.bytes();
    assert_eq!(h.joint_credibility_report(request.expected_evaluation_revision), Err(Error::Limit.into()));
    assert_eq!(h.promote_credibility(h.revision(), &request), Err(Error::Limit.into())); assert_eq!(root.bytes(), bytes);
}
