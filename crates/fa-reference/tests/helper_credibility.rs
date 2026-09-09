//! Public integration: actual recorded reference votes -> independent labels ->
//! bounded weights -> fenced controller -> fresh review and endpoint publication.
//! No real inference, evaluator authentication or production durability is claimed.

use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::PublicationEndpoint;
use fa_reference::action::consequence::gate::{ReviewBinding, TargetCeiling};
use fa_reference::action::consequence::gate::containment::{ActorState, ResetRequest, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, OversightBroker, ReviewWindow, action_frame};
use fa_reference::action::consequence::oversight::credibility::{Assessment, EvaluationProtocol, Fraction, GroundTruth, IndependentEvaluator, QualificationFailure};
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::{Error, Snapshot};
use std::collections::BTreeMap;

fn protocol() -> EvaluationProtocol {
    EvaluationProtocol { domain: 1, stratum: 1, period: 1, minimum_violation_origins: 2, minimum_benign_origins: 2,
        precision_floor: Fraction { numerator: 1, denominator: 2 }, recall_floor: Fraction { numerator: 1, denominator: 2 },
        false_positive_ceiling: Fraction { numerator: 1, denominator: 2 }, false_stop_budget: 10 }
}
fn spec(epoch: u64, version: u64) -> ActionSpec {
    ActionSpec { version: VERSION, scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: version, generation: 1 }),
        payload: b"publish".to_vec(), required_witnesses: Vec::new(), policy_epoch: epoch, deadline: ElapsedTick(100), units: 16 }
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, vec![9])]) } }
fn exact_policy(generation: u64) -> Policy { Policy::new(generation, vec![Predicate::ExactValue { key: 7, value: vec![9] }]).unwrap() }
fn fixture(protocol: EvaluationProtocol) -> (OversightBroker, PublicationEndpoint, IndependentEvaluator, CommitteeContract) {
    let contracts = CommitteeContract::new(["alice", "bob"].into_iter().enumerate().map(|(index, member)| (
        member.to_owned(), HelperContract::new(InputProfileBinding { profile_id: index as u64 + 1,
            profile_bytes: member.as_bytes().to_vec(), policy_epoch: 0, model_epoch: 1, tokenizer_epoch: 1 }, 1, b"Review".to_vec()).unwrap(),
    )).collect()).unwrap();
    let config = ControllerConfig { scope: spec(0, 1).scope, total: 100, max_attempts: 16,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1,
            tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3, policy: exact_policy(1),
        congress: CongressPolicy { generation: 1, members: ["alice", "bob"].into_iter().map(|member| (member.to_owned(), MemberPolicy { cohort: member.to_owned(), weight: 1 })).collect(),
            caps: Caps { per_member: 10, per_cohort: 10 }, continue_minimum: 2, continue_hold_maximum: 0, narrow_at: 21, suspend_at: 22,
            minimum_members: 2, minimum_cohorts: 2 },
        narrowed_targets: TargetCeiling::new(&[spec(0, 1).target.unwrap()]).unwrap() };
    let mut endpoint = PublicationEndpoint::new(spec(0, 1).target.unwrap(), Vec::new(), 200, 16).unwrap();
    let mut broker = OversightBroker::new(config, &mut endpoint, contracts.clone()).unwrap();
    let evaluator = broker.enable_credibility(protocol).unwrap();
    broker.observe_time(ElapsedTick(1)).unwrap(); endpoint.observe_time(ElapsedTick(1)).unwrap();
    let ack = endpoint.install_fence(broker.fence_request()).unwrap(); broker.confirm_fence(ack).unwrap();
    (broker, endpoint, evaluator, contracts)
}
fn prepare(b: &mut OversightBroker, c: &CommitteeContract, id: u64, version: u64) -> (FrozenAction, CommitteeInput) {
    let action = b.propose(id, spec(b.inspect().ledger.epoch, version), &snapshot()).unwrap().action;
    let views = c.members().iter().map(|(name, helper)| {
        let mut bytes = action_frame(&action); let boundary = bytes.len(); bytes.extend_from_slice(helper.question()); let end = bytes.len();
        let input = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
            SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
            SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
        ], Vec::new()).unwrap();
        let manifest = EvidenceViewManifest::new(input, AuthorizationProjection {
            projection_id: 1, policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new(),
        }, Vec::new()).unwrap();
        (name.clone(), manifest)
    }).collect();
    let input = CommitteeInput::capture(&action, c, views).unwrap(); b.record_inputs(id, 0, input.clone()).unwrap(); (action, input)
}
fn review(b: &mut OversightBroker, id: u64, input: &CommitteeInput, verdicts: [Verdict; 2]) {
    let mut s = b.begin_review(id, id + 10, [1; 32], ReviewWindow { commit_by: ElapsedTick(10), reveal_by: ElapsedTick(20) }, &snapshot()).unwrap();
    for (i, member) in ["alice", "bob"].into_iter().enumerate() {
        let value = s.commitment(member, verdicts[i], member.as_bytes()).unwrap(); s.commit(member, value, ElapsedTick(1)).unwrap();
    }
    s.open_reveals(ElapsedTick(1)).unwrap();
    for (i, member) in ["alice", "bob"].into_iter().enumerate() { s.reveal(member, verdicts[i], member.as_bytes(), ElapsedTick(1)).unwrap(); }
    b.apply_review(s.finish(ElapsedTick(1)).unwrap(), Some(input), &snapshot()).unwrap();
}
fn label(b: &mut OversightBroker, e: &IndependentEvaluator, id: u64, origin: u64, truth: GroundTruth) {
    let ticket = b.evaluation_ticket(id + 10).unwrap();
    let sealed = e.assess(&ticket, Assessment { origin, evidence_id: [3; 32], truth }).unwrap();
    b.record_evaluation(sealed).unwrap();
}
fn train(b: &mut OversightBroker, e: &IndependentEvaluator, c: &CommitteeContract) -> Vec<(FrozenAction, CommitteeInput)> {
    let votes = [[Verdict::Hold, Verdict::Hold], [Verdict::Hold, Verdict::Allow], [Verdict::Allow, Verdict::Allow], [Verdict::Allow, Verdict::Allow]];
    let mut proposals = Vec::new();
    for (i, votes) in votes.into_iter().enumerate() {
        let id = i as u64 + 1; let proposal = prepare(b, c, id, 1); review(b, id, &proposal.1, votes);
        label(b, e, id, id, if id <= 2 { GroundTruth::Violation } else { GroundTruth::Benign }); proposals.push(proposal);
    }
    proposals
}
#[test]
fn labelled_dissenter_gains_credit_and_rotation_fences_permits_without_refunding_unknowns() {
    let (mut b, mut endpoint, e, c) = fixture(protocol()); let proposals = train(&mut b, &e, &c);
    let pending = b.authorize(3, Some(&proposals[2].1), &snapshot()).unwrap();
    let issued = b.authorize(4, Some(&proposals[3].1), &snapshot()).unwrap();
    let message = b.dispatch(&issued, &proposals[3].0, Some(&proposals[3].1), &snapshot()).unwrap(); b.acknowledgment_lost(4).unwrap();
    let metrics = b.credibility_report().unwrap(); assert!(metrics.qualified());
    assert_eq!(metrics.members["alice"].true_positives, 2); assert_eq!(metrics.members["bob"].false_negatives, 1);
    let before = b.inspect();
    assert_eq!(b.promote_credibility(before.sequence + 1, before.ledger.epoch, metrics.revision), Err(Error::Stale)); assert_eq!(b.inspect(), before);
    let promotion = b.promote_credibility(before.sequence, before.ledger.epoch, metrics.revision).unwrap();
    assert_eq!(promotion.change.current.members["alice"].weight, 10); assert_eq!(promotion.change.current.members["bob"].weight, 5);
    assert_eq!(promotion.change.current.caps, promotion.change.previous.caps);
    assert_eq!(promotion.change.current.continue_minimum, promotion.change.previous.continue_minimum);
    assert_eq!(promotion.change.current.minimum_members, 2);
    assert_eq!(promotion.change.cancelled, vec![1, 2, 3]); assert_eq!(promotion.change.refunded_units, 16);
    assert_eq!(b.inspect().ledger.available, 84); assert_eq!(b.inspect().ledger.charged, 16); assert_eq!(b.inspect().ledger.stages[&4], ActionState::Unknown);
    assert!(b.dispatch(&pending, &proposals[2].0, Some(&proposals[2].1), &snapshot()).is_err());
    b.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap(); assert_eq!(b.inspect().ledger.stages[&4], ActionState::Confirmed);
    let fresh = prepare(&mut b, &c, 5, 2); assert_eq!(fresh.0.spec().policy_epoch, 1); review(&mut b, 5, &fresh.1, [Verdict::Allow; 2]);
    let permit = b.authorize(5, Some(&fresh.1), &snapshot()).unwrap();
    let message = b.dispatch(&permit, &fresh.0, Some(&fresh.1), &snapshot()).unwrap(); b.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 2); assert_eq!(b.credibility_promotions().unwrap(), &[promotion]);
}
#[test]
fn pending_labels_and_discarded_rounds_cannot_be_cherry_picked_away() {
    let (mut b, _, e, c) = fixture(protocol()); train(&mut b, &e, &c);
    let (_, input) = prepare(&mut b, &c, 5, 1);
    let discarded = b.begin_review(5, 99, [1; 32], ReviewWindow { commit_by: ElapsedTick(10), reveal_by: ElapsedTick(20) }, &snapshot()).unwrap(); drop(discarded);
    let report = b.credibility_report().unwrap(); assert_eq!(report.pending_cases, 1); assert_eq!(report.scoped_cases, 5);
    let before = b.inspect(); assert_eq!(b.promote_credibility(before.sequence, before.ledger.epoch, report.revision), Err(Error::Incomplete));
    assert_eq!(b.inspect(), before); assert!(b.authorize(5, Some(&input), &snapshot()).is_err()); b.cancel(5).unwrap();
}
#[test]
fn censoring_is_retained_and_only_independent_final_labels_unlock_promotion() {
    let (mut b, _, e, c) = fixture(protocol());
    for id in 1..=4 {
        let (_, input) = prepare(&mut b, &c, id, 1); review(&mut b, id, &input, if id <= 2 { [Verdict::Hold; 2] } else { [Verdict::Allow; 2] });
        label(&mut b, &e, id, id, if id == 4 { GroundTruth::Censored } else if id <= 2 { GroundTruth::Violation } else { GroundTruth::Benign });
    }
    let report = b.credibility_report().unwrap(); assert_eq!(report.censored_cases, 1); assert!(!report.qualified());
    let before = b.inspect(); assert_eq!(b.promote_credibility(before.sequence, before.ledger.epoch, report.revision), Err(Error::Incomplete));
    label(&mut b, &e, 4, 4, GroundTruth::Benign); assert_eq!(b.evaluation_history(14).unwrap().len(), 2);
    let qualified = b.credibility_report().unwrap(); assert!(qualified.qualified());
    assert_eq!(b.promote_credibility(before.sequence, before.ledger.epoch, report.revision), Err(Error::Stale));
    b.promote_credibility(before.sequence, before.ledger.epoch, qualified.revision).unwrap();
}
#[test]
fn false_alarm_perfection_cannot_promote_a_helper_that_misses_every_violation() {
    let (mut b, _, e, c) = fixture(protocol());
    for id in 1..=4 { let (_, input) = prepare(&mut b, &c, id, 1); review(&mut b, id, &input, [Verdict::Allow; 2]); label(&mut b, &e, id, id, if id <= 2 { GroundTruth::Violation } else { GroundTruth::Benign }); }
    let report = b.credibility_report().unwrap(); assert_eq!(report.members["alice"].false_positives, 0);
    assert_eq!(report.members["alice"].false_negatives, 2); assert!(report.failures.contains(&QualificationFailure::Recall("alice".into())));
    let state = b.inspect(); assert_eq!(b.promote_credibility(state.sequence, state.ledger.epoch, report.revision), Err(Error::Incomplete)); assert_eq!(b.inspect(), state);
}
#[test]
fn false_stop_exhaustion_opens_incident_without_suppressing_helpers_or_blocking_cancellation() {
    let mut p = protocol(); p.false_stop_budget = 1;
    let (mut b, _, e, c) = fixture(p); let proposals = train(&mut b, &e, &c);
    let _permit = b.authorize(3, Some(&proposals[2].1), &snapshot()).unwrap();
    let (_, input) = prepare(&mut b, &c, 5, 1); review(&mut b, 5, &input, [Verdict::Hold; 2]); label(&mut b, &e, 5, 5, GroundTruth::Benign);
    let report = b.credibility_report().unwrap(); assert!(report.calibration_incident_open); assert_eq!(report.lifetime_false_stops, 1);
    let state = b.inspect(); assert_eq!(b.promote_credibility(state.sequence, state.ledger.epoch, report.revision), Err(Error::Incomplete));
    assert!(b.credibility_promotions().unwrap().is_empty()); b.cancel(3).unwrap(); assert_eq!(b.inspect().ledger.available, 100);
}
#[test]
fn same_origin_repetitions_do_not_supply_extra_attack_trials() {
    let (mut b, _, e, c) = fixture(protocol());
    for id in 1..=4 {
        let (_, input) = prepare(&mut b, &c, id, 1); review(&mut b, id, &input, if id <= 2 { [Verdict::Hold; 2] } else { [Verdict::Allow; 2] });
        label(&mut b, &e, id, if id <= 2 { 1 } else { 2 }, if id <= 2 { GroundTruth::Violation } else { GroundTruth::Benign });
    }
    let report = b.credibility_report().unwrap(); assert_eq!((report.scoped_cases, report.violation_origins, report.benign_origins), (4, 1, 1)); assert!(!report.qualified());
}
#[test]
fn evaluator_credentials_are_controller_bound_and_cannot_be_replaced_after_bootstrap() {
    let (mut a, _, ea, c) = fixture(protocol()); let (mut b, _, eb, _) = fixture(protocol());
    let (_, input) = prepare(&mut a, &c, 1, 1); review(&mut a, 1, &input, [Verdict::Allow; 2]);
    let ticket = a.evaluation_ticket(11).unwrap(); let assessment = Assessment { origin: 1, evidence_id: [1; 32], truth: GroundTruth::Benign };
    assert_eq!(eb.assess(&ticket, assessment).unwrap_err(), Error::Binding);
    let label = ea.assess(&ticket, assessment).unwrap(); assert_eq!(b.record_evaluation(label), Err(Error::Binding));
    assert_eq!(a.enable_credibility(protocol()).unwrap_err(), Error::Duplicate);
}
#[test]
fn checkpoint_reset_preserves_labels_and_policy_change_does_not_reuse_old_stratum_denominators() {
    let (mut b, _, e, c) = fixture(protocol()); let checkpoint = b.capture_checkpoint(1, 0).unwrap(); train(&mut b, &e, &c);
    let report = b.credibility_report().unwrap(); let state = b.inspect();
    b.reset(ResetRequest { checkpoint, expected_control_sequence: state.sequence, expected_actor_revision: b.actor_revision(),
        binding: ReviewBinding { round: 90, evidence_root: [2; 32], reducer_generation: 1 }, retained_targets: TargetCeiling::new(&[spec(0, 1).target.unwrap()]).unwrap() }).unwrap();
    assert_eq!(b.credibility_report().unwrap(), report); assert_eq!(b.evaluation_history(11).unwrap().len(), 1);
    let state = b.inspect(); b.replace_policy(state.sequence, state.ledger.epoch, exact_policy(2)).unwrap();
    let next = b.credibility_report().unwrap(); assert_eq!(next.retained_cases, 4); assert_eq!(next.scoped_cases, 0); assert!(!next.qualified());
}
