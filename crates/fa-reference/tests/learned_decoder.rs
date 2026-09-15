//! Original-engine incremental monitoring, with independent ordinary execution.
#[path = "support/decoder_fixture.rs"] mod fixture;
use fa_reference::action::consequence::activation::monitor::{MonitorOutcome, learned::{
    LearnedMonitorBudget, LearnedRefinementMonitor, model::{KvTap, LearnedAuditBudget,
    LearnedAuditPreparationBudget, LearnedModelMonitor}}};
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, DecoderIdentity,
    DecoderModel, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS, monitoring::{LearnedDecoderPolicy,
    LearnedDecoderStatus, LearnedStreamRetention}};
use fa_reference::action::consequence::activation::tensor::kv::experiment::KvSide;
use fa_reference::action::consequence::activation::tensor::kv::model::learned::{GroupKey,
    LearnedKvCodec, LearnedKvPolicy, FitBudget};
use fa_reference::Error;
use std::collections::{BTreeMap, BTreeSet};

fn inference() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
fn model() -> DecoderModel {
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 },
        DecoderShape { vocabulary: 3, hidden: 2, intermediate: 2, layers: 2,
            query_heads: 1, cache_heads: 1, context: 64 }, 0.00001, 10000.0).unwrap();
    let mut layers = fixture::zero_layers(&profile);
    for layer in &mut layers { layer.values = vec![1.0, 0.0, 0.0, 1.0]; }
    DecoderModel::new(profile, vec![1.0, 0.0, -1.0, 0.0, 0.0, 1.0], layers,
        vec![1.0; 2], vec![1.0, 0.0, 0.0, 1.0, 1.0, -1.0]).unwrap()
}
fn fitted(model: &DecoderModel) -> LearnedKvCodec {
    let source = model.recompute(11, &[0, 1], inference()).unwrap().cache_image().unwrap();
    LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(101, source)]), FitBudget::default()).unwrap()
}
fn policy(model: &DecoderModel, selected: Option<(Vec<f32>, f32)>, retention: LearnedStreamRetention,
    budget: LearnedAuditBudget) -> LearnedDecoderPolicy
{
    let profile = model.cache_profile();
    let mut taps = BTreeMap::new();
    for (layer, contract) in profile.layers() {
        for (side, tensor) in [(KvSide::Key, contract.keys()), (KvSide::Value, contract.values())] {
            let (weights, threshold) = match &selected {
                Some(value) if *layer == 2 && side == KvSide::Value => value.clone(),
                _ => (vec![0.0; tensor.dimensions()], 1.0),
            };
            let probe = LinearProbe::new(1, 1, tensor.profile(), &weights, 0.0, threshold).unwrap();
            taps.insert(KvTap { layer: *layer, side },
                LearnedRefinementMonitor::new(vec![probe], LearnedMonitorBudget::default()).unwrap());
        }
    }
    let monitor = LearnedModelMonitor::new(profile.clone(), taps, budget).unwrap();
    LearnedDecoderPolicy::new(fitted(model), monitor, retention,
        LearnedAuditPreparationBudget::default(), inference()).unwrap()
}
fn bits(values: &[f32]) -> Vec<u32> { values.iter().map(|v| v.to_bits()).collect() }

