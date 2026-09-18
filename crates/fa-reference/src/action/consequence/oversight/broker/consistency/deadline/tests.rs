//! A real pending prediction is the timer basis; silence is never a negative vote.
use super::*;
use crate::action::{ActionSpec, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
use crate::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastModel, ForecastRegistration};
use crate::action::consequence::activation::probe::LinearProbe;
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::{EndpointOutcome, PublicationEndpoint, StopRequest};
use crate::action::consequence::gate::TargetCeiling;
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, ReviewWindow, action_frame};
use crate::action::consequence::oversight::consistency::{ConsistencyConfig, ConsistencyStopCause, ConsistencyStopPolicy};
use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use crate::reducer::Caps;
use crate::round::Verdict;
use crate::Snapshot;
use std::collections::BTreeMap;

fn spec(epoch: u64) -> ActionSpec {
    ActionSpec { version: VERSION,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 }),
        payload: b"safe".to_vec(), required_witnesses: vec![], policy_epoch: epoch,
        deadline: ElapsedTick(100), units: 16 }
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn profile() -> CaptureProfile {
    CaptureProfile { tenant: 1, model: 2, model_generation: 1, tap: 4, layout_generation: 5 }
}
fn forecast(owner: &mut OversightBroker, id: u64) -> ConsistencyDeadline {
    let source = SourceFrame::capture(FrameIdentity { profile: profile(), stream: 7, sequence: id, position: 0 }, &[-1.0]).unwrap();
    owner.forecast_action(id, owner.actor_revision(), &source).unwrap();
    owner.consistency_deadline().unwrap().unwrap()
}
fn fixture(stopping: bool) -> (OversightBroker, PublicationEndpoint) {
    let contracts = CommitteeContract::new(BTreeMap::from([("helper".to_owned(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"deadline-control".to_vec(), model_epoch: 1, tokenizer_epoch: 1, policy_epoch: 0 },
        9, b"Review".to_vec()).unwrap())])).unwrap();
    let target = spec(0).target.unwrap();
    let config = ControllerConfig {
        scope: spec(0).scope, total: 100, max_attempts: 16,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1,
            tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart },
            vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::UnitsAtMost(16)]).unwrap(),
        congress: CongressPolicy {
            generation: 1, members: BTreeMap::from([("helper".to_owned(), MemberPolicy { cohort: "a".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1,
        }, narrowed_targets: TargetCeiling::new(&[target]).unwrap(),
    };
    let mut endpoint = PublicationEndpoint::new(target, b"old".to_vec(), 200, 16).unwrap();
    let mut owner = OversightBroker::new(config, &mut endpoint, contracts).unwrap();
    owner.observe_time(ElapsedTick(1)).unwrap(); endpoint.observe_time(ElapsedTick(1)).unwrap();
    owner.confirm_fence(endpoint.install_fence(owner.fence_request()).unwrap()).unwrap();
    let pair = BinaryForecast::new(32768, 32768).unwrap();
    owner.enable_action_consistency(ConsistencyConfig {
        model: ForecastModel::new(LinearProbe::new(1, 1, profile(), &[1.0], 0.0, 0.0).unwrap(), ForecastRegistration {
            domain: 11, generation: 1, policy_generation: 1, event_prefix: b"risk".to_vec(),
            negative: pair, at_threshold: pair, positive: pair,
        }).unwrap(), alpha: ErrorBudget::new(1, 4).unwrap(), stream: 7, max_predictions: 8, max_prediction_age_ticks: 8,
    }).unwrap();
    if stopping { owner.enable_consistency_stop(ConsistencyStopPolicy::new(11, 1, 7007).unwrap()).unwrap(); }
    (owner, endpoint)
}
fn review(owner: &mut OversightBroker, action: &FrozenAction, id: u64) -> CommitteeInput {
    let helper = &owner.contracts().members()["helper"];
    let mut bytes = action_frame(action); let end = bytes.len(); bytes.extend_from_slice(helper.question());
    let input = ActualHelperInput::new(bytes.clone(), helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { span: ByteSpan { start: 0, end }, kind: PartKind::Other },
        SubmittedPart { span: ByteSpan { start: end, end: bytes.len() }, kind: PartKind::Question },
    ], vec![]).unwrap();
    let view = EvidenceViewManifest::new(input, AuthorizationProjection {
        projection_id: 9, policy_epoch: action.spec().policy_epoch, projected_originals: vec![],
    }, vec![]).unwrap();
    let input = CommitteeInput::capture(action, owner.contracts(), BTreeMap::from([("helper".to_owned(), view)])).unwrap();
    owner.record_inputs(id, 0, input.clone()).unwrap();
    assert!(owner.authorize(id, Some(&input), &snapshot()).is_err());
    let now = owner.inspect().ledger.elapsed.unwrap();
    let mut session = owner.begin_review(id, 100 + id, [1; 32], ReviewWindow {
        commit_by: ElapsedTick(now.0 + 5), reveal_by: ElapsedTick(now.0 + 10),
    }, &snapshot()).unwrap();
    let digest = session.commitment("helper", Verdict::Allow, b"salt").unwrap();
    session.commit("helper", digest, now).unwrap(); session.open_reveals(now).unwrap();
    session.reveal("helper", Verdict::Allow, b"salt", now).unwrap();
    owner.apply_review(session.finish(now).unwrap(), Some(&input), &snapshot()).unwrap(); input
}

#[test]
fn early_timer_preserves_a_proposal_that_still_requires_original_review() {
    let (mut owner, mut endpoint) = fixture(true);
    assert_eq!(owner.consistency_deadline().unwrap(), None);
    let deadline = forecast(&mut owner, 1);
    assert_eq!(deadline.created_at, ElapsedTick(1)); assert_eq!(deadline.expires_at, ElapsedTick(9));
    owner.observe_time(ElapsedTick(8)).unwrap();
    let before = owner.inspect(); let evidence = owner.consistency_evidence().unwrap().clone();
    assert_eq!(owner.expire_consistency_deadline(deadline), Ok(false));
    assert_eq!(owner.inspect(), before); assert_eq!(owner.consistency_evidence().unwrap(), &evidence);
    let action = owner.propose(1, spec(0), &snapshot()).unwrap().action;
    let input = review(&mut owner, &action, 1);
    let key = owner.authorize(1, Some(&input), &snapshot()).unwrap();
    let message = owner.dispatch(&key, &action, Some(&input), &snapshot()).unwrap();
    endpoint.observe_time(ElapsedTick(8)).unwrap();
    owner.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.payload(), b"safe"); assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(owner.consistency_evidence().unwrap().samples(), 1);
    owner.observe_time(ElapsedTick(9)).unwrap();
    assert_eq!(owner.expire_consistency_deadline(deadline), Err(Error::Missing));
    assert!(!owner.consistency_coverage_lost().unwrap()); assert!(owner.stop_receipt().is_none());
}

