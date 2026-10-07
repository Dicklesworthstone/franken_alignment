//! Real original generation, source checking, forecasting, congress and two keys.
//! Synthetic probabilities/weights are causal controls, not calibration evidence.
use super::*;
use crate::{Snapshot, Error};
use crate::action::{ActionSpec, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget,
    ForecastModel, ForecastRegistration};
use crate::action::consequence::activation::probe::LinearProbe;
use crate::action::consequence::activation::monitor::{MonitorOutcome, learned::LearnedMonitorWork};
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderModel,
    sampling::{SamplingPolicy, SamplingStart, monitored::{GenerationBudget, GenerationSpec,
        GenerationTelemetryBudget, GenerationStatus, LearnedGeneration}}};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::PublicationEndpoint;
use crate::action::consequence::gate::TargetCeiling;
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract,
    ReviewWindow, action_frame, consistency::ConsistencyConfig,
    decoder_monitoring::LearnedDecoderBindingLimits, human::{HumanReviewer, HumanReviewPolicy},
    learned_source::LearnedSourceConfig};
use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use crate::reducer::Caps;
use crate::round::Verdict;
use std::collections::{BTreeMap, BTreeSet};
#[path = "../../../learned_host/tests/model.rs"]
mod numerical;
use numerical::{model, policy};

fn source_config(model: &DecoderModel, alarm: bool) -> LearnedSourceConfig {
    LearnedSourceConfig { stream: 21, evaluation_origin: 201, monitor_generation: 31,
        spec: GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
            policy: SamplingPolicy::new(1, 1, 3, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173,
        }).unwrap(), policy: policy(model, alarm), budget: GenerationBudget::default(),
        telemetry: GenerationTelemetryBudget::default() }
}
fn original(model: &DecoderModel, source: LearnedSourceConfig) -> LearnedGeneration {
    model.monitored_generation_with_telemetry(source.stream, source.evaluation_origin,
        source.spec, source.policy, source.budget, source.telemetry).unwrap()
}
fn config(model: &DecoderModel, side: KvSide, weights: &[f32]) -> LearnedConsistencyConfig {
    let layer = &model.cache_profile().layers()[&1];
    let tensor = match side { KvSide::Key => layer.keys(), KvSide::Value => layer.values() };
    let neutral = BinaryForecast::new(32_768, 32_768).unwrap();
    LearnedConsistencyConfig { consistency: ConsistencyConfig {
        model: ForecastModel::new(LinearProbe::new(91, 1, tensor.profile(), weights, 0.0, 0.0).unwrap(),
            ForecastRegistration { domain: 71, generation: 2, policy_generation: 1,
                event_prefix: b"risk".to_vec(), negative: neutral, at_threshold: neutral, positive: neutral }).unwrap(),
        alpha: ErrorBudget::new(1, 4).unwrap(), stream: 21, max_predictions: 8, max_prediction_age_ticks: 8,
    }, layer: 1, side, budget: LearnedMonitorBudget::default() }
}
fn scope() -> Scope { Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect } }
fn target() -> ResolvedTarget {
    ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 }
}
fn owner() -> (OversightBroker, PublicationEndpoint, HumanReviewer) {
    let contract = CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(),
        HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: Vec::new(),
            tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 1, b"approve?".to_vec()).unwrap(),
    )])).unwrap();
    let mut endpoint = PublicationEndpoint::new(target(), b"initial".to_vec(), 1000, 8).unwrap();
    let mut owner = OversightBroker::new(ControllerConfig {
        scope: scope(), total: 100, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
            model_generation: 3, tokenizer_generation: 4, state_schema_generation: 1,
            grade: RestartGrade::FunctionalRestart }, vec![], vec![0], vec![0], 0).unwrap(),
        suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::PayloadAtMost(1024)]).unwrap(),
        congress: CongressPolicy { generation: 1,
            members: BTreeMap::from([("reviewer".to_owned(), MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1,
            continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1,
        }, narrowed_targets: TargetCeiling::new(&[target()]).unwrap(),
    }, &mut endpoint, contract).unwrap();
    let reviewer = owner.enable_human_review(HumanReviewPolicy {
        reviewer_id: 55, max_validity_ticks: 100, max_requests: 8,
    }).unwrap();
    owner.observe_time(ElapsedTick(1)).unwrap(); endpoint.observe_time(ElapsedTick(1)).unwrap();
    owner.confirm_fence(endpoint.install_fence(owner.fence_request()).unwrap()).unwrap();
    (owner, endpoint, reviewer)
}
fn attach(owner: &mut OversightBroker, model: &DecoderModel, config: LearnedSourceConfig) {
    owner.own_learned_generation(model.clone(), config, LearnedDecoderBindingLimits::default()).unwrap();
}
fn step(owner: &mut OversightBroker) -> Result<(), Error> {
    let state = owner.hosted_learned_generation()?;
    owner.advance_hosted_learned(state.actor_revision, state.position).map(|_| ())
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn spec(owner: &OversightBroker) -> ActionSpec {
    ActionSpec { version: VERSION, scope: scope(), target: Some(target()), payload: b"visible".to_vec(),
        required_witnesses: vec![], policy_epoch: owner.inspect().ledger.epoch,
        deadline: ElapsedTick(100), units: 16 }
}
fn review(owner: &mut OversightBroker, action: &FrozenAction) -> CommitteeInput {
    let helper = &owner.contracts().members()["reviewer"];
    let mut bytes = action_frame(action); let boundary = bytes.len();
    bytes.extend_from_slice(helper.question()); let end = bytes.len();
    let input = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
    ], vec![]).unwrap();
    let view = EvidenceViewManifest::new(input, AuthorizationProjection {
        projection_id: helper.projection_id(), policy_epoch: action.spec().policy_epoch, projected_originals: vec![],
    }, vec![]).unwrap();
    let inputs = CommitteeInput::capture(action, owner.contracts(), BTreeMap::from([("reviewer".to_owned(), view)])).unwrap();
    owner.record_inputs(1, owner.input_revision(1).unwrap(), inputs.clone()).unwrap();
    assert!(owner.authorize(1, Some(&inputs), &snapshot()).is_err());
    let mut session = owner.begin_review(1, 101, [7; 32], ReviewWindow {
        commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30),
    }, &snapshot()).unwrap();
    let salt = [11; 32];
    let digest = session.commitment("reviewer", Verdict::Allow, &salt).unwrap();
    session.commit("reviewer", digest, ElapsedTick(1)).unwrap(); session.open_reveals(ElapsedTick(1)).unwrap();
    session.reveal("reviewer", Verdict::Allow, &salt, ElapsedTick(1)).unwrap();
    let receipt = session.finish(ElapsedTick(1)).unwrap();
    owner.apply_review(receipt, Some(&inputs), &snapshot()).unwrap(); inputs
}

