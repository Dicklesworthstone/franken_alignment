//! Run-wide learned-telemetry accounting over original monitored generation.
#[path = "support/decoder_fixture.rs"] mod fixture;
use fa_reference::action::consequence::activation::monitor::learned::{LearnedMonitorBudget,
    LearnedRefinementMonitor, model::{KvTap, LearnedAuditBudget, LearnedAuditPreparationBudget, LearnedModelMonitor}};
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, DecoderIdentity,
    DecoderModel, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS, monitoring::{LearnedDecoderPolicy,
    LearnedStreamRetention}, sampling::{SamplingPolicy, SamplingStart, monitored::{GenerationBudget,
    GenerationSpec, GenerationStatus, GenerationTelemetryBudget, GenerationTelemetryWork}}};
use fa_reference::action::consequence::activation::tensor::kv::experiment::KvSide;
use fa_reference::action::consequence::activation::tensor::kv::model::learned::{LearnedKvCodec,
    LearnedKvPolicy, FitBudget};
use fa_reference::Error;
use std::collections::{BTreeMap, BTreeSet};

fn inference() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
fn model() -> DecoderModel {
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 }, DecoderShape { vocabulary: 3, hidden: 2,
            intermediate: 2, layers: 2, query_heads: 1, cache_heads: 1, context: 32 }, 0.00001, 10000.0).unwrap();
    let mut layers = fixture::zero_layers(&profile);
    for layer in &mut layers { layer.values = vec![1.0, 0.0, 0.0, 1.0]; }
    DecoderModel::new(profile, vec![1.0, 0.0, -1.0, 0.0, 0.0, 1.0], layers,
        vec![1.0; 2], vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0]).unwrap()
}
fn policy(model: &DecoderModel, selected: Option<(Vec<f32>, f32)>, retention: LearnedStreamRetention) -> LearnedDecoderPolicy {
    let training = model.recompute(11, &[0, 1], inference()).unwrap().cache_image().unwrap();
    let codec = LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(101, training)]), FitBudget::default()).unwrap();
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
    let monitor = LearnedModelMonitor::new(profile.clone(), taps, LearnedAuditBudget::default()).unwrap();
    LearnedDecoderPolicy::new(codec, monitor, retention, LearnedAuditPreparationBudget::default(), inference()).unwrap()
}
fn start(vocabulary: usize) -> SamplingStart {
    SamplingStart { policy: SamplingPolicy::new(1, 1, vocabulary, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 19 }
}
fn spec(model: &DecoderModel, prompt: Vec<u32>, count: usize) -> GenerationSpec {
    GenerationSpec::new(prompt, count, BTreeSet::new(), start(model.profile().shape().vocabulary)).unwrap()
}
fn quiet(model: &DecoderModel) -> LearnedDecoderPolicy { policy(model, None, LearnedStreamRetention::None) }

#[test]
fn one_refinement_for_the_whole_run_cannot_reset_at_the_next_position() {
    let model = model();
    let guarded = policy(&model, Some((vec![0.0, -1.0], 0.5)), LearnedStreamRetention::All);
    let mut telemetry = GenerationTelemetryBudget::default(); telemetry.monitor_refinements = 1;
    let mut run = model.monitored_generation_with_telemetry(21, 201, spec(&model, vec![2, 2, 2], 1),
        guarded, GenerationBudget::default(), telemetry).unwrap();
    let first = run.advance(0).unwrap();
    assert!(first.accepted().is_some()); assert!(first.audit().complete_quiet());
    assert_eq!(run.telemetry_work().monitor_refinements, 1); assert_eq!(run.position(), 1);
    let before = run.accepted_cache_image().unwrap().encode().unwrap();
    let second = run.advance(1).unwrap();
    assert!(second.accepted().is_none());
    assert_eq!(second.audit().outcome(), fa_reference::action::consequence::activation::monitor::MonitorOutcome::BudgetExhausted);
    assert_eq!(run.status(), GenerationStatus::Held(fa_reference::action::consequence::activation::monitor::MonitorOutcome::BudgetExhausted));
    assert_eq!(run.telemetry_work().monitor_refinements, 1); assert_eq!(run.position(), 1);
    assert_eq!(run.accepted_tokens(), &[2]); assert_eq!(run.accepted_cache_image().unwrap().encode().unwrap(), before);
    assert_eq!(run.advance(1).unwrap_err(), Error::WrongState);
}

#[test]
fn probe_coordinate_allowance_is_conserved_across_quiet_tokens() {
    let model = model();
    let mut oracle = model.monitored_generation(21, 201, spec(&model, vec![0], 1), quiet(&model), GenerationBudget::default()).unwrap();
    oracle.advance(0).unwrap();
    let per = oracle.telemetry_work().monitor_probe_coordinates;
    assert!(per > 0);
    let mut telemetry = GenerationTelemetryBudget::default(); telemetry.monitor_probe_coordinates = per * 2;
    let mut run = model.monitored_generation_with_telemetry(22, 201, spec(&model, vec![0, 0, 0], 1),
        quiet(&model), GenerationBudget::default(), telemetry).unwrap();
    assert!(run.advance(0).unwrap().accepted().is_some());
    assert!(run.advance(1).unwrap().accepted().is_some());
    assert_eq!(run.telemetry_work().monitor_probe_coordinates, per * 2);
    let event = run.advance(2).unwrap();
    assert!(event.accepted().is_none());
    assert_eq!(event.audit().outcome(), fa_reference::action::consequence::activation::monitor::MonitorOutcome::BudgetExhausted);
    assert_eq!(run.telemetry_work().monitor_probe_coordinates, per * 2);
    assert_eq!(run.position(), 2); assert_eq!(run.accepted_tokens(), &[0, 0]);
}

#[test]
fn source_check_bytes_cannot_be_reused_after_the_first_accepted_token() {
    let model = model();
    let mut oracle = model.monitored_generation(21, 201, spec(&model, vec![0], 1), quiet(&model), GenerationBudget::default()).unwrap();
    oracle.advance(0).unwrap();
    let one = oracle.telemetry_work().source_check_encoded_bytes;
    assert!(one > 0);
    let mut telemetry = GenerationTelemetryBudget::default(); telemetry.source_check_encoded_bytes = one;
    let mut run = model.monitored_generation_with_telemetry(22, 201, spec(&model, vec![0, 0], 1),
        quiet(&model), GenerationBudget::default(), telemetry).unwrap();
    run.advance(0).unwrap();
    let before = run.accepted_cache_image().unwrap().encode().unwrap();
    let old = run.telemetry_work();
    assert_eq!(run.advance(1).unwrap_err(), Error::Limit);
    assert_eq!(run.status(), GenerationStatus::Failed(Error::Limit));
    assert_eq!(run.position(), 1); assert_eq!(run.accepted_tokens(), &[0]);
    assert_eq!(run.accepted_cache_image().unwrap().encode().unwrap(), before);
    assert_eq!(run.telemetry_work(), old);
    assert_eq!(run.advance(1).unwrap_err(), Error::WrongState);
}

#[test]
fn reported_telemetry_is_the_sum_of_actual_returned_audits_not_per_token_caps() {
    let model = model();
    let mut run = model.monitored_generation(21, 201, spec(&model, vec![0, 0], 2), quiet(&model), GenerationBudget::default()).unwrap();
    let mut expected = GenerationTelemetryWork::default();
    for position in 0..4 {
        let event = run.advance(position).unwrap();
        let checked = event.audit().source().report(); let monitor = event.audit().work();
        expected.compression_source_values += checked.source_values as u64;
        expected.compression_encoded_bytes += event.compression().encoded_bytes as u64;
        expected.compression_work_units += event.compression().work_units_reserved;
        expected.source_check_values += checked.source_values as u64;
        expected.source_check_encoded_bytes += checked.total_encoded_bytes as u64;
        expected.source_check_reconstruction_products += checked.reconstruction_products;
        expected.monitor_encoded_bytes += monitor.encoded_bytes as u64;
        expected.monitor_probe_coordinates += monitor.probe_coordinates as u64;
        expected.monitor_reconstruction_products += monitor.reconstruction_products;
        expected.monitor_materialized_values += monitor.materialized_values as u64;
        expected.monitor_refinements += monitor.refinements as u64;
        assert_eq!(run.telemetry_work(), expected);
    }
    assert_eq!(run.telemetry_work().monitor_refinements, 0);
    assert_eq!(run.telemetry_work().compression_source_values,
        4 * model.cache_profile().values_per_token() as u64);
}

#[test]
fn invalid_aggregate_budget_refuses_before_inference_and_terminal_polling_spends_nothing() {
    let model = model(); let spec = spec(&model, vec![0], 1); let policy = quiet(&model);
    let mut invalid = GenerationTelemetryBudget::default(); invalid.monitor_refinements += 1;
    assert!(matches!(model.monitored_generation_with_telemetry(21, 201, spec.clone(), policy.clone(),
        GenerationBudget::default(), invalid), Err(Error::Limit)));
    let mut run = model.monitored_generation_with_telemetry(21, 201, spec, policy,
        GenerationBudget::default(), GenerationTelemetryBudget::default()).unwrap();
    run.run_to_stop().unwrap();
    let telemetry = run.telemetry_work(); let work = run.work(); let state = run.sampler_state();
    assert_eq!(run.run_to_stop().unwrap(), run.status());
    assert_eq!(run.telemetry_work(), telemetry); assert_eq!(run.work(), work); assert_eq!(run.sampler_state(), state);
}
