//! Native controls and original endpoint outcomes, not caller-asserted stops.
use super::*;
use crate::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
use crate::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastModel, ForecastRegistration};
use crate::action::consequence::activation::probe::LinearProbe;
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::{EndpointOutcome, PublicationEndpoint};
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::action::consequence::gate::TargetCeiling;
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, ReviewWindow, action_frame};
use crate::action::consequence::oversight::consistency::ConsistencyConfig;
use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use crate::reducer::Caps;
use crate::round::Verdict;
use crate::Snapshot;
use std::collections::BTreeMap;

fn policy() -> ConsistencyStopPolicy { ConsistencyStopPolicy::new(11, 1, 7007).unwrap() }
fn profile() -> CaptureProfile {
    CaptureProfile { tenant: 1, model: 2, model_generation: 1, tap: 4, layout_generation: 5 }
}
fn source(sequence: u64) -> SourceFrame {
    SourceFrame::capture(FrameIdentity { profile: profile(), stream: 7, sequence, position: 0 }, &[-1.0]).unwrap()
}
fn spec(epoch: u64, payload: &[u8]) -> ActionSpec {
    ActionSpec { version: VERSION,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 }),
        payload: payload.to_vec(), required_witnesses: vec![], policy_epoch: epoch, deadline: ElapsedTick(100), units: 16 }
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn fixture(stopping: bool, limit: usize, stream: Option<StreamProfile>) -> (OversightBroker, PublicationEndpoint) {
    let contracts = CommitteeContract::new(BTreeMap::from([("helper".to_owned(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"profile".to_vec(), model_epoch: 1, tokenizer_epoch: 1, policy_epoch: 0 },
        9, b"Review".to_vec(),
    ).unwrap())])).unwrap();
    let target = spec(0, b"risk").target.unwrap();
    let config = ControllerConfig {
        scope: spec(0, b"risk").scope, total: 100, max_attempts: 16,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
            model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
            grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::UnitsAtMost(16)]).unwrap(),
        congress: CongressPolicy {
            generation: 1, members: BTreeMap::from([("helper".to_owned(), MemberPolicy { cohort: "a".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1,
        }, narrowed_targets: TargetCeiling::new(&[target]).unwrap(),
    };
    let mut endpoint = match stream {
        None => PublicationEndpoint::new(target, b"old".to_vec(), 200, 16).unwrap(),
        Some(stream) => PublicationEndpoint::new_stream(target, stream, 200, 16).unwrap(),
    };
    let mut owner = OversightBroker::new(config, &mut endpoint, contracts).unwrap();
    owner.observe_time(ElapsedTick(1)).unwrap(); endpoint.observe_time(ElapsedTick(1)).unwrap();
    owner.confirm_fence(endpoint.install_fence(owner.fence_request()).unwrap()).unwrap();
    let pair = BinaryForecast::new(16384, 49152).unwrap();
    owner.enable_action_consistency(ConsistencyConfig {
        model: ForecastModel::new(LinearProbe::new(1, 1, profile(), &[1.0], 0.0, 0.0).unwrap(), ForecastRegistration {
            domain: 11, generation: 1, policy_generation: 1, event_prefix: b"risk".to_vec(),
            negative: pair, at_threshold: pair, positive: pair,
        }).unwrap(), alpha: ErrorBudget::new(1, 4).unwrap(), stream: 7,
        max_predictions: limit, max_prediction_age_ticks: 8,
    }).unwrap();
    if stopping { owner.enable_consistency_stop(policy()).unwrap(); }
    (owner, endpoint)
}
fn propose(owner: &mut OversightBroker, id: u64, payload: &[u8]) -> Result<FrozenAction, Error> {
    owner.forecast_action(id, owner.actor_revision(), &source(id))?;
    owner.propose(id, spec(owner.inspect().ledger.epoch, payload), &snapshot()).map(|p| p.action)
}
fn inputs(owner: &OversightBroker, action: &FrozenAction) -> CommitteeInput {
    let helper = &owner.contracts().members()["helper"];
    let mut bytes = action_frame(action); let end = bytes.len(); bytes.extend_from_slice(helper.question());
    let input = ActualHelperInput::new(bytes.clone(), helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { span: ByteSpan { start: 0, end }, kind: PartKind::Other },
        SubmittedPart { span: ByteSpan { start: end, end: bytes.len() }, kind: PartKind::Question },
    ], vec![]).unwrap();
    let view = EvidenceViewManifest::new(input, AuthorizationProjection {
        projection_id: 9, policy_epoch: action.spec().policy_epoch, projected_originals: vec![],
    }, vec![]).unwrap();
    CommitteeInput::capture(action, owner.contracts(), BTreeMap::from([("helper".to_owned(), view)])).unwrap()
}
fn review(owner: &mut OversightBroker, action: &FrozenAction, id: u64) -> CommitteeInput {
    let input = inputs(owner, action); owner.record_inputs(id, 0, input.clone()).unwrap();
    let now = ElapsedTick(1);
    let mut session = owner.begin_review(id, 100 + id, [1; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(10),
    }, &snapshot()).unwrap();
    let digest = session.commitment("helper", Verdict::Allow, b"salt").unwrap();
    session.commit("helper", digest, now).unwrap(); session.open_reveals(now).unwrap();
    session.reveal("helper", Verdict::Allow, b"salt", now).unwrap();
    owner.apply_review(session.finish(now).unwrap(), Some(&input), &snapshot()).unwrap(); input
}

#[test]
fn opt_in_quiet_work_still_needs_congress_and_reaches_the_original_endpoint() {
    for stopping in [false, true] {
        let (mut owner, mut endpoint) = fixture(stopping, 8, None);
        assert!(owner.propose(1, spec(0, b"safe"), &snapshot()).is_err());
        let action = propose(&mut owner, 1, b"safe").unwrap();
        let input = inputs(&owner, &action);
        assert!(owner.authorize(1, Some(&input), &snapshot()).is_err());
        let input = review(&mut owner, &action, 1);
        let key = owner.authorize(1, Some(&input), &snapshot()).unwrap();
        let message = owner.dispatch(&key, &action, Some(&input), &snapshot()).unwrap();
        let receipt = endpoint.deliver(&message).unwrap();
        owner.accept_receipt(receipt).unwrap();
        assert_eq!(endpoint.payload(), b"safe"); assert_eq!(endpoint.execution_count(), 1);
        assert_eq!(owner.consistency_evidence().unwrap().samples(), 1);
        assert!(owner.consistency_stop_incident().is_none()); assert!(owner.stop_receipt().is_none());
    }
}

#[test]
fn a_real_crossing_cancels_unsent_work_but_requires_native_drain_for_sent_work() {
    // The same first proposal is unsent, sent but absent, or executed with a lost acknowledgment.
    for state in 0..3 {
        let (mut owner, mut endpoint) = fixture(true, 8, None);
        let action = propose(&mut owner, 1, b"risk one").unwrap();
        let input = review(&mut owner, &action, 1);
        let key = owner.authorize(1, Some(&input), &snapshot()).unwrap();
        if state != 0 {
            let message = owner.dispatch(&key, &action, Some(&input), &snapshot()).unwrap();
            if state == 2 { assert!(matches!(endpoint.deliver(&message).unwrap().outcome(), EndpointOutcome::Executed { .. })); }
        }
        assert_eq!(propose(&mut owner, 2, b"risk two"), Err(Error::WrongState));
        assert_eq!(owner.consistency_evidence().unwrap().samples(), 2);
        assert!(owner.consistency_observation(2).unwrap().crossed());
        let incident = owner.consistency_stop_incident().unwrap().clone();
        assert_eq!(incident.cause, ConsistencyStopCause::ThresholdCrossed { first_sample: 2 });
        assert_eq!(incident.receipt.as_ref().unwrap().request().operation, 7007);
        assert!(owner.inspect().suspended); assert!(!owner.inspect().ledger.stages.contains_key(&2));
        assert_eq!(owner.inspect().ledger.reserved, 0);
        assert_eq!(owner.inspect().ledger.charged, if state == 0 { 0 } else { 16 });
        assert_eq!(owner.inspect().ledger.stages[&1], if state == 0 { ActionState::Cancelled } else { ActionState::Unknown });
        let before = owner.inspect(); owner.enforce_consistency_stop().unwrap();
        assert_eq!(owner.inspect(), before); assert_eq!(owner.consistency_stop_incident(), Some(&incident));
        assert!(owner.dispatch(&key, &action, Some(&input), &snapshot()).is_err());
        let sweep = owner.progress_stop(&mut endpoint).unwrap(); assert!(sweep.progress.drained());
        assert_eq!(owner.inspect().ledger.charged, if state == 2 { 16 } else { 0 });
        assert_eq!(endpoint.execution_count(), u64::from(state == 2));
    }
}

#[test]
fn rejected_forecasts_do_not_stop_but_real_prediction_exhaustion_does() {
    let (mut owner, _) = fixture(true, 1, None);
    assert_eq!(owner.forecast_action(0, 0, &source(1)), Err(Error::InvalidInput));
    assert_eq!(owner.forecast_action(1, 7, &source(1)), Err(Error::Stale));
    assert!(owner.stop_receipt().is_none()); assert!(owner.consistency_stop_incident().is_none());
    propose(&mut owner, 1, b"safe").unwrap();
    assert_eq!(owner.forecast_action(2, 0, &source(1)), Err(Error::Stale));
    assert!(owner.stop_receipt().is_none());
    assert_eq!(owner.forecast_action(2, 0, &source(2)), Err(Error::Limit));
    let incident = owner.consistency_stop_incident().unwrap();
    assert_eq!(incident.cause, ConsistencyStopCause::CoverageLost);
    assert_eq!(incident.prediction_jobs, 1); assert_eq!(incident.observed_samples, 1);
    assert!(incident.receipt.is_some()); assert!(owner.consistency_evidence().unwrap().first_crossing().is_none());
}

#[test]
fn missing_coverage_keeps_pending_evidence_and_never_relabels_an_existing_stop() {
    let (mut owner, _) = fixture(true, 8, None);
    owner.forecast_action(1, 0, &source(1)).unwrap();
    let c = owner.inspect();
    let other = owner.request_stop(StopRequest { operation: 99,
        expected_control_sequence: c.sequence, expected_authority_epoch: c.ledger.epoch }).unwrap();
    owner.consistency_unavailable().unwrap();
    let incident = owner.consistency_stop_incident().unwrap().clone();
    assert_eq!(incident.policy, policy()); assert_eq!(incident.receipt, Some(other));
    assert_eq!(incident.pending_attempt, Some(1)); assert_eq!(incident.observed_samples, 0);
    assert_eq!(incident.cause, ConsistencyStopCause::CoverageLost);
    owner.consistency_unavailable().unwrap();
    assert_eq!(owner.consistency_stop_incident(), Some(&incident));
}

#[test]
fn malformed_stream_observation_is_lost_coverage_not_a_reassuring_negative() {
    let stream = StreamProfile::new(1, 1, 4, 64, 128).unwrap();
    let (mut owner, _) = fixture(true, 8, Some(stream));
    owner.require_stream_message_consistency(stream).unwrap();
    owner.forecast_action(1, 0, &source(1)).unwrap();
    assert!(owner.propose(1, spec(0, b"bad frame"), &snapshot()).is_err());
    assert_eq!(owner.pending_forecast().unwrap(), Some(1));
    assert_eq!(owner.consistency_evidence().unwrap().samples(), 0);
    assert!(owner.stop_receipt().is_some());
    assert_eq!(owner.consistency_stop_incident().unwrap().cause, ConsistencyStopCause::CoverageLost);
}

#[test]
fn policy_is_opt_in_frozen_and_not_retroactively_installed_after_a_prediction() {
    for values in [(0, 1, 1), (1, 0, 1), (1, 1, 0)] {
        assert_eq!(ConsistencyStopPolicy::new(values.0, values.1, values.2), Err(Error::InvalidInput));
    }
    let (mut owner, _) = fixture(true, 8, None);
    assert_eq!(owner.enable_consistency_stop(policy()), Err(Error::Duplicate));
    let (mut owner, _) = fixture(false, 8, None);
    owner.forecast_action(1, 0, &source(1)).unwrap();
    assert_eq!(owner.enable_consistency_stop(policy()), Err(Error::WrongState));
    owner.consistency_unavailable().unwrap();
    assert!(owner.stop_receipt().is_none()); assert!(owner.enforce_consistency_stop().unwrap().is_none());
}