#[test]
fn real_accepted_audit_matches_original_predictor_without_advancing_generator() {
    for side in [KvSide::Key, KvSide::Value] {
        let model = model(); let source = source_config(&model, false);
        let mut control = original(&model, source.clone());
        let (mut owner, endpoint, _) = owner(); attach(&mut owner, &model, source);
        let direct = config(&model, side, &[0.0, 1.0]).consistency.model;
        owner.enable_owned_learned_action_consistency(config(&model, side, &[0.0, 1.0])).unwrap();
        let mut refinements = 0;
        for attempt in 1..=2 {
            control.advance(control.position()).unwrap(); step(&mut owner).unwrap();
            let before = owner.hosted_learned_generation().unwrap();
            let run = owner.hosted_learned_original().unwrap();
            let cache = run.accepted_cache_image().unwrap();
            let row = KvRow { layer: 1, side, position: before.position - 1 };
            let token = cache.layer(1).unwrap().token(row.position).unwrap();
            let raw = match side { KvSide::Key => token.key(), KvSide::Value => token.value() };
            let expected = direct.predict(raw).unwrap();
            let audit = run.last_event().unwrap().audit().source().clone();
            let original_report = direct.clone().into_learned(LearnedMonitorBudget::default()).unwrap()
                .predict(&audit, row).unwrap();
            let got = owner.forecast_owned_learned_action(attempt, before.actor_revision).unwrap();
            assert_eq!(got.prediction().unwrap().forecast(), expected.forecast());
            assert_eq!(got.prediction().unwrap().observation().frame(), raw.identity());
            assert_eq!(got.work(), original_report.work()); refinements += got.work().refinements;
            assert_eq!(owner.hosted_learned_generation().unwrap(), before);
            let run = owner.hosted_learned_original().unwrap();
            assert_eq!(run.accepted_cache_image().unwrap().encode().unwrap(), control.accepted_cache_image().unwrap().encode().unwrap());
            assert_eq!(run.sampler_state(), control.sampler_state()); assert_eq!(run.work(), control.work());
            assert_eq!(run.telemetry_work(), control.telemetry_work());
            owner.propose(attempt, spec(&owner), &snapshot()).unwrap();
            assert_eq!(owner.consistency_evidence().unwrap().samples(), attempt as usize);
        }
        assert!(refinements > 0, "actual learned uncertainty must exercise the residual path");
        assert_eq!(endpoint.execution_count(), 0);
    }
}