#[test]
fn accepted_nontrivial_execution_is_bitwise_original_and_audits_one_new_position_not_the_prefix() {
    let model = fixture::model(fixture::profile(16));
    let policy = policy(&model, None, LearnedStreamRetention::None, LearnedAuditBudget::default());
    let mut guarded = model.monitored_session(21, 201, policy).unwrap();
    let mut ordinary = model.session(21).unwrap();
    let mut inspected_values = 0;
    for (position, token) in [4, 3, 5, 0, 2, 1].into_iter().enumerate() {
        let expected = ordinary.advance(position as u64, token, inference()).unwrap();
        let event = guarded.advance(position as u64, token).unwrap();
        let step = event.step().unwrap();
        assert_eq!(bits(&step.logits), bits(&expected.logits));
        assert_eq!(step.work, expected.work); assert_eq!(step.layers.len(), expected.layers.len());
        assert_eq!(event.audit().first_position(), position as u64);
        assert_eq!(event.audit().end_position(), position as u64 + 1);
        assert_eq!(event.audit().planned_rows(), 4); assert!(event.audit().complete_quiet());
        let source = event.audit().source();
        assert_eq!(source.report().source_values, model.cache_profile().values_per_token());
        for layer in source.descriptor().layers().values() {
            assert_eq!(layer.token_count, 1); assert_eq!(layer.source_revision, 1);
            assert_eq!(layer.first_position, position as u64);
            assert_eq!(layer.first_sequence, position as u64 + 1); assert_eq!(layer.stream, 21);
        }
        inspected_values += source.report().source_values;
        assert_eq!(guarded.accepted_cache_image().unwrap().encode().unwrap(), ordinary.cache_image().unwrap().encode().unwrap());
    }
    assert_eq!(inspected_values, 6 * model.cache_profile().values_per_token());
    assert_eq!(guarded.accepted_tokens(), ordinary.tokens());
    assert_eq!(guarded.status(), LearnedDecoderStatus::Active);
}

#[test]
fn late_erased_alarm_holds_original_publication_and_cannot_be_skipped_or_retried() {
    let model = model();
    let policy = policy(&model, Some((vec![0.0, 1.0], 0.5)), LearnedStreamRetention::All, LearnedAuditBudget::default());
    let mut guarded = model.monitored_session(21, 201, policy).unwrap();
    let first = guarded.advance(0, 0).unwrap(); guarded.advance(1, 0).unwrap();
    let before = guarded.accepted_cache_image().unwrap().encode().unwrap();
    let old_logits = bits(guarded.accepted_logits().unwrap());
    let candidate = model.recompute(21, &[0, 0, 2], inference()).unwrap();
    assert_ne!(bits(candidate.logits().unwrap()), old_logits);
    let event = guarded.advance(2, 2).unwrap();
    assert!(event.step().is_none()); assert_eq!(event.audit().outcome(), MonitorOutcome::Alarm);
    assert_eq!(event.audit().planned_rows(), 4); assert_eq!(event.audit().quiet_rows(), 3);
    assert_eq!(event.audit().work().refinements, 1);
    assert_eq!(guarded.status(), LearnedDecoderStatus::Held(MonitorOutcome::Alarm));
    assert_eq!(guarded.position(), 2); assert_eq!(guarded.accepted_tokens(), &[0, 0]);
    assert_eq!(bits(guarded.accepted_logits().unwrap()), old_logits);
    assert_eq!(guarded.accepted_cache_image().unwrap().encode().unwrap(), before);
    assert_eq!(guarded.advance(2, 0).unwrap_err(), Error::WrongState);
    assert_eq!(guarded.advance(3, 0).unwrap_err(), Error::WrongState);
    assert!(first.step().is_some()); assert!(first.audit().complete_quiet());
    assert_eq!(first.audit().first_position(), 0);
}

#[test]
fn missing_evidence_budget_exhaustion_and_threshold_equality_all_withhold_pending_logits() {
    let model = model();
    for (retention, refinements, weights, threshold, expected) in [
        (LearnedStreamRetention::None, 128, vec![0.0, 1.0], 0.5, MonitorOutcome::Unresolved),
        (LearnedStreamRetention::All, 0, vec![0.0, 1.0], 0.5, MonitorOutcome::BudgetExhausted),
        (LearnedStreamRetention::All, 128, vec![0.0, 0.0], 0.0, MonitorOutcome::AtThreshold),
    ] {
        let budget = LearnedAuditBudget { monitoring: LearnedMonitorBudget { refinements, ..LearnedMonitorBudget::default() },
            ..LearnedAuditBudget::default() };
        let mut guarded = model.monitored_session(21, 201, policy(&model, Some((weights, threshold)), retention, budget)).unwrap();
        let before = guarded.accepted_cache_image().unwrap().encode().unwrap();
        let event = guarded.advance(0, 2).unwrap();
        assert_eq!(event.audit().outcome(), expected); assert!(event.step().is_none());
        assert_eq!(guarded.status(), LearnedDecoderStatus::Held(expected));
        assert_eq!(guarded.position(), 0); assert!(guarded.accepted_tokens().is_empty());
        assert_eq!(guarded.accepted_logits(), Err(Error::Incomplete));
        assert_eq!(guarded.accepted_cache_image().unwrap().encode().unwrap(), before);
        assert_eq!(guarded.advance(0, 0).unwrap_err(), Error::WrongState);
    }
}

