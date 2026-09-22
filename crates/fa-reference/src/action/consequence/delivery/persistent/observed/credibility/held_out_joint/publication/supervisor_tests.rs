//! Real canonical files and original two-key publication, not detector evidence.
use super::*;
use crate::action::{ActionSpec, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::congress::{CongressPolicy, CredibilityBinding, CredibilityRequirements, MemberPolicy};
use crate::action::consequence::congress::credibility::{Campaign, CaseSpec, CredibilityLedger,
    EvaluationLabel, EvaluationScope, HelperGeneration, LabelSource, LabelVerdict, Observation};
use crate::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, FilePermit, JournalLimits, Reconciliation, RecoveryReserve};
use crate::action::consequence::delivery::persistent::credibility::CredibilityActivation;
use crate::action::consequence::delivery::persistent::observed::FileHumanPermit;
use crate::action::consequence::delivery::persistent::observed::publication::witnesses::{FilePublicationEvidence, FilePublicationInputs};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, ReviewWindow};
use crate::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use crate::reducer::Caps;
use crate::round::{Verdict, commitment};
use crate::witness::refinement::RefinementBudget;
use crate::witness::refinement::index::routing::RoutingBudget;
use crate::Snapshot;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        Self(std::env::temp_dir().join(format!("fa-joint-publication-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))))
    }
    fn bytes(&self) -> Vec<u8> { fs::read(self.0.join(storage::CANONICAL)).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if self.0.exists() && let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("joint publication cleanup: {error}");
        }
    }
}
fn input_profile(epoch: u64, model: u64) -> InputProfileBinding {
    InputProfileBinding { profile_id: 1, profile_bytes: vec![], tokenizer_epoch: 1, policy_epoch: epoch, model_epoch: model }
}
fn profile(hold: u64) -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 32,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
                grade: RestartGrade::ExactRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
            suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::Absent { key: 7 }]).unwrap(),
            congress: CongressPolicy { generation: 1, members: ["a", "b"].into_iter().map(|name|
                (name.into(), MemberPolicy { cohort: name.into(), weight: 10 })).collect(),
                caps: Caps { per_member: 10, per_cohort: 10 }, continue_minimum: 5,
                continue_hold_maximum: hold, narrow_at: 20, suspend_at: 30,
                minimum_members: 2, minimum_cohorts: 2 },
            narrowed_targets: vec![target], target, initial_payload: vec![], retention_ticks: 200,
            max_deliveries: 32, clock_domain: 1, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(["a", "b"].into_iter().map(|name| (name.into(),
            HelperContract::new(input_profile(0, 1), 1, b"approve?".to_vec()).unwrap())).collect()).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 100, max_requests: 32 },
    }
}
fn policy(feed: bool) -> JointPublicationProfile {
    JointPublicationProfile {
        joint: super::super::HeldOutJointPolicy::new(71, 1, 1, 2, 0, 0,
            super::super::HeldOutJointBudget { cases: 3, member_outcomes: 12 }).unwrap(),
        validation: Some(PublicationLimits { bindings: 16,
            validation: RefinementBudget { steps: 100_000, value_bytes: 1_048_576 } }),
        feed: feed.then_some(JointPublicationFeed {
            changes: PublicationChangePolicy { source: 41, after: 0,
                lookup: RoutingBudget { steps: 10_000, bytes: 1_048_576 } },
            freshness: PublicationFreshnessPolicy { clock_domain: 1, max_age_ticks: 100 },
            snapshot_fallback: true,
        }),
    }
}