#[test]
fn quiet_prediction_still_requires_congress_and_independent_human_key() {
    let model = model(); let (mut owner, mut endpoint, human) = owner();
    attach(&mut owner, &model, source_config(&model, false));
    owner.enable_owned_learned_action_consistency(config(&model, KvSide::Key, &[0.0, 0.0])).unwrap();
    step(&mut owner).unwrap();
    assert!(owner.propose(1, spec(&owner), &snapshot()).is_err());
    owner.forecast_owned_learned_action(1, owner.actor_revision()).unwrap().prediction().unwrap();
    let action = owner.propose(1, spec(&owner), &snapshot()).unwrap().action;
    let input = review(&mut owner, &action);
    let permit = owner.authorize(1, Some(&input), &snapshot()).unwrap();
    assert!(owner.dispatch(&permit, &action, Some(&input), &snapshot()).is_err());
    let request = owner.request_human_approval(10, 1, Some(&input), ElapsedTick(80)).unwrap();
    let human = human.approve(&request, ElapsedTick(1)).unwrap();
    let message = owner.dispatch_with_human(&permit, &human, &action, Some(&input), &snapshot()).unwrap();
    owner.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.payload(), b"visible"); assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(owner.inspect().ledger.charged, 16);
    assert!(owner.dispatch_with_human(&permit, &human, &action, Some(&input), &snapshot()).is_err());
}

#[test]
fn source_selection_and_shape_are_atomic_and_cannot_be_installed_after_work() {
    let model = model();
    for invalid in 0..5 {
        let (mut owner, _, _) = owner(); attach(&mut owner, &model, source_config(&model, false));
        let mut bad = config(&model, KvSide::Key, if invalid == 4 { &[0.0] } else { &[0.0, 0.0] });
        match invalid { 0 => bad.layer = 0, 1 => bad.layer = 3, 2 => bad.side = KvSide::Value,
            3 => bad.consistency.stream += 1, _ => {} }
        let before = owner.hosted_learned_generation().unwrap();
        assert!(owner.enable_owned_learned_action_consistency(bad).is_err());
        assert!(!owner.action_consistency_required()); assert_eq!(owner.hosted_learned_generation().unwrap(), before);
        owner.enable_owned_learned_action_consistency(config(&model, KvSide::Key, &[0.0, 0.0])).unwrap();
        assert_eq!(owner.enable_learned_action_consistency(config(&model, KvSide::Key, &[0.0, 0.0])), Err(Error::Duplicate));
        assert!(owner.owned_learned_consistency_required());
    }
    let (mut owner, _, _) = owner(); attach(&mut owner, &model, source_config(&model, false)); step(&mut owner).unwrap();
    assert_eq!(owner.enable_owned_learned_action_consistency(config(&model, KvSide::Key, &[0.0, 0.0])), Err(Error::WrongState));
    assert!(!owner.action_consistency_required());
}

#[test]
fn supplied_capture_and_stale_caller_cannot_replace_the_owned_source() {
    let model = model(); let (mut owner, _, _) = owner();
    attach(&mut owner, &model, source_config(&model, false));
    owner.enable_owned_learned_action_consistency(config(&model, KvSide::Key, &[0.0, 0.0])).unwrap();
    assert!(owner.forecast_owned_learned_action(1, owner.actor_revision()).is_err()); // no accepted frame yet
    step(&mut owner).unwrap();
    let revision = owner.actor_revision();
    let source = owner.hosted_learned_original().unwrap().last_event().unwrap().audit().source().clone();
    let row = KvRow { layer: 1, side: KvSide::Key, position: 0 };
    assert!(matches!(owner.forecast_learned_action(1, revision, &source, row), Err(Error::Binding)));
    assert!(matches!(owner.forecast_owned_learned_action(1, revision - 1), Err(Error::Stale)));
    assert_eq!(owner.consistency.as_ref().unwrap().jobs, 0);
    owner.forecast_owned_learned_action(1, revision).unwrap().prediction().unwrap();
    assert!(owner.forecast_owned_learned_action(2, revision).is_err());
    assert_eq!(owner.pending_forecast().unwrap(), Some(1));
    assert_eq!(owner.consistency.as_ref().unwrap().jobs, 1);
}

