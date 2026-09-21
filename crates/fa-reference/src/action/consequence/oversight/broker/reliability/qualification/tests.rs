use super::*;
use crate::action::{ActionSpec, ElapsedTick, FrozenAction, Permit, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::congress::{CongressPolicy, CredibilityBinding, CredibilityRequirements, MemberPolicy};
use crate::action::consequence::congress::credibility::{Campaign, CaseSpec, CredibilityLedger,
    EvaluationLabel, EvaluationScope, HelperGeneration, LabelSource, LabelVerdict, Observation};
use crate::action::consequence::delivery::{EndpointOutcome, EndpointStatus, PublicationEndpoint};
use crate::action::consequence::gate::{TargetCeiling, containment::{ActorState, RestartGrade, RestartProfile}};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate, controller::ControllerConfig};
use crate::action::consequence::oversight::{CommitteeInput, HelperContract, ReviewWindow};
use crate::action::consequence::oversight::credibility::{EvaluationProtocol, Fraction};
use crate::action::consequence::oversight::joint_credibility::{JointPromotionPolicy, JointReplayBudget};
use crate::action::consequence::oversight::human::{HumanPermit, HumanReviewPolicy, HumanReviewer};
use crate::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::round::Verdict;
use crate::Snapshot;
use std::collections::{BTreeMap, BTreeSet};

