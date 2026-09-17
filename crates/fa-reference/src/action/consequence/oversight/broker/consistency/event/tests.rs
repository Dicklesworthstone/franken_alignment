use super::*;
use crate::action::{ActionState, ElapsedTick, Purpose, ResolvedTarget, Scope};
use crate::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
use crate::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
use crate::action::consequence::activation::probe::LinearProbe;
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::{PublicationEndpoint, stream::StreamView};
use crate::action::consequence::gate::TargetCeiling;
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate, controller::ControllerConfig};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract, consistency::ConsistencyConfig};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::Snapshot;
use std::collections::BTreeMap;

fn stream() -> StreamProfile { StreamProfile::new(7, 1, 4, 1024, 4096).unwrap() }
fn capture() -> CaptureProfile { CaptureProfile { tenant: 1, model: 2, model_generation: 1, tap: 4, layout_generation: 5 } }
fn model() -> ForecastModel {
    let pair = BinaryForecast::new(16384, 49152).unwrap();
    ForecastModel::new(LinearProbe::new(1, 1, capture(), &[1.0], 0.0, 0.0).unwrap(),
        ForecastRegistration { domain: 11, generation: 1, policy_generation: 1,
            event_prefix: b"risk".to_vec(), negative: pair, at_threshold: pair, positive: pair }).unwrap()
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn fixture(streamed: bool, units: u64) -> OversightBroker {
    let target = ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 };
    let scope = Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect };
    let contracts = CommitteeContract::new(BTreeMap::from([("helper".to_owned(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"profile".to_vec(),
            model_epoch: 1, tokenizer_epoch: 1, policy_epoch: 0 }, 9, b"Review".to_vec()).unwrap())])).unwrap();
    let config = ControllerConfig { scope, total: 10000, max_attempts: 16,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
            model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
            grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::UnitsAtMost(units)]).unwrap(),
        congress: CongressPolicy { generation: 1,
            members: BTreeMap::from([("helper".to_owned(), MemberPolicy { cohort: "a".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: TargetCeiling::new(&[target]).unwrap() };
    let mut endpoint = if streamed { PublicationEndpoint::new_stream(target, stream(), 200, 16).unwrap() }
        else { PublicationEndpoint::new(target, Vec::new(), 200, 16).unwrap() };
    let mut broker = OversightBroker::new(config, &mut endpoint, contracts).unwrap();
    broker.observe_time(ElapsedTick(1)).unwrap(); endpoint.observe_time(ElapsedTick(1)).unwrap();
    broker.confirm_fence(endpoint.install_fence(broker.fence_request()).unwrap()).unwrap();
    broker.enable_action_consistency(ConsistencyConfig { model: model(), alpha: ErrorBudget::new(1, 4).unwrap(),
        stream: 7, max_predictions: 16, max_prediction_age_ticks: 8 }).unwrap();
    broker
}
fn forecast(broker: &mut OversightBroker, attempt: u64) {
    let source = SourceFrame::capture(FrameIdentity { profile: capture(), stream: 7, sequence: attempt, position: 0 }, &[-1.0]).unwrap();
    broker.forecast_action(attempt, broker.actor_revision(), &source).unwrap();
}

#[test]
fn new_message_not_binary_header_or_prior_messages_defines_the_event() {
    let model = model(); let domain = ConsistencyEventDomain::StreamMessagePrefix(stream());
    let empty = StreamView::empty(stream()); let first = empty.encode_message("risk: α").unwrap();
    assert_eq!(ConsistencyEventDomain::PayloadPrefix.classify(&model, &first), Ok(false));
    assert_eq!(domain.classify(&model, &first), Ok(true));
    let prior = empty.advance(&first).unwrap();
    assert_eq!(domain.classify(&model, &prior.encode_message("ordinary β").unwrap()), Ok(false));
    assert_eq!(domain.classify(&model, &prior.encode_message("risk: β").unwrap()), Ok(true));
    assert_eq!(domain.classify(&model, &prior.encode_finish().unwrap()), Ok(false));
    assert_eq!(ConsistencyEventDomain::PayloadPrefix.classify(&model, b"risk: raw"), Ok(true));
}

#[test]
fn whole_frame_shape_and_exact_stream_contract_are_required() {
    let domain = ConsistencyEventDomain::StreamMessagePrefix(stream()); let model = model();
    let bytes = StreamView::empty(stream()).encode_message("risk").unwrap();
    for cut in 0..bytes.len() { assert!(domain.classify(&model, &bytes[..cut]).is_err()); }
    let mut trailing = bytes.clone(); trailing.push(0); assert!(domain.classify(&model, &trailing).is_err());
    let mut utf8 = bytes.clone(); *utf8.last_mut().unwrap() = 255; assert!(domain.classify(&model, &utf8).is_err());
    let foreign = StreamProfile::new(7, 2, 4, 1024, 4096).unwrap();
    assert_eq!(ConsistencyEventDomain::StreamMessagePrefix(foreign).classify(&model, &bytes), Err(Error::Binding));
    assert_eq!(domain.classify(&model, &bytes), Ok(true));
}

#[test]
fn event_contract_cannot_switch_endpoints_or_change_after_a_forecast() {
    let mut plain = fixture(false, 4096);
    assert_eq!(plain.require_stream_message_consistency(stream()), Err(Error::Binding));
    assert_eq!(plain.consistency_event_domain(), Some(ConsistencyEventDomain::PayloadPrefix));
    let mut broker = fixture(true, 4096);
    broker.require_stream_message_consistency(stream()).unwrap();
    assert_eq!(broker.require_stream_message_consistency(stream()), Err(Error::Duplicate));
    let mut late = fixture(true, 4096); forecast(&mut late, 1);
    assert_eq!(late.require_stream_message_consistency(stream()), Err(Error::WrongState));
    assert_eq!(late.pending_forecast(), Ok(Some(1)));
    assert_eq!(late.consistency_evidence().unwrap().samples(), 0);
}

#[test]
fn denied_messages_still_contribute_and_cross_the_original_lifetime_process() {
    let mut broker = fixture(true, 1); broker.require_stream_message_consistency(stream()).unwrap();
    for id in 1..=2 {
        forecast(&mut broker, id);
        let spec = broker.stream_message_spec("risk: denied", ElapsedTick(20)).unwrap();
        let result = broker.propose(id, spec, &snapshot()).unwrap();
        assert_eq!(result.state, ActionState::Denied);
        let observed = broker.consistency_observation(id).unwrap();
        assert!(observed.event()); assert_eq!(observed.event_domain(), ConsistencyEventDomain::StreamMessagePrefix(stream()));
        assert_eq!(observed.factor(), BinaryForecast::new(16384, 49152).unwrap().factor(true));
    }
    assert_eq!(broker.consistency_evidence().unwrap().first_crossing(), Some(2));
    assert_eq!(broker.pending_forecast(), Ok(None));
}

#[test]
fn downstream_refusal_preserves_classified_message_but_malformed_release_loses_coverage() {
    let mut broker = fixture(true, 4096); broker.require_stream_message_consistency(stream()).unwrap(); forecast(&mut broker, 1);
    let spec = broker.stream_message_spec("risk: observed", ElapsedTick(20)).unwrap();
    let mut missing = snapshot(); missing.complete = false;
    assert!(broker.propose(1, spec, &missing).is_err());
    assert!(broker.consistency_observation(1).unwrap().event());
    assert_eq!(broker.pending_forecast(), Ok(None));
    assert!(!broker.consistency_coverage_lost().unwrap());
    forecast(&mut broker, 2);
    let mut bad = broker.stream_message_spec("ordinary", ElapsedTick(20)).unwrap(); bad.payload.pop();
    assert!(broker.propose(2, bad, &snapshot()).is_err());
    assert_eq!(broker.consistency_evidence().unwrap().samples(), 1);
    assert_eq!(broker.pending_forecast(), Ok(Some(2))); assert!(broker.consistency_coverage_lost().unwrap());
    let valid = broker.stream_message_spec("ordinary", ElapsedTick(20)).unwrap();
    assert!(broker.propose(2, valid, &snapshot()).is_err());
    assert_eq!(broker.consistency_evidence().unwrap().samples(), 1);
}

#[test]
fn valid_negative_message_enters_original_review_without_gaining_permission() {
    let mut broker = fixture(true, 4096); broker.require_stream_message_consistency(stream()).unwrap(); forecast(&mut broker, 1);
    let spec = broker.stream_message_spec("ordinary", ElapsedTick(20)).unwrap();
    let result = broker.propose(1, spec, &snapshot()).unwrap();
    assert_eq!(result.state, ActionState::Reviewing);
    assert!(!broker.consistency_observation(1).unwrap().event());
    assert_eq!(broker.consistency_evidence().unwrap().samples(), 1);
    assert!(broker.authorize(1, None, &snapshot()).is_err());
}