#[test]
fn held_and_failed_hosts_never_fall_back_to_an_earlier_quiet_audit() {
    let model = model();
    for failure in 0..2 {
        let mut cfg = source_config(&model, failure == 0);
        if failure == 1 { cfg.telemetry.source_check_values = model.cache_profile().values_per_token() as u64; }
        let (mut owner, endpoint, _) = owner(); attach(&mut owner, &model, cfg);
        owner.enable_owned_learned_action_consistency(config(&model, KvSide::Key, &[0.0, 0.0])).unwrap();
        step(&mut owner).unwrap();
        owner.forecast_owned_learned_action(1, owner.actor_revision()).unwrap().prediction().unwrap();
        owner.propose(1, spec(&owner), &snapshot()).unwrap();
        let old_work = owner.learned_consistency_work().unwrap();
        {
            let result = step(&mut owner);
            if failure == 0 { result.unwrap(); assert!(matches!(owner.hosted_learned_generation().unwrap().status, GenerationStatus::Held(_))); }
            else { assert_eq!(result, Err(Error::Limit)); }
        }
        assert!(owner.forecast_owned_learned_action(2, owner.actor_revision()).is_err());
        assert_eq!(owner.learned_consistency_work().unwrap(), old_work);
        assert_eq!(owner.consistency_evidence().unwrap().samples(), 1);
        assert_eq!(owner.pending_forecast().unwrap(), None);
        assert!(owner.propose(2, spec(&owner), &snapshot()).is_err());
        assert_eq!(endpoint.execution_count(), 0);
    }
}

#[test]
fn owned_source_does_not_get_free_forecast_work_or_inventory_across_tokens() {
    let model = model(); let mut control = original(&model, source_config(&model, false));
    control.advance(0).unwrap();
    let audit = control.last_event().unwrap().audit().source();
    let direct = config(&model, KvSide::Key, &[0.0, 0.0]).consistency.model
        .into_learned(LearnedMonitorBudget::default()).unwrap();
    let cost = direct.predict(audit, KvRow { layer: 1, side: KvSide::Key, position: 0 }).unwrap().work();
    for limit_storage in [false, true] {
        let (mut owner, endpoint, _) = owner(); attach(&mut owner, &model, source_config(&model, false));
        let mut lifetime = LearnedMonitorBudget::default();
        if !limit_storage { lifetime.encoded_bytes = cost.encoded_bytes; }
        let inventory = if limit_storage { audit.report().total_encoded_bytes } else { MAX_CHECKED_KV_BYTES };
        owner.enable_owned_learned_action_consistency_with_limits(config(&model, KvSide::Key, &[0.0, 0.0]), lifetime, inventory).unwrap();
        step(&mut owner).unwrap();
        owner.forecast_owned_learned_action(1, owner.actor_revision()).unwrap().prediction().unwrap();
        owner.propose(1, spec(&owner), &snapshot()).unwrap(); step(&mut owner).unwrap();
        let result = owner.forecast_owned_learned_action(2, owner.actor_revision());
        if limit_storage { assert!(matches!(result, Err(Error::Limit))); }
        else {
            let report = result.unwrap(); assert_eq!(report.prediction().unwrap_err(), Error::Limit);
            assert_eq!(report.monitor().outcome(), MonitorOutcome::BudgetExhausted);
            assert_eq!(report.work(), LearnedMonitorWork::default());
        }
        assert_eq!(owner.learned_consistency_work().unwrap(), cost);
        assert!(owner.consistency_coverage_lost().unwrap()); assert_eq!(owner.pending_forecast().unwrap(), None);
        assert_eq!(owner.consistency_evidence().unwrap().samples(), 1); assert_eq!(endpoint.execution_count(), 0);
    }
}