#[test]
fn structural_residual_selection_follows_absolute_positions_and_never_retains_unselected_heads() {
    let model = model(); let selected = GroupKey { layer: 2, side: KvSide::Value, head: 0 };
    let retention = LearnedStreamRetention::Heads(BTreeSet::from([selected]));
    let policy = policy(&model, Some((vec![0.0, -1.0], 0.5)), retention, LearnedAuditBudget::default());
    let mut guarded = model.monitored_session(21, 201, policy).unwrap();
    for position in 0..3 {
        let event = guarded.advance(position, 2).unwrap();
        assert!(event.step().is_some()); assert_eq!(event.audit().work().refinements, 1);
        assert_eq!(event.audit().source().report().retained_groups, 1);
        let last = event.audit().rows().last().unwrap();
        let promoted = last.view().refined_groups().collect::<Vec<_>>();
        assert_eq!(promoted.len(), 1); assert_eq!(promoted[0].row.position, position);
        assert_eq!(promoted[0].row.layer, 2); assert_eq!(promoted[0].row.side, KvSide::Value);
    }
}

#[test]
fn declared_training_overlap_and_invalid_calls_cannot_consume_or_relabel_an_accepted_prefix() {
    let model = model(); let policy = policy(&model, None, LearnedStreamRetention::None, LearnedAuditBudget::default());
    assert_eq!(model.monitored_session(21, 101, policy.clone()).unwrap_err(), Error::Duplicate);
    assert_eq!(model.monitored_session(11, 201, policy.clone()).unwrap_err(), Error::Duplicate);
    assert_eq!(model.monitored_session(21, 0, policy.clone()).unwrap_err(), Error::InvalidInput);
    let mut guarded = model.monitored_session(21, 201, policy).unwrap();
    assert_eq!(guarded.advance(1, 0).unwrap_err(), Error::Stale);
    assert_eq!(guarded.advance(0, 3).unwrap_err(), Error::InvalidInput);
    assert_eq!(guarded.status(), LearnedDecoderStatus::Active); assert!(guarded.last_event().is_none());
    assert!(guarded.advance(0, 0).unwrap().step().is_some());
    assert_eq!(guarded.advance(0, 0).unwrap_err(), Error::Stale);
    assert_eq!(guarded.position(), 1);
}

#[test]
fn preparation_and_inference_failures_latch_without_committing_a_candidate() {
    let model = model(); let original = policy(&model, None, LearnedStreamRetention::All, LearnedAuditBudget::default());
    for compression_failure in [false, true] {
        let mut preparation = original.preparation();
        if compression_failure { preparation.compression.work_units = 0; }
        let inference = if compression_failure { inference() } else { DecoderBudget { scalar_products: 0 } };
        let policy = LearnedDecoderPolicy::new(original.codec().clone(), original.monitor().clone(),
            LearnedStreamRetention::All, preparation, inference).unwrap();
        let mut guarded = model.monitored_session(21, 201, policy).unwrap();
        let before = guarded.accepted_cache_image().unwrap().encode().unwrap();
        assert_eq!(guarded.advance(0, 0).unwrap_err(), Error::Limit);
        assert_eq!(guarded.status(), LearnedDecoderStatus::Failed(Error::Limit));
        assert_eq!(guarded.position(), 0); assert!(guarded.last_event().is_none());
        assert_eq!(guarded.accepted_cache_image().unwrap().encode().unwrap(), before);
        assert_eq!(guarded.advance(0, 1).unwrap_err(), Error::WrongState);
    }
}