#[test]
fn exact_expiry_preserves_unknown_forecast_and_dispatched_accounting_until_native_drain() {
    for executed in [false, true] {
        let (mut owner, mut endpoint) = fixture(true);
        forecast(&mut owner, 1);
        let action = owner.propose(1, spec(0), &snapshot()).unwrap().action;
        let input = review(&mut owner, &action, 1);
        let key = owner.authorize(1, Some(&input), &snapshot()).unwrap();
        let message = owner.dispatch(&key, &action, Some(&input), &snapshot()).unwrap();
        if executed { assert!(matches!(endpoint.deliver(&message).unwrap().outcome(), EndpointOutcome::Executed { .. })); }
        let deadline = forecast(&mut owner, 2);
        let evidence = owner.consistency_evidence().unwrap().clone();
        owner.observe_time(deadline.expires_at).unwrap();
        assert_eq!(owner.expire_consistency_deadline(deadline), Ok(true));
        assert_eq!(owner.consistency_evidence().unwrap(), &evidence);
        assert_eq!(owner.pending_forecast().unwrap(), Some(2));
        assert_eq!(owner.consistency_deadline().unwrap(), Some(deadline));
        let incident = owner.consistency_stop_incident().unwrap().clone();
        assert_eq!(incident.cause, ConsistencyStopCause::CoverageLost);
        assert_eq!(incident.prediction_jobs, 2); assert_eq!(incident.observed_samples, 1);
        assert_eq!(owner.inspect().ledger.charged, 16);
        let before = owner.inspect();
        assert_eq!(owner.expire_consistency_deadline(deadline), Ok(true));
        assert_eq!(owner.inspect(), before); assert_eq!(owner.consistency_stop_incident(), Some(&incident));
        endpoint.observe_time(deadline.expires_at).unwrap();
        let drained = owner.progress_stop(&mut endpoint).unwrap(); assert!(drained.progress.drained());
        assert_eq!(owner.inspect().ledger.charged, if executed { 16 } else { 0 });
        assert_eq!(endpoint.execution_count(), u64::from(executed));
    }
}

#[test]
fn changed_or_obsolete_timer_cannot_expire_a_different_forecast() {
    let (mut owner, _) = fixture(true);
    let first = forecast(&mut owner, 1);
    owner.propose(1, spec(0), &snapshot()).unwrap();
    let next = forecast(&mut owner, 2);
    owner.observe_time(next.expires_at).unwrap();
    let before = owner.inspect();
    assert_eq!(owner.expire_consistency_deadline(first), Err(Error::Stale));
    for field in 0..6 {
        let mut bad = next;
        match field { 0 => bad.attempt += 1, 1 => bad.actor_revision += 1,
            2 => bad.authority_epoch += 1, 3 => bad.source_sequence += 1,
            4 => bad.created_at.0 += 1, _ => bad.expires_at.0 += 1 }
        assert_eq!(owner.expire_consistency_deadline(bad), Err(Error::Stale));
    }
    assert_eq!(owner.check_consistency_deadline(next, ElapsedTick(8)), Err(Error::Stale));
    assert_eq!(owner.inspect(), before); assert!(!owner.consistency_coverage_lost().unwrap());
    assert_eq!(owner.expire_consistency_deadline(next), Ok(true));
}

#[test]
fn expiry_retains_legacy_hold_and_preserves_an_existing_other_stop_identity() {
    for already_stopped in [false, true] {
        let (mut owner, _) = fixture(already_stopped);
        let deadline = forecast(&mut owner, 1);
        let existing = if already_stopped {
            let c = owner.inspect();
            Some(owner.request_stop(StopRequest { operation: 9009,
                expected_control_sequence: c.sequence, expected_authority_epoch: c.ledger.epoch }).unwrap())
        } else { None };
        owner.observe_time(deadline.expires_at).unwrap();
        owner.expire_consistency_deadline(deadline).unwrap();
        assert!(owner.consistency_coverage_lost().unwrap());
        assert_eq!(owner.consistency_evidence().unwrap().samples(), 0);
        assert_eq!(owner.pending_forecast().unwrap(), Some(1));
        assert_eq!(owner.stop_receipt(), existing.as_ref());
        assert!(owner.forecast_action(2, owner.actor_revision(), &SourceFrame::capture(
            FrameIdentity { profile: profile(), stream: 7, sequence: 2, position: 0 }, &[-1.0]).unwrap()).is_err());
    }
}
