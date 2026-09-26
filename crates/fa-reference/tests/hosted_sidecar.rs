//! Actual owned inference, checked learned KV, full-input reviews and delivery.
//! Small weights and scripted verdicts are controls, not detector qualification.
use fa_reference::{Error, Snapshot};
use fa_reference::action::{ActionSpec, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::activation::monitor::{RefinementBudget, RefinementMonitor};
use fa_reference::action::consequence::activation::monitor::decoder::{MonitoredStep, MonitoringStatus,
    sampled::MonitoredSampledDecoder};
use fa_reference::action::consequence::activation::monitor::learned::model::LearnedAuditPreparationBudget;
use fa_reference::action::consequence::activation::probe::{LinearProbe,
    learned::{CheckedKvBudget, CheckedLearnedKv, KvGroup, KvRow, ResidualRetention}};
use fa_reference::action::consequence::activation::tensor::kv::{experiment::KvSide,
    decoder::{DecoderBudget, DecoderIdentity, DecoderLayerWeights, DecoderModel, DecoderProfile,
        DecoderShape, MAX_DECODER_PRODUCTS, sampling::{SamplingPolicy, SamplingStart}},
    model::learned::{CompressionBudget, FitBudget, LearnedKvCodec, LearnedKvPolicy}};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{EndpointOutcome, PublicationEndpoint};
use fa_reference::action::consequence::gate::{TargetCeiling,
    containment::{ActorState, RestartGrade, RestartProfile,
        session::policy::{Policy, Predicate, controller::ControllerConfig}}};
use fa_reference::action::consequence::oversight::{CommitteeContract, HelperContract, ObservedReview,
    OversightBroker, ReviewWindow, decoder_monitoring::DecoderBindingLimits,
    decoder_host::sidecar::{HostedSidecar, HostedSidecarRequest},
    sidecar::{SidecarCongressBudget, SidecarIdentity}};
use fa_reference::full_input::{InputProfileBinding, PartKind};
use fa_reference::round::Verdict;
use std::collections::BTreeMap;

fn model(generation: u64) -> DecoderModel {
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2,
        model_generation: generation, tokenizer_generation: 4, profile_generation: 5 },
        DecoderShape { vocabulary: 3, hidden: 2, intermediate: 2, layers: 2,
            query_heads: 1, cache_heads: 1, context: 16 }, 0.00001, 10000.0).unwrap();
    let layer = DecoderLayerWeights { attention_norm: vec![1.0; 2],
        queries: vec![1.0, 0.0, 0.0, 1.0], keys: vec![1.0, 0.0, 0.0, 1.0],
        values: vec![1.0, 0.0, 0.0, 1.0], attention_output: vec![0.25, 0.0, 0.0, 0.25],
        feed_forward_norm: vec![1.0; 2], gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4] };
    DecoderModel::new(profile, vec![1.0, 0.0, -1.0, 0.0, 0.0, 1.0], vec![layer.clone(), layer],
        vec![1.0; 2], vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0]).unwrap()
}
fn numerical_budget() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
fn codec(model: &DecoderModel, stream: u64) -> LearnedKvCodec {
    let training = model.recompute(stream, &[0, 1], numerical_budget()).unwrap().cache_image().unwrap();
    LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(101, training)]), FitBudget::default()).unwrap()
}
fn scope() -> Scope {
    Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect }
}
fn target() -> ResolvedTarget {
    ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 }
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn setup(threshold: f32, prefix: &[u32]) -> (OversightBroker, PublicationEndpoint, DecoderModel, LearnedKvCodec) {
    let model = model(3);
    let codec = codec(&model, 11);
    let allowance = RefinementBudget { encoded_bytes: 100_000, probe_coordinates: 100_000 };
    let mut monitors = BTreeMap::new();
    for layer in 1..=2 {
        let probe = LinearProbe::new(1, 1, model.residual_contract(layer).unwrap().profile(),
            &[0.0, 1.0], 0.0, threshold).unwrap();
        monitors.insert(layer, RefinementMonitor::new(vec![probe], vec![23], allowance).unwrap());
    }
    let numerical = MonitoredSampledDecoder::new(model.clone(), 21, 1, monitors, allowance,
        SamplingStart { policy: SamplingPolicy::new(1, 1, 3, 1.0, 1, 1.0).unwrap(), stream: 71, seed: 173 }).unwrap();
    let actor = ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
        model_generation: 3, tokenizer_generation: 4, state_schema_generation: 1,
        grade: RestartGrade::FunctionalRestart }, Vec::new(), vec![0], vec![0], 0).unwrap();
    let contracts = CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(),
        HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: Vec::new(),
            tokenizer_epoch: 0, policy_epoch: 0, model_epoch: 0 }, 1, b"approve?".to_vec()).unwrap())])).unwrap();
    let mut endpoint = PublicationEndpoint::new(target(), b"initial".to_vec(), 1000, 16).unwrap();
    let mut owner = OversightBroker::new(ControllerConfig { scope: scope(), total: 100,
        max_attempts: 16, actor, suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::PayloadAtMost(1024)]).unwrap(),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".to_owned(),
            MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
            caps: fa_reference::reducer::Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1,
            continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: TargetCeiling::new(&[target()]).unwrap() }, &mut endpoint, contracts).unwrap();
    owner.observe_time(ElapsedTick(1)).unwrap();
    endpoint.observe_time(ElapsedTick(1)).unwrap();
    owner.confirm_fence(endpoint.install_fence(owner.fence_request()).unwrap()).unwrap();
    owner.own_sampled_decoder(numerical, DecoderBindingLimits::default()).unwrap();
    for (position, token) in prefix.iter().copied().enumerate() {
        assert!(matches!(owner.advance_hosted_forced(owner.actor_revision(), position as u64,
            token, numerical_budget()).unwrap(), MonitoredStep::Released(_)));
    }
    (owner, endpoint, model, codec)
}
fn propose(owner: &mut OversightBroker, id: u64) -> FrozenAction {
    owner.propose(id, ActionSpec { version: VERSION, scope: scope(), target: Some(target()),
        payload: b"visible".to_vec(), required_witnesses: Vec::new(), policy_epoch: owner.inspect().ledger.epoch,
        deadline: ElapsedTick(100), units: 16 }, &snapshot()).unwrap().action
}
fn group() -> KvGroup { KvGroup { row: KvRow { layer: 1, side: KvSide::Value, position: 0 }, head: 0 } }
fn request(codec: &LearnedKvCodec) -> HostedSidecarRequest {
    HostedSidecarRequest { codec: codec.clone(), evaluation_origin: 201,
        retention: ResidualRetention::All, preparation: LearnedAuditPreparationBudget::default(),
        identity: SidecarIdentity { object_id: 77, generation: 1, transform_id: 9001 },
        priority: vec![group()], budget: SidecarCongressBudget::default() }
}
fn begin(owner: &mut OversightBroker, id: u64, codec: &LearnedKvCodec) -> HostedSidecar {
    owner.begin_hosted_sidecar(id, owner.actor_revision(), owner.input_revision(id).unwrap(), request(codec)).unwrap()
}
fn window() -> ReviewWindow { ReviewWindow { commit_by: ElapsedTick(5), reveal_by: ElapsedTick(10) } }
fn review(owner: &mut OversightBroker, id: u64, round: u64, verdict: Verdict) -> ObservedReview {
    let mut session = owner.begin_review(id, round, [9; 32], window(), &snapshot()).unwrap();
    let salt = b"original-reviewer";
    let commitment = session.commitment("reviewer", verdict, salt).unwrap();
    session.commit("reviewer", commitment, ElapsedTick(1)).unwrap();
    session.open_reveals(ElapsedTick(1)).unwrap();
    session.reveal("reviewer", verdict, salt, ElapsedTick(1)).unwrap();
    session.finish(ElapsedTick(1)).unwrap()
}