fn state() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn publication_input(epoch: u64, model: u64) -> FilePublicationInputs {
    let bytes = b"original publication view".to_vec(); let end = bytes.len();
    FilePublicationInputs::new(None, Some(ActualHelperInput::new(bytes, input_profile(epoch, model),
        vec![SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: 0, end } }], vec![]).unwrap()))
}
fn reviewed(host: &mut FileOversight, p: &FileOversightProfile, id: u64) -> (FrozenAction, CommitteeInput) {
    let epoch = host.inspect().control.ledger.epoch;
    let action = host.propose(host.revision(), id, ActionSpec { version: VERSION, scope: p.delivery.scope,
        target: Some(host.inspect().target), payload: b"publish".to_vec(), required_witnesses: vec![],
        policy_epoch: epoch, deadline: ElapsedTick(100), units: 7 }, state()).unwrap();
    let publication = publication_input(epoch, 1);
    host.bind_publication_evidence(host.revision(), id, FilePublicationEvidence::new(publication.clone(), vec![]).unwrap()).unwrap();
    let revision = host.publication_input_revision(id).unwrap();
    host.record_publication_inputs(host.revision(), id, revision, Some(publication)).unwrap();
    let source = EvidenceSnapshot::new(EvidenceIdentity { source: 1, generation: id, scope: p.delivery.scope },
        state(), ["a", "b"].into_iter().map(|name| (name.into(), b"context".to_vec())).collect()).unwrap();
    let inputs = source.inputs_for(&action, &p.committee).unwrap();
    host.record_inputs(host.revision(), id, 0, inputs.clone()).unwrap();
    let round = 100 + id;
    host.begin_review(host.revision(), id, round, [9; 32], ReviewWindow { commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) }, state()).unwrap();
    for member in ["a", "b"] {
        host.commit_review(host.revision(), round, member, commitment(round, member, &[9; 32], Verdict::Allow, b"salt").unwrap()).unwrap();
    }
    host.open_reveals(host.revision(), round).unwrap();
    for member in ["a", "b"] { host.reveal_review(host.revision(), round, member, Verdict::Allow, b"salt".to_vec()).unwrap(); }
    host.finish_review(host.revision(), round, Some(&inputs), state()).unwrap().unwrap();
    (action, inputs)
}
fn prepared(host: &mut FileOversight, human: &FileHumanReviewer, p: &FileOversightProfile, id: u64)
    -> (FrozenAction, CommitteeInput, FilePermit, FileHumanPermit)
{
    let (action, inputs) = reviewed(host, p, id);
    let automatic = host.authorize(host.revision(), id, &inputs, state()).unwrap();
    let request = host.request_human_approval(host.revision(), 1000 + id, id, &inputs, ElapsedTick(80)).unwrap();
    let revision = host.revision(); let key = human.approve(host, revision, &request).unwrap();
    (action, inputs, automatic, key)
}
fn activation(host: &FileOversight, p: &FileOversightProfile, operation: u64, generation: u64) -> CredibilityActivation {
    let mut ledger = CredibilityLedger::new(Campaign {
        scope: EvaluationScope { campaign: operation, model_generation: 1, evaluator_generation: 1, held_out_manifest: [8; 32] },
        label_owner: "evaluator".into(), helpers: ["a", "b"].into_iter().map(|name| (name.into(), HelperGeneration { generation: 1, cohort: name.into() })).collect(),
        strata: BTreeSet::from(["effect".into()]), cases: (1..=3).map(|id| CaseSpec {
            id, stratum: "effect".into(), evidence_root: [id as u8; 32], dispatch_sequence: 2 }).collect(),
    }).unwrap();
    for id in 1..=3 {
        let observations = ["a", "b"].into_iter().map(|name| (name.into(),
            if id == 1 && name == "a" || id == 2 && name == "b" { Observation::Hold { first_sequence: 1 } }
            else { Observation::Clear })).collect();
        ledger.record_observations(id, observations).unwrap();
        ledger.record_label(id, EvaluationLabel { owner: "evaluator".into(), evaluator_generation: 1,
            source: LabelSource::IndependentEvaluation, evidence_root: [id as u8; 32], recorded_sequence: 2,
            verdict: if id == 3 { LabelVerdict::Safe } else { LabelVerdict::Violation } }).unwrap();
    }
    let snapshot = ledger.seal(2).unwrap();
    CredibilityActivation { operation, expected_control_sequence: host.inspect().control.sequence,
        expected_epoch: host.inspect().control.ledger.epoch, scope: p.delivery.scope,
        policy_generation: p.delivery.policy.generation(), actor_profile: p.delivery.actor.profile(),
        binding: CredibilityBinding { scope: snapshot.scope().clone(), label_owner: snapshot.label_owner().into(),
            helpers: snapshot.helpers().clone(), strata: snapshot.strata().clone(), reducer_generation: generation },
        stratum: "effect".into(), requirements: CredibilityRequirements {
            minimum_safe_cases: 1, minimum_violation_cases: 2, minimum_precision_ppm: 1_000_000,
            minimum_timely_recall_ppm: 500_000, maximum_false_positive_ppm: 0, base_weight: 10,
            lead_bonus_weight: 0, lead_saturation_sequences: 0, maximum_evidence_age: 100,
            maximum_member_share_ppm: 500_000, maximum_cohort_share_ppm: 500_000 }, snapshot }
}
fn ready(hold: u64) -> (Directory, FileOversight, FileHumanReviewer, FileOversightProfile) {
    let root = Directory::new(); let p = profile(hold);
    let (mut host, human) = FileOversight::create_with_joint_publication(&root.0, p.clone(), policy(false)).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    for id in 1..=2 { reviewed(&mut host, &p, id); host.cancel(host.revision(), id).unwrap(); }
    (root, host, human, p)
}