fn scope() -> Scope {
    Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect }
}
fn target() -> ResolvedTarget {
    ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 }
}
fn actor() -> ActorState {
    ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
        model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
        grade: RestartGrade::ExactRestart }, vec![1], vec![2], vec![3], 1).unwrap()
}
fn contracts(model: u64) -> CommitteeContract {
    CommitteeContract::new(["a", "b"].into_iter().map(|name| (name.to_owned(),
        HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: b"profile".to_vec(),
            model_epoch: model, tokenizer_epoch: 1, policy_epoch: 0 }, 1, b"approve?".to_vec()).unwrap()
    )).collect()).unwrap()
}
fn setup() -> (OversightBroker, HumanReviewer, PublicationEndpoint) {
    let mut endpoint = PublicationEndpoint::new(target(), Vec::new(), 200, 32).unwrap();
    let mut host = OversightBroker::new(ControllerConfig { scope: scope(), total: 100,
        max_attempts: 64, actor: actor(), suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::Absent { key: 7 }]).unwrap(),
        congress: CongressPolicy { generation: 1, members: ["a", "b"].into_iter().map(|name|
            (name.to_owned(), MemberPolicy { cohort: name.to_owned(), weight: 8 })).collect(),
            caps: Caps { per_member: 10, per_cohort: 10 }, continue_minimum: 16,
            continue_hold_maximum: 0, narrow_at: 16, suspend_at: 20,
            minimum_members: 2, minimum_cohorts: 2 },
        narrowed_targets: TargetCeiling::new(&[target()]).unwrap(),
    }, &mut endpoint, contracts(1)).unwrap();
    let reviewer = host.enable_human_review(HumanReviewPolicy {
        reviewer_id: 7, max_validity_ticks: 100, max_requests: 32,
    }).unwrap();
    host.confirm_fence(endpoint.install_fence(host.fence_request()).unwrap()).unwrap();
    host.observe_time(ElapsedTick(1)).unwrap();
    endpoint.observe_time(ElapsedTick(1)).unwrap();
    (host, reviewer, endpoint)
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn spec(host: &OversightBroker) -> ActionSpec {
    ActionSpec { version: VERSION, scope: scope(), target: Some(target()), payload: b"hello".to_vec(),
        required_witnesses: Vec::new(), policy_epoch: host.inspect().ledger.epoch,
        deadline: ElapsedTick(100), units: 5 }
}
fn source(id: u64, text: &[u8]) -> EvidenceSnapshot {
    EvidenceSnapshot::new(EvidenceIdentity { source: 1, generation: id, scope: scope() },
        snapshot(), ["a", "b"].into_iter().map(|name| (name.to_owned(), text.to_vec())).collect()).unwrap()
}
fn reviewed(host: &mut OversightBroker, id: u64) -> (FrozenAction, CommitteeInput) {
    let source = source(id, b"context");
    let action = host.propose(id, spec(host), source.snapshot()).unwrap().action;
    let inputs = source.inputs_for(&action, host.contracts()).unwrap();
    host.record_inputs(id, 0, inputs.clone()).unwrap();
    let mut session = host.begin_review(id, id + 100, source.reference_root(),
        ReviewWindow { commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30) }, &snapshot()).unwrap();
    for name in ["a", "b"] {
        let digest = session.commitment(name, Verdict::Allow, b"salt").unwrap();
        session.commit(name, digest, ElapsedTick(1)).unwrap();
    }
    session.open_reveals(ElapsedTick(1)).unwrap();
    for name in ["a", "b"] { session.reveal(name, Verdict::Allow, b"salt", ElapsedTick(1)).unwrap(); }
    host.apply_review(session.finish(ElapsedTick(1)).unwrap(), Some(&inputs), &snapshot()).unwrap();
    (action, inputs)
}
fn seed(host: &mut OversightBroker) {
    for id in [1, 2] { reviewed(host, id); host.cancel(id).unwrap(); }
}
fn keys(host: &mut OversightBroker, reviewer: &HumanReviewer, id: u64)
    -> (FrozenAction, CommitteeInput, Permit, HumanPermit)
{
    let (action, inputs) = reviewed(host, id);
    let automatic = host.authorize(id, Some(&inputs), &snapshot()).unwrap();
    let request = host.request_human_approval(id, id, Some(&inputs), ElapsedTick(90)).unwrap();
    let human = reviewer.approve(&request, ElapsedTick(1)).unwrap();
    (action, inputs, automatic, human)
}
fn activation(host: &OversightBroker, operation: u64, generation: u64) -> CredibilityActivation {
    let mut ledger = CredibilityLedger::new(Campaign {
        scope: EvaluationScope { campaign: operation, model_generation: 1, evaluator_generation: 1,
            held_out_manifest: [9; 32] }, label_owner: "evaluator".into(),
        helpers: ["a", "b"].into_iter().map(|name| (name.to_owned(), HelperGeneration {
            generation: 1, cohort: name.to_owned() })).collect(),
        strata: BTreeSet::from(["publication".into()]),
        cases: [1, 2].into_iter().map(|id| CaseSpec { id, stratum: "publication".into(),
            evidence_root: [id as u8; 32], dispatch_sequence: 2 }).collect(),
    }).unwrap();
    for id in [1, 2] {
        let observation = if id == 1 { Observation::Clear } else { Observation::Hold { first_sequence: 1 } };
        ledger.record_observations(id, ["a", "b"].into_iter().map(|name| (name.to_owned(), observation)).collect()).unwrap();
        ledger.record_label(id, EvaluationLabel { owner: "evaluator".into(), evaluator_generation: 1,
            source: LabelSource::IndependentEvaluation, evidence_root: [id as u8; 32], recorded_sequence: 2,
            verdict: if id == 1 { LabelVerdict::Safe } else { LabelVerdict::Violation } }).unwrap();
    }
    let evidence = ledger.seal(2).unwrap();
    CredibilityActivation { operation, expected_control_sequence: host.inspect().sequence,
        expected_epoch: host.inspect().ledger.epoch, scope: scope(), policy_generation: 1,
        actor_profile: actor().profile(), binding: CredibilityBinding { scope: evidence.scope().clone(),
            label_owner: evidence.label_owner().into(), helpers: evidence.helpers().clone(),
            strata: evidence.strata().clone(), reducer_generation: generation }, stratum: "publication".into(),
        requirements: CredibilityRequirements { minimum_safe_cases: 1, minimum_violation_cases: 1,
            minimum_precision_ppm: 1_000_000, minimum_timely_recall_ppm: 1_000_000,
            maximum_false_positive_ppm: 0, base_weight: 10, lead_bonus_weight: 0,
            lead_saturation_sequences: 0, maximum_evidence_age: 100,
            maximum_member_share_ppm: 500_000, maximum_cohort_share_ppm: 500_000 }, snapshot: evidence }
}
fn withdrawal(host: &OversightBroker, operation: u64) -> CredibilityWithdrawalRequest {
    CredibilityWithdrawalRequest { operation, expected_control_sequence: host.inspect().sequence,
        expected_epoch: host.inspect().ledger.epoch }
}
fn protocol() -> EvaluationProtocol {
    EvaluationProtocol { domain: 1, stratum: 1, period: 1, minimum_violation_origins: 1,
        minimum_benign_origins: 1, precision_floor: Fraction { numerator: 1, denominator: 2 },
        recall_floor: Fraction { numerator: 1, denominator: 2 },
        false_positive_ceiling: Fraction { numerator: 1, denominator: 2 }, false_stop_budget: 10 }
}