#[test]
fn actual_hosted_cache_builds_exact_coarse_input_and_original_review_can_publish() {
    let (mut owner, mut endpoint, model, codec) = setup(100.0, &[0, 1]);
    let action = propose(&mut owner, 1);
    let before = owner.hosted_decoder().unwrap();
    let sidecar = begin(&mut owner, 1, &codec);
    let actual = model.recompute(21, &[0, 1], numerical_budget()).unwrap().cache_image().unwrap();
    let (image, compression) = codec.evaluate_held_out(201, &actual, CompressionBudget::default()).unwrap();
    let expected = CheckedLearnedKv::new(image, &actual, ResidualRetention::All, CheckedKvBudget::default()).unwrap();
    assert_eq!(sidecar.source().encode().unwrap(), expected.encode().unwrap());
    assert_eq!(sidecar.compression(), &compression);
    assert_eq!(sidecar.source().report().source_values, 16);
    assert_eq!(sidecar.actor_revision(), before.actor_revision);
    assert_eq!(sidecar.input_revision(), 1);
    assert_eq!(owner.hosted_decoder().unwrap(), before);
    assert_eq!(sidecar.round().work().rounds, 1);
    assert!(sidecar.round().selected_groups().is_empty());
    let input = owner.current_hosted_sidecar(&sidecar).unwrap().clone();
    for view in input.views().values() {
        let actual = view.actual_input();
        let part = actual.ordered_parts().iter().position(|p| matches!(p.kind, PartKind::Evidence { .. })).unwrap();
        assert_eq!(actual.part_bytes(part).unwrap(), sidecar.round().payload());
    }
    assert!(owner.authorize(1, Some(&input), &snapshot()).is_err());
    assert_eq!(endpoint.execution_count(), 0);
    let completed = review(&mut owner, 1, 101, Verdict::Allow);
    owner.apply_review(completed, Some(&input), &snapshot()).unwrap();
    let permit = owner.authorize(1, Some(&input), &snapshot()).unwrap();
    let dispatch = owner.dispatch(&permit, &action, Some(&input), &snapshot()).unwrap();
    let receipt = endpoint.deliver(&dispatch).unwrap();
    assert_eq!(receipt.outcome(), EndpointOutcome::Executed { resulting_version: 2 });
    owner.accept_receipt(receipt).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(endpoint.payload(), b"visible");
}

