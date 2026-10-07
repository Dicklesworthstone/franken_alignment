//! Original decoder/codec, original broker, original congress and real endpoint.
//! Synthetic weights isolate lost coordinates; they are not detector training.
use super::*;
use crate::action::{ActionSpec, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::activation::{SourceFrame,
    consistency::{BinaryForecast, ErrorBudget, ForecastModel, ForecastRegistration},
    probe::LinearProbe,
    tensor::kv::{decoder::{DecoderBudget, DecoderIdentity, DecoderLayerWeights,
        DecoderModel, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS},
        model::{ModelKvImage, learned::{CompressionBudget, FitBudget, LearnedKvCodec, LearnedKvPolicy}}}};
use crate::action::consequence::activation::probe::learned::CheckedKvBudget;
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::PublicationEndpoint;
use crate::action::consequence::gate::TargetCeiling;
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, ReviewWindow, action_frame};
use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use crate::reducer::Caps;
use crate::round::Verdict;
use crate::Snapshot;
use std::collections::BTreeMap;

pub(super) fn row() -> KvRow { KvRow { layer: 1, side: KvSide::Key, position: 0 } }

pub(super) fn source(value: [f32; 2], retention: ResidualRetention, tokens: usize, stream: u64)
    -> (CheckedLearnedKv, ModelKvImage)
{
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2,
        model_generation: 3, tokenizer_generation: 4, profile_generation: 5 },
        DecoderShape { vocabulary: 4, hidden: 2, intermediate: 2, layers: 1,
            query_heads: 1, cache_heads: 1, context: 8 }, 0.00001, 10000.0).unwrap();
    let model = DecoderModel::new(profile,
        vec![1.0, 0.0, -1.0, 0.0, value[0], value[1], 0.0, 1.0],
        vec![DecoderLayerWeights {
            attention_norm: vec![1.0; 2], queries: vec![0.0; 4],
            keys: vec![1.0, 0.0, 0.0, 1.0], values: vec![1.0, 0.0, 0.0, 1.0],
            attention_output: vec![0.0; 4], feed_forward_norm: vec![1.0; 2],
            gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4],
        }], vec![1.0; 2], vec![0.0; 8]).unwrap();
    let inference = DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS };
    let positive = model.recompute(11, &[0], inference).unwrap().cache_image().unwrap();
    let negative = model.recompute(12, &[1], inference).unwrap().cache_image().unwrap();
    let codec = LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(101, positive), (102, negative)]), FitBudget::default()).unwrap();
    let original = model.recompute(stream, &vec![2; tokens], inference).unwrap().cache_image().unwrap();
    let (image, report) = codec.evaluate_held_out(202, &original, CompressionBudget::default()).unwrap();
    assert!(!report.training_source_overlap);
    let checked = CheckedLearnedKv::new(image, &original, retention, CheckedKvBudget::default()).unwrap();
    (checked, original)
}

pub(super) fn raw(source: &ModelKvImage) -> &SourceFrame {
    source.layer(row().layer).unwrap().token(row().position).unwrap().key()
}

pub(super) fn config(source: &CheckedLearnedKv, weights: &[f32], alpha: ErrorBudget) -> ConsistencyConfig {
    let identity = source.row_shape(row()).unwrap().0;
    ConsistencyConfig {
        model: ForecastModel::new(LinearProbe::new(91, 1, identity.profile, weights, 0.0, 0.0).unwrap(), ForecastRegistration {
            domain: 71, generation: 2, policy_generation: 1, event_prefix: b"risk".to_vec(),
            negative: BinaryForecast::new(49_152, 16_384).unwrap(),
            at_threshold: BinaryForecast::new(32_768, 32_768).unwrap(),
            positive: BinaryForecast::new(16_384, 49_152).unwrap(),
        }).unwrap(), alpha, stream: identity.stream, max_predictions: 8, max_prediction_age_ticks: 8,
    }
}

pub(super) fn spec(epoch: u64, payload: &[u8]) -> ActionSpec {
    ActionSpec { version: VERSION,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 }),
        payload: payload.to_vec(), required_witnesses: vec![], policy_epoch: epoch, deadline: ElapsedTick(100), units: 16 }
}
pub(super) fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }

pub(super) fn owner() -> (OversightBroker, PublicationEndpoint) {
    let contracts = CommitteeContract::new(BTreeMap::from([("helper".to_owned(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"profile".to_vec(), model_epoch: 1, tokenizer_epoch: 1, policy_epoch: 0 },
        9, b"Review".to_vec(),
    ).unwrap())])).unwrap();
    let target = spec(0, b"risk").target.unwrap();
    let config = ControllerConfig {
        scope: spec(0, b"risk").scope, total: 100, max_attempts: 16,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
            model_generation: 3, tokenizer_generation: 4, state_schema_generation: 1,
            grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
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
    (owner, endpoint)
}

pub(super) fn enable(owner: &mut OversightBroker, source: &CheckedLearnedKv, weights: &[f32],
    budget: LearnedMonitorBudget, alpha: ErrorBudget)
{
    owner.enable_learned_action_consistency(LearnedConsistencyConfig {
        consistency: config(source, weights, alpha), layer: 1, side: KvSide::Key, budget,
    }).unwrap();
}

pub(super) fn inputs(owner: &OversightBroker, action: &FrozenAction) -> CommitteeInput {
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

pub(super) fn review(owner: &mut OversightBroker, action: &FrozenAction, id: u64) -> CommitteeInput {
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
