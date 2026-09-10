//! Pre-action forecasting through actual reference congress, two-key delivery
//! and endpoint reconciliation. Supplied activations are not a real host capture.

use fa_reference::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
use fa_reference::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastModel, ForecastRegistration};
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::PublicationEndpoint;
use fa_reference::action::consequence::gate::{ReviewBinding, TargetCeiling};
use fa_reference::action::consequence::gate::containment::{ActorState, ResetRequest, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, ObservedReview, OversightBroker, ReviewWindow, action_frame};
use fa_reference::action::consequence::oversight::consistency::ConsistencyConfig;
use fa_reference::action::consequence::oversight::human::{HumanDisposition, HumanReviewPolicy};
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::{Error, Snapshot};
use std::collections::BTreeMap;

fn spec(epoch: u64, payload: &[u8]) -> ActionSpec {
    ActionSpec { version: VERSION,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 }),
        payload: payload.to_vec(), required_witnesses: vec![], policy_epoch: epoch, deadline: ElapsedTick(100), units: 16 }
}
fn actor(tokens: usize) -> ActorState {
    ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1,
        tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart },
        vec![1; tokens], vec![2], vec![3], tokens as u64).unwrap()
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn profile() -> CaptureProfile { CaptureProfile { tenant: 1, model: 2, model_generation: 1, tap: 4, layout_generation: 5 } }
fn source(sequence: u64, position: u64) -> SourceFrame {
    SourceFrame::capture(FrameIdentity { profile: profile(), stream: 7, sequence, position }, &[-1.0]).unwrap()
}
fn consistency(max_predictions: usize) -> ConsistencyConfig {
    let forecast = BinaryForecast::new(16_384, 49_152).unwrap();
    ConsistencyConfig {
        model: ForecastModel::new(LinearProbe::new(1, 1, profile(), &[1.0], 0.0, 0.0).unwrap(), ForecastRegistration {
            domain: 11, generation: 1, policy_generation: 1, event_prefix: b"publish".to_vec(),
            negative: forecast, at_threshold: forecast, positive: forecast,
        }).unwrap(),
        alpha: ErrorBudget::new(1, 4).unwrap(), stream: 7, max_predictions, max_prediction_age_ticks: 8,
    }
}
fn bare() -> (OversightBroker, PublicationEndpoint, CommitteeContract) {
    let contracts = CommitteeContract::new(BTreeMap::from([("helper".to_owned(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"profile".to_vec(), model_epoch: 1, tokenizer_epoch: 1, policy_epoch: 0 },
        9, b"Review".to_vec(),
    ).unwrap())])).unwrap();
    let target = spec(0, b"publish").target.unwrap();
    let config = ControllerConfig {
        scope: spec(0, b"publish").scope, total: 100, max_attempts: 16, actor: actor(1), suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::UnitsAtMost(16)]).unwrap(),
        congress: CongressPolicy {
            generation: 1, members: BTreeMap::from([("helper".to_owned(), MemberPolicy { cohort: "a".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1,
        }, narrowed_targets: TargetCeiling::new(&[target]).unwrap(),
    };
    let mut endpoint = PublicationEndpoint::new(target, b"old".to_vec(), 200, 16).unwrap();
    let mut broker = OversightBroker::new(config, &mut endpoint, contracts.clone()).unwrap();
    broker.observe_time(ElapsedTick(1)).unwrap(); endpoint.observe_time(ElapsedTick(1)).unwrap();
    broker.confirm_fence(endpoint.install_fence(broker.fence_request()).unwrap()).unwrap();
    (broker, endpoint, contracts)
}
fn fixture(limit: usize) -> (OversightBroker, PublicationEndpoint, CommitteeContract) {
    let (mut broker, endpoint, contracts) = bare();
    broker.enable_action_consistency(consistency(limit)).unwrap();
    (broker, endpoint, contracts)
}
fn inputs(action: &FrozenAction, contracts: &CommitteeContract) -> CommitteeInput {
    let helper = &contracts.members()["helper"];
    let mut bytes = action_frame(action); let end = bytes.len(); bytes.extend_from_slice(helper.question());
    let input = ActualHelperInput::new(bytes.clone(), helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { span: ByteSpan { start: 0, end }, kind: PartKind::Other },
        SubmittedPart { span: ByteSpan { start: end, end: bytes.len() }, kind: PartKind::Question },
    ], vec![]).unwrap();
    let manifest = EvidenceViewManifest::new(input, AuthorizationProjection {
        projection_id: 9, policy_epoch: action.spec().policy_epoch, projected_originals: vec![],
    }, vec![]).unwrap();
    CommitteeInput::capture(action, contracts, BTreeMap::from([("helper".to_owned(), manifest)])).unwrap()
}
fn prepare(broker: &mut OversightBroker, contracts: &CommitteeContract, id: u64, payload: &[u8]) -> (FrozenAction, CommitteeInput) {
    broker.forecast_action(id, broker.actor_revision(), &source(id, 0)).unwrap();
    let proposal = broker.propose(id, spec(broker.inspect().ledger.epoch, payload), &snapshot()).unwrap();
    let input = inputs(&proposal.action, contracts); broker.record_inputs(id, 0, input.clone()).unwrap();
    (proposal.action, input)
}
fn completed(broker: &mut OversightBroker, id: u64, round: u64, verdict: Verdict) -> ObservedReview {
    let now = broker.inspect().ledger.elapsed.unwrap();
    let mut session = broker.begin_review(id, round, [1; 32], ReviewWindow {
        commit_by: ElapsedTick(now.0 + 5), reveal_by: ElapsedTick(now.0 + 10),
    }, &snapshot()).unwrap();
    let vote = session.commitment("helper", verdict, b"salt").unwrap();
    session.commit("helper", vote, now).unwrap(); session.open_reveals(now).unwrap();
    session.reveal("helper", verdict, b"salt", now).unwrap(); session.finish(now).unwrap()
}
fn approve(broker: &mut OversightBroker, id: u64, round: u64, input: &CommitteeInput) {
    let review = completed(broker, id, round, Verdict::Allow);
    broker.apply_review(review, Some(input), &snapshot()).unwrap();
}

#[test]
fn forecast_precedes_action_and_quiet_evidence_still_needs_congress() {
    let (mut broker, mut endpoint, contracts) = fixture(16);
    assert_eq!(broker.propose(1, spec(0, b"other"), &snapshot()), Err(Error::Incomplete));
    assert!(broker.inspect().ledger.stages.is_empty());
    let (action, input) = prepare(&mut broker, &contracts, 1, b"other");
    assert!(!broker.consistency_observation(1).unwrap().event());
    assert_eq!(broker.consistency_evidence().unwrap().samples(), 1);
    assert!(broker.authorize(1, Some(&input), &snapshot()).is_err());
    approve(&mut broker, 1, 11, &input);
    let permit = broker.authorize(1, Some(&input), &snapshot()).unwrap();
    let message = broker.dispatch(&permit, &action, Some(&input), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.payload(), b"other");
    assert_eq!(broker.inspect().ledger.charged, 16);
}

#[test]
fn repeated_mismatch_blocks_an_already_issued_permit_and_new_approval() {
    let (mut broker, endpoint, contracts) = fixture(16);
    let (first, input) = prepare(&mut broker, &contracts, 1, b"publish");
    approve(&mut broker, 1, 11, &input);
    let permit = broker.authorize(1, Some(&input), &snapshot()).unwrap();
    let (_, second) = prepare(&mut broker, &contracts, 2, b"publish again");
    assert_eq!(broker.consistency_evidence().unwrap().first_crossing(), Some(2));
    assert_eq!(broker.dispatch(&permit, &first, Some(&input), &snapshot()).unwrap_err(), Error::Incomplete);
    let approval = completed(&mut broker, 2, 12, Verdict::Allow);
    assert_eq!(broker.apply_review(approval, Some(&second), &snapshot()).unwrap_err(), Error::Incomplete);
    assert_eq!(endpoint.execution_count(), 0);
    assert_eq!(broker.inspect().ledger.stages[&2], ActionState::Reviewing);
    assert_eq!(broker.inspect().ledger.reserved, 16);
    broker.cancel(1).unwrap();
    assert_eq!(broker.inspect().ledger.available, 100);
    let hold = completed(&mut broker, 2, 13, Verdict::Hold);
    broker.apply_review(hold, None, &snapshot()).unwrap();
    assert!(broker.consistency_evidence().unwrap().crossed());
}

#[test]
fn exact_denial_is_observed_instead_of_selectively_dropped() {
    let (mut broker, _, _) = fixture(16);
    broker.forecast_action(1, 0, &source(1, 0)).unwrap();
    let mut forbidden = spec(0, b"publish"); forbidden.units = 17;
    let proposal = broker.propose(1, forbidden, &snapshot()).unwrap();
    assert_eq!(proposal.state, ActionState::Denied);
    assert_eq!(broker.consistency_evidence().unwrap().samples(), 1);
    assert!(broker.consistency_observation(1).unwrap().event());
    assert_eq!(broker.propose(1, spec(0, b"other"), &snapshot()), Err(Error::Duplicate));
    assert_eq!(broker.forecast_action(1, 0, &source(2, 0)).unwrap_err(), Error::Duplicate);
}

#[test]
fn missing_policy_snapshot_does_not_erase_the_observed_category() {
    let (mut broker, _, _) = fixture(16);
    broker.forecast_action(1, 0, &source(1, 0)).unwrap();
    let mut unavailable = snapshot(); unavailable.complete = false;
    assert_eq!(broker.propose(1, spec(0, b"publish"), &unavailable), Err(Error::Incomplete));
    assert!(broker.inspect().ledger.stages.is_empty());
    assert_eq!(broker.consistency_evidence().unwrap().samples(), 1);
    assert!(broker.consistency_observation(1).unwrap().event());
    assert_eq!(broker.pending_forecast(), Ok(None));
    assert_eq!(broker.propose(1, spec(0, b"other"), &snapshot()), Err(Error::Duplicate));
}

#[test]
fn outstanding_forecast_cannot_be_rerolled_or_skipped_to_dispatch_old_work() {
    let (mut broker, _, contracts) = fixture(16);
    let (action, input) = prepare(&mut broker, &contracts, 1, b"other");
    approve(&mut broker, 1, 11, &input);
    let permit = broker.authorize(1, Some(&input), &snapshot()).unwrap();
    broker.forecast_action(2, 0, &source(2, 0)).unwrap();
    assert_eq!(broker.forecast_action(3, 0, &source(3, 0)).unwrap_err(), Error::WrongState);
    assert_eq!(broker.propose(3, spec(0, b"other"), &snapshot()), Err(Error::Binding));
    assert_eq!(broker.dispatch(&permit, &action, Some(&input), &snapshot()).unwrap_err(), Error::Incomplete);
    assert_eq!(broker.consistency_evidence().unwrap().samples(), 1);
    broker.propose(2, spec(0, b"other"), &snapshot()).unwrap();
    broker.dispatch(&permit, &action, Some(&input), &snapshot()).unwrap();
}

#[test]
fn capture_identity_position_and_sequence_are_checked_before_admission() {
    let (mut broker, _, _) = fixture(16);
    assert_eq!(broker.forecast_action(1, 1, &source(1, 0)).unwrap_err(), Error::Stale);
    for field in 0..4 {
        let mut identity = source(1, 0).identity();
        match field { 0 => identity.profile.model += 1, 1 => identity.stream += 1,
            2 => identity.position += 1, _ => identity.profile.tenant += 1 }
        let bad = SourceFrame::capture(identity, &[-1.0]).unwrap();
        assert_eq!(broker.forecast_action(1, 0, &bad).unwrap_err(), Error::Binding);
    }
    assert_eq!(broker.pending_forecast(), Ok(None));
    broker.forecast_action(1, 0, &source(1, 0)).unwrap();
    broker.propose(1, spec(0, b"other"), &snapshot()).unwrap();
    assert_eq!(broker.forecast_action(2, 0, &source(1, 0)).unwrap_err(), Error::Stale);
    broker.forecast_action(2, 0, &source(2, 0)).unwrap();
}

#[test]
fn expired_prediction_stays_unresolved_instead_of_becoming_a_free_retry() {
    let (mut broker, _, _) = fixture(16);
    broker.forecast_action(1, 0, &source(1, 0)).unwrap();
    broker.observe_time(ElapsedTick(9)).unwrap();
    assert_eq!(broker.propose(1, spec(0, b"publish"), &snapshot()), Err(Error::Stale));
    assert_eq!(broker.pending_forecast(), Ok(Some(1)));
    assert_eq!(broker.forecast_action(2, 0, &source(2, 0)).unwrap_err(), Error::WrongState);
    assert_eq!(broker.consistency_evidence().unwrap().samples(), 0);
}

#[test]
fn actor_may_generate_after_prediction_but_later_state_cannot_reuse_approval() {
    let (mut broker, _, contracts) = fixture(16);
    broker.forecast_action(1, 0, &source(1, 0)).unwrap();
    broker.replace_actor_state(0, actor(2)).unwrap();
    let proposal = broker.propose(1, spec(0, b"other"), &snapshot()).unwrap();
    let observation = broker.consistency_observation(1).unwrap();
    assert_eq!(observation.forecast_actor_revision(), 0);
    assert_eq!(observation.observed_actor_revision(), 1);
    let input = inputs(&proposal.action, &contracts); broker.record_inputs(1, 0, input.clone()).unwrap();
    approve(&mut broker, 1, 11, &input);
    let permit = broker.authorize(1, Some(&input), &snapshot()).unwrap();
    broker.replace_actor_state(1, actor(3)).unwrap();
    assert_eq!(broker.dispatch(&permit, &proposal.action, Some(&input), &snapshot()).unwrap_err(), Error::Stale);
    assert_eq!(broker.consistency_evidence().unwrap().samples(), 1);
}

#[test]
fn reset_preserves_likelihood_and_old_sequence_cannot_restart_the_test() {
    let (mut broker, _, contracts) = fixture(16);
    let checkpoint = broker.capture_checkpoint(1, 0).unwrap();
    prepare(&mut broker, &contracts, 1, b"publish");
    broker.reset(ResetRequest {
        checkpoint, expected_control_sequence: broker.inspect().sequence,
        expected_actor_revision: broker.actor_revision(),
        binding: ReviewBinding { round: 90, evidence_root: [9; 32], reducer_generation: 1 },
        retained_targets: TargetCeiling::new(&[spec(0, b"publish").target.unwrap()]).unwrap(),
    }).unwrap();
    assert_eq!(broker.consistency_evidence().unwrap().samples(), 1);
    assert_eq!(broker.forecast_action(2, broker.actor_revision(), &source(1, 0)).unwrap_err(), Error::Stale);
    prepare(&mut broker, &contracts, 2, b"publish");
    assert_eq!(broker.consistency_evidence().unwrap().first_crossing(), Some(2));
    assert_eq!(broker.inspect().ledger.epoch, 1);
    assert_eq!(broker.incident_count(), 1);
    assert_eq!(broker.enable_action_consistency(consistency(16)), Err(Error::Duplicate));
}

#[test]
fn latched_alarm_does_not_block_real_endpoint_receipts_or_refund_unknowns() {
    let (mut broker, mut endpoint, contracts) = fixture(16);
    let (action, input) = prepare(&mut broker, &contracts, 1, b"publish");
    approve(&mut broker, 1, 11, &input);
    let permit = broker.authorize(1, Some(&input), &snapshot()).unwrap();
    let message = broker.dispatch(&permit, &action, Some(&input), &snapshot()).unwrap();
    let receipt = endpoint.deliver(&message).unwrap(); broker.acknowledgment_lost(1).unwrap();
    prepare(&mut broker, &contracts, 2, b"publish");
    assert!(broker.consistency_evidence().unwrap().crossed());
    assert_eq!(broker.cancel(1), Err(Error::WrongState));
    assert_eq!(broker.inspect().ledger.charged, 16);
    broker.accept_receipt(receipt).unwrap();
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(broker.inspect().ledger.available, 84);
    assert_eq!(endpoint.execution_count(), 1);
}

#[test]
fn human_key_obeys_the_same_sequential_hold_and_successful_control() {
    for second_is_event in [false, true] {
        let (mut broker, mut endpoint, contracts) = fixture(16);
        let reviewer = broker.enable_human_review(HumanReviewPolicy {
            reviewer_id: 9, max_validity_ticks: 20, max_requests: 8,
        }).unwrap();
        let (action, input) = prepare(&mut broker, &contracts, 1, b"publish");
        approve(&mut broker, 1, 11, &input);
        let automatic = broker.authorize(1, Some(&input), &snapshot()).unwrap();
        let request = broker.request_human_approval(101, 1, Some(&input), ElapsedTick(10)).unwrap();
        let human = reviewer.approve(&request, ElapsedTick(1)).unwrap();
        prepare(&mut broker, &contracts, 2, if second_is_event { b"publish" } else { b"other" });
        let result = broker.dispatch_with_human(&automatic, &human, &action, Some(&input), &snapshot());
        if second_is_event {
            assert_eq!(result.unwrap_err(), Error::Incomplete);
            assert_eq!(broker.human_status(101).unwrap().disposition, HumanDisposition::Approved);
            assert_eq!(endpoint.execution_count(), 0);
            assert_eq!(broker.inspect().ledger.reserved, 16);
        } else {
            broker.accept_receipt(endpoint.deliver(&result.unwrap()).unwrap()).unwrap();
            assert_eq!(broker.human_status(101).unwrap().disposition, HumanDisposition::Consumed);
            assert_eq!(endpoint.execution_count(), 1);
        }
    }
}

#[test]
fn capture_capacity_holds_prior_approval_but_allows_cancellation() {
    let (mut broker, _, contracts) = fixture(1);
    let (action, input) = prepare(&mut broker, &contracts, 1, b"other");
    approve(&mut broker, 1, 11, &input);
    let permit = broker.authorize(1, Some(&input), &snapshot()).unwrap();
    assert_eq!(broker.forecast_action(2, 0, &source(2, 0)).unwrap_err(), Error::Limit);
    assert_eq!(broker.consistency_coverage_lost(), Ok(true));
    assert_eq!(broker.dispatch(&permit, &action, Some(&input), &snapshot()).unwrap_err(), Error::Incomplete);
    broker.cancel(1).unwrap();
    assert_eq!(broker.inspect().ledger.available, 100);
}

#[test]
fn capture_outage_and_changed_policy_cannot_reset_calibration_history() {
    let (mut broker, _, contracts) = fixture(16);
    prepare(&mut broker, &contracts, 1, b"publish");
    let before = broker.consistency_evidence().unwrap().clone();
    broker.replace_policy(0, 0, Policy::new(2, vec![Predicate::UnitsAtMost(16)]).unwrap()).unwrap();
    assert_eq!(broker.forecast_action(2, 0, &source(2, 0)).unwrap_err(), Error::Stale);
    assert_eq!(broker.consistency_evidence().unwrap(), &before);
    broker.consistency_unavailable().unwrap();
    assert_eq!(broker.forecast_action(2, 0, &source(2, 0)).unwrap_err(), Error::Incomplete);
    assert_eq!(broker.consistency_evidence().unwrap(), &before);
}

#[test]
fn bootstrap_validation_and_unconfigured_behavior_are_preserved() {
    let (mut broker, _, _) = bare();
    assert_eq!(broker.enable_action_consistency(consistency(0)), Err(Error::InvalidInput));
    assert_eq!(broker.enable_action_consistency(consistency(513)), Err(Error::Limit));
    assert!(!broker.action_consistency_required());
    broker.propose(1, spec(0, b"other"), &snapshot()).unwrap();
    assert_eq!(broker.enable_action_consistency(consistency(16)), Err(Error::WrongState));
}