#[test]
fn activation_invalidates_old_keys_but_the_same_two_key_pipeline_publishes_new_work() {
    let (mut host, reviewer, mut endpoint) = setup(); seed(&mut host);
    let (old_action, old_input, old_automatic, old_human) = keys(&mut host, &reviewer, 10);
    let request = activation(&host, 20, 2);
    let change = host.activate_credibility(request, &contracts(1)).unwrap();
    assert_eq!(change.refunded_units, 5);
    assert_eq!(host.inspect().ledger.available, 100);
    assert!(host.dispatch_with_human(&old_automatic, &old_human, &old_action, Some(&old_input), &snapshot()).is_err());
    let (action, input, automatic, human) = keys(&mut host, &reviewer, 11);
    assert!(host.dispatch(&automatic, &action, Some(&input), &snapshot()).is_err());
    assert!(host.dispatch_with_human(&automatic, &human, &action, Some(&input), &snapshot()).is_err());
    assert_eq!(endpoint.execution_count(), 0);
    host.confirm_fence(endpoint.install_fence(host.fence_request()).unwrap()).unwrap();
    let envelope = host.dispatch_with_human(&automatic, &human, &action, Some(&input), &snapshot()).unwrap();
    let receipt = endpoint.deliver(&envelope).unwrap();
    assert_eq!(receipt.outcome(), EndpointOutcome::Executed { resulting_version: 2 });
    host.accept_receipt(receipt).unwrap();
    assert_eq!(endpoint.payload(), b"hello");
    assert_eq!(host.inspect().ledger.charged, 5);
}

#[test]
fn complete_contract_and_authority_bindings_refuse_atomically_next_to_a_working_control() {
    let (mut host, _, endpoint) = setup(); seed(&mut host);
    let request = activation(&host, 20, 2); let before = host.inspect();
    assert_eq!(host.activate_credibility(request.clone(), &contracts(2)), Err(Error::Binding));
    let mut foreign = request.clone(); foreign.scope.tenant += 1;
    assert_eq!(host.activate_credibility(foreign, &contracts(1)), Err(Error::Binding));
    assert_eq!(host.inspect(), before);
    assert_eq!(host.credibility_changes().len(), 0);
    assert_eq!(endpoint.execution_count(), 0);
    host.activate_credibility(request, &contracts(1)).unwrap();
    assert_eq!(host.check_credibility(), Ok(()));
}

#[test]
fn historical_retries_do_not_clear_a_later_reviews_automatic_or_human_approval() {
    let (mut host, reviewer, mut endpoint) = setup(); seed(&mut host);
    let first = activation(&host, 20, 2);
    let original = host.activate_credibility(first.clone(), &contracts(1)).unwrap();
    let lost = withdrawal(&host, 21);
    let loss = host.withdraw_credibility(lost.clone()).unwrap();
    let next = activation(&host, 22, 3);
    host.activate_credibility(next, &contracts(1)).unwrap();
    host.confirm_fence(endpoint.install_fence(host.fence_request()).unwrap()).unwrap();
    let (action, input, automatic, human) = keys(&mut host, &reviewer, 10);
    let before = host.inspect();
    assert_eq!(host.activate_credibility(first, &contracts(1)).unwrap(), original);
    assert_eq!(host.withdraw_credibility(lost).unwrap(), loss);
    assert_eq!(host.inspect(), before);
    let envelope = host.dispatch_with_human(&automatic, &human, &action, Some(&input), &snapshot()).unwrap();
    endpoint.deliver(&envelope).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
}

#[test]
fn withdrawal_preserves_inflight_liability_until_the_original_endpoint_settles() {
    for executed in [false, true] {
        let (mut host, reviewer, mut endpoint) = setup(); seed(&mut host);
        let request = activation(&host, 20, 2);
        host.activate_credibility(request, &contracts(1)).unwrap();
        host.confirm_fence(endpoint.install_fence(host.fence_request()).unwrap()).unwrap();
        let (action, input, automatic, human) = keys(&mut host, &reviewer, 10);
        let envelope = host.dispatch_with_human(&automatic, &human, &action, Some(&input), &snapshot()).unwrap();
        if executed { endpoint.deliver(&envelope).unwrap(); }
        host.acknowledgment_lost(10).unwrap();
        let request = withdrawal(&host, 21); host.withdraw_credibility(request).unwrap();
        assert_eq!(host.check_credibility(), Err(Error::Stale));
        assert_eq!(host.inspect().ledger.charged, 5);
        host.confirm_fence(endpoint.install_fence(host.fence_request()).unwrap()).unwrap();
        assert!(endpoint.deliver(&envelope).is_err());
        let query = host.status_query(10).unwrap();
        let status = endpoint.status(&query).unwrap();
        assert_eq!(matches!(status, EndpointStatus::Resolved(_)), executed);
        let receipt = endpoint.seal_unexecuted(&query).unwrap();
        host.accept_receipt(receipt).unwrap();
        assert_eq!(host.inspect().ledger.charged, if executed { 5 } else { 0 });
        assert_eq!(endpoint.execution_count(), u64::from(executed));
    }
}