#[test]
fn actual_source_advance_invalidates_both_sidecar_and_the_original_permit_path() {
    let (mut owner, mut endpoint, _, codec) = setup(100.0, &[0]);
    let action = propose(&mut owner, 1);
    let sidecar = begin(&mut owner, 1, &codec);
    let input = owner.current_hosted_sidecar(&sidecar).unwrap().clone();
    let completed = review(&mut owner, 1, 101, Verdict::Allow);
    owner.apply_review(completed, Some(&input), &snapshot()).unwrap();
    let permit = owner.authorize(1, Some(&input), &snapshot()).unwrap();
    owner.advance_hosted_forced(owner.actor_revision(), 1, 1, numerical_budget()).unwrap();
    assert_eq!(owner.current_hosted_sidecar(&sidecar), Err(Error::Stale));
    assert!(owner.begin_review(1, 102, [9; 32], window(), &snapshot()).is_err());
    assert!(owner.dispatch(&permit, &action, Some(&input), &snapshot()).is_err());
    assert_eq!(endpoint.execution_count(), 0);
    // A fresh proposal binds the new actual source and remains usable.
    propose(&mut owner, 2);
    let fresh = begin(&mut owner, 2, &codec);
    assert_eq!(fresh.source().report().source_values, 16);
    assert_eq!(sidecar.source().report().source_values, 8);
    let input = owner.current_hosted_sidecar(&fresh).unwrap().clone();
    let completed = review(&mut owner, 2, 103, Verdict::Allow);
    owner.apply_review(completed, Some(&input), &snapshot()).unwrap();
    let permit = owner.authorize(2, Some(&input), &snapshot()).unwrap();
    let dispatch = owner.dispatch(&permit, input.action(), Some(&input), &snapshot()).unwrap();
    owner.accept_receipt(endpoint.deliver(&dispatch).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
}

#[test]
fn same_looking_brokers_cannot_adopt_each_others_live_sidecar_handle() {
    let (mut first, _, _, codec) = setup(100.0, &[0, 1]);
    let (mut second, _, _, other_codec) = setup(100.0, &[0, 1]);
    propose(&mut first, 1); propose(&mut second, 1);
    let left = begin(&mut first, 1, &codec);
    let right = begin(&mut second, 1, &other_codec);
    assert_eq!(left.round(), right.round());
    assert!(first.current_hosted_sidecar(&left).is_ok());
    assert!(second.current_hosted_sidecar(&right).is_ok());
    assert_eq!(first.current_hosted_sidecar(&right), Err(Error::Binding));
    assert_eq!(second.current_hosted_sidecar(&left), Err(Error::Binding));
}

#[test]
fn source_preconditions_and_exact_limits_do_not_overwrite_existing_inputs_on_failure() {
    let (mut owner, endpoint, model, codec) = setup(100.0, &[0, 1]);
    propose(&mut owner, 1);
    let sidecar = begin(&mut owner, 1, &codec);
    let before = owner.hosted_decoder().unwrap();
    let bytes = owner.captured_input_bytes();
    let mut exact = request(&codec);
    exact.preparation.compression.source_values = 16;
    exact.preparation.source_check.source_values = 16;
    let idempotent = owner.begin_hosted_sidecar(1, owner.actor_revision(), 1, exact.clone()).unwrap();
    assert_eq!(idempotent.input_revision(), 1);
    assert_eq!(owner.captured_input_bytes(), bytes);
    let mut cases = Vec::new();
    let mut short = exact.clone(); short.preparation.source_check.source_values = 15;
    cases.push((short, Error::Limit));
    let mut short = exact; short.preparation.compression.source_values = 15;
    cases.push((short, Error::Limit));
    let mut overlap = request(&codec); overlap.evaluation_origin = 101;
    cases.push((overlap, Error::Duplicate));
    let mut training = request(&codec); training.codec = self::codec(&model, 21);
    cases.push((training, Error::Duplicate));
    let mut foreign = request(&codec); foreign.codec = self::codec(&self::model(99), 11);
    cases.push((foreign, Error::Binding));
    let mut unavailable = request(&codec); unavailable.retention = ResidualRetention::None;
    cases.push((unavailable, Error::Missing));
    for (request, expected) in cases {
        assert_eq!(owner.begin_hosted_sidecar(1, owner.actor_revision(), 1, request).err(), Some(expected));
        assert_eq!(owner.hosted_decoder().unwrap(), before);
        assert_eq!(owner.captured_input_bytes(), bytes);
        assert_eq!(owner.current_hosted_sidecar(&sidecar).unwrap(), sidecar.round().input());
    }
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn stale_input_versions_and_policy_epochs_cannot_relabel_retained_sidecar_as_current() {
    let (mut owner, _, _, codec) = setup(100.0, &[0]);
    propose(&mut owner, 1);
    let old = begin(&mut owner, 1, &codec);
    let mut different = request(&codec); different.identity.transform_id += 1;
    let new = owner.begin_hosted_sidecar(1, owner.actor_revision(), 1, different).unwrap();
    assert_eq!(new.input_revision(), 2);
    assert_eq!(owner.current_hosted_sidecar(&old), Err(Error::Stale));
    assert!(owner.current_hosted_sidecar(&new).is_ok());
    let before = owner.captured_input_bytes();
    assert_eq!(owner.begin_hosted_sidecar(1, owner.actor_revision(), 1, request(&codec)).err(), Some(Error::Stale));
    assert_eq!(owner.captured_input_bytes(), before);
    owner.revoke_epoch().unwrap();
    assert_eq!(owner.current_hosted_sidecar(&new), Err(Error::Stale));
}

#[test]
fn a_held_numerical_owner_cannot_export_its_earlier_quiet_prefix_as_fresh_evidence() {
    let (mut owner, endpoint, _, codec) = setup(0.5, &[0]);
    propose(&mut owner, 1);
    let sidecar = begin(&mut owner, 1, &codec);
    assert!(owner.current_hosted_sidecar(&sidecar).is_ok());
    assert!(matches!(owner.advance_hosted_forced(owner.actor_revision(), 1, 2, numerical_budget()).unwrap(), MonitoredStep::Held(_)));
    let before = owner.hosted_decoder().unwrap();
    assert_eq!(before.status, MonitoringStatus::Held);
    assert!(owner.current_hosted_sidecar(&sidecar).is_err());
    assert!(owner.begin_hosted_sidecar(1, owner.actor_revision(), 1, request(&codec)).is_err());
    assert_eq!(owner.hosted_decoder().unwrap(), before);
    assert_eq!(sidecar.source().report().source_values, 8);
    assert_eq!(endpoint.execution_count(), 0);
}