#[test]
fn joint_and_final_view_guards_both_control_actual_publication() {
    for (hold, changed) in [(5, false), (0, false), (0, true)] {
        let (root, mut host, human, p) = ready(hold);
        let request = activation(&host, &p, 10, 2);
        let before = host.inspect(); let bytes = root.bytes();
        let result = host.activate_credibility(host.revision(), request, &p.committee);
        if hold == 5 {
            assert_eq!(result, Err(Error::Incomplete.into()));
            assert_eq!(host.inspect(), before); assert_eq!(root.bytes(), bytes);
            continue;
        }
        result.unwrap();
        assert!(host.held_out_joint_report(10).unwrap().unwrap().qualified());
        let (action, inputs, automatic, key) = prepared(&mut host, &human, &p, 3);
        host.dispatch(host.revision(), &automatic, &key, &action, &inputs, state()).unwrap();
        if changed {
            let revision = host.publication_input_revision(3).unwrap();
            host.record_publication_inputs(host.revision(), 3, revision,
                Some(publication_input(action.spec().policy_epoch, 2))).unwrap();
        }
        let publication = host.publish_checked(host.revision(), 3, Some(&inputs), state(), ElapsedTick(2)).unwrap();
        assert_eq!(matches!(publication.outcome, EndpointOutcome::Executed { .. }), !changed);
        host.reconcile(host.revision(), 3).unwrap();
        assert_eq!(host.inspect().executions, u64::from(!changed));
        assert_eq!(host.inspect().control.ledger.charged, if changed { 0 } else { 7 });
        let bytes = root.bytes();
        let history = FileOversight::read_joint_publication(&root.0, &p, policy(false)).unwrap();
        assert_eq!(history.journal, host.inspect()); assert_eq!(history.promotions.len(), 1);
        assert_eq!(root.bytes(), bytes);
    }
}

#[test]
fn recovery_keeps_unknown_charges_and_requires_new_qualification_and_both_keys() {
    let (root, mut host, human, p) = ready(0);
    let first = activation(&host, &p, 10, 2);
    let receipt = host.activate_credibility(host.revision(), first.clone(), &p.committee).unwrap();
    let (action, inputs, automatic, key) = prepared(&mut host, &human, &p, 3);
    host.dispatch(host.revision(), &automatic, &key, &action, &inputs, state()).unwrap();
    drop(host);
    let (mut host, human) = FileOversight::open_with_joint_publication(&root.0, p.clone(), policy(false)).unwrap();
    assert!(!host.clock_ready()); assert_eq!(host.check_credibility(), Err(Error::Stale.into()));
    assert_eq!(host.inspect().control.ledger.charged, 7);
    assert_eq!(host.activate_credibility(0, first, &p.committee).unwrap(), receipt);
    assert_eq!(host.check_credibility(), Err(Error::Stale.into()));
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert!(host.dispatch(host.revision(), &automatic, &key, &action, &inputs, state()).is_err());
    assert_eq!(host.reconcile(host.revision(), 3).unwrap(), Reconciliation::AwaitingResolution);
    assert_eq!(host.inspect().control.ledger.charged, 7);
    assert_eq!(host.seal_unexecuted(host.revision(), 3).unwrap(), Reconciliation::Resolved(
        EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed }));
    let next = activation(&host, &p, 11, 3);
    host.activate_credibility(host.revision(), next, &p.committee).unwrap();
    let (action, inputs, automatic, key) = prepared(&mut host, &human, &p, 4);
    host.dispatch(host.revision(), &automatic, &key, &action, &inputs, state()).unwrap();
    assert!(matches!(host.publish_checked(host.revision(), 4, Some(&inputs), state(), ElapsedTick(3)).unwrap().outcome,
        EndpointOutcome::Executed { .. }));
    host.reconcile(host.revision(), 4).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 7);
}

#[test]
fn live_selection_checks_precede_source_work_without_changing_authority() {
    let root = Directory::new(); let p = profile(0); let expected = policy(true);
    let (host, _) = FileOversight::create_with_joint_publication(&root.0, p, expected).unwrap();
    let before = host.inspect(); let bytes = root.bytes();
    host.check_joint_publication_profile(expected).unwrap();
    for field in 0..6 {
        let mut wrong = expected;
        match field {
            0 => wrong.joint = super::super::HeldOutJointPolicy::new(72, 1, 1, 2, 0, 0,
                expected.joint.budget()).unwrap(),
            1 => wrong.validation.as_mut().unwrap().validation.steps -= 1,
            2 => wrong.feed.as_mut().unwrap().changes.source += 1,
            3 => wrong.feed.as_mut().unwrap().freshness.max_age_ticks -= 1,
            4 => wrong.feed.as_mut().unwrap().snapshot_fallback = false,
            _ => wrong.feed = None,
        }
        assert_eq!(host.check_joint_publication_profile(wrong), Err(Error::Binding.into()));
        assert_eq!(host.inspect(), before); assert_eq!(root.bytes(), bytes);
        assert!(host.storage_failure().is_none());
    }
    host.check_joint_publication_profile(expected).unwrap();
}