#[test]
fn qualified_weights_do_not_bypass_whole_input_equality() {
    let (mut host, reviewer, mut endpoint) = setup(); seed(&mut host);
    let request = activation(&host, 20, 2); host.activate_credibility(request, &contracts(1)).unwrap();
    host.confirm_fence(endpoint.install_fence(host.fence_request()).unwrap()).unwrap();
    let (action, input, automatic, human) = keys(&mut host, &reviewer, 10);
    let changed = source(11, b"changed context").inputs_for(&action, host.contracts()).unwrap();
    let before = host.inspect();
    assert!(host.dispatch_with_human(&automatic, &human, &action, Some(&changed), &snapshot()).is_err());
    assert_eq!(host.inspect(), before);
    let envelope = host.dispatch_with_human(&automatic, &human, &action, Some(&input), &snapshot()).unwrap();
    endpoint.deliver(&envelope).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
}

#[test]
fn offline_activation_cannot_bypass_an_owned_round_or_joint_replay_requirement() {
    for joint in [false, true] {
        let (mut host, _, _) = setup();
        let _evaluator = host.enable_credibility(protocol()).unwrap();
        if joint {
            host.enable_joint_credibility(JointPromotionPolicy::new(71, 1,
                Fraction { numerator: 0, denominator: 1 }, Fraction { numerator: 0, denominator: 1 },
                JointReplayBudget::default()).unwrap()).unwrap();
        }
        seed(&mut host);
        let request = activation(&host, 20, 2); let before = host.inspect();
        assert_eq!(host.activate_credibility(request, &contracts(1)), Err(Error::Binding));
        assert_eq!(host.inspect(), before);
        assert_eq!(host.credibility_changes().len(), 0);
    }
}

#[test]
fn owned_round_bootstrap_cannot_replace_an_active_offline_lane_even_without_attempts() {
    let (mut host, _, _) = setup();
    // Advance the original policy journal without ever admitting an attempt.
    for generation in [2, 3] {
        let before = host.inspect();
        host.replace_policy(before.sequence, before.ledger.epoch,
            Policy::new(generation, vec![Predicate::Absent { key: 7 }]).unwrap()).unwrap();
    }
    let mut request = activation(&host, 20, 2); request.policy_generation = 3;
    host.activate_credibility(request, &contracts(1)).unwrap();
    assert_eq!(host.enable_credibility(protocol()).unwrap_err(), Error::Binding);
    assert_eq!(host.check_credibility(), Ok(()));
}

#[test]
fn expired_evidence_stops_dispatch_but_not_exact_denial_or_cancellation() {
    let (mut host, reviewer, mut endpoint) = setup(); seed(&mut host);
    let mut request = activation(&host, 20, 2); request.requirements.maximum_evidence_age = 2;
    host.activate_credibility(request, &contracts(1)).unwrap(); // Sequence 3, valid through 4.
    host.confirm_fence(endpoint.install_fence(host.fence_request()).unwrap()).unwrap();
    let (action, input, automatic, human) = keys(&mut host, &reviewer, 10); // Sequence 4.
    reviewed(&mut host, 11); // Sequence 5; qualification now stale.
    assert_eq!(host.check_credibility(), Err(Error::Stale));
    assert!(host.dispatch_with_human(&automatic, &human, &action, Some(&input), &snapshot()).is_err());
    let mut denied = snapshot(); denied.values.insert(7, vec![1]);
    let proposal = host.propose(12, spec(&host), &denied).unwrap();
    assert_eq!(proposal.state, crate::action::ActionState::Denied);
    host.cancel(10).unwrap();
    assert_eq!(host.inspect().ledger.available, 100);
    assert_eq!(endpoint.execution_count(), 0);
}
