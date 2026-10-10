//! Continuations and complete recipe binding over independently merged dense data.
use super::*;
#[allow(dead_code)]
#[path = "../support/learned_text_model.rs"]
mod training;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{
    SampledSession, archive::{ArchiveLimits, SampledArchive},
    monitored::{GenerationBudget, GenerationSpec, GenerationTelemetryBudget},
    replay::{CheckpointLimits, ReplayBudget, ReplayableGeneration,
        archive::ArchiveLimits as GenerationArchiveLimits},
};
use std::collections::BTreeSet;

fn same(a: &SampledSession, b: &SampledSession) {
    assert_eq!(a.tokens(), b.tokens()); assert_eq!(a.sampled_positions(), b.sampled_positions());
    assert_eq!(a.sampler_state(), b.sampler_state());
    assert_eq!(a.logits().map(bits), b.logits().map(bits));
}
#[test]
fn adapted_sampled_archive_recomputes_against_independent_dense_weights_then_continues() {
    let adapted = adapt(&base(), 2, false, &TARGETS);
    let mut source = adapted.recompute_sampled(9, &[65, 66], budget(), start()).unwrap();
    source.advance_sampled(2, sample_budget()).unwrap();
    source.advance_forced(3, 68, budget()).unwrap();
    let checkpoint = source.checkpoint().unwrap();
    let bytes = checkpoint.encode_archive(ArchiveLimits::default()).unwrap();
    let fresh = dense(&oracle(&base_tensors(), &adapter(2, false, &TARGETS), 2));
    let archive = SampledArchive::decode(&bytes, &fresh, &start().policy, ArchiveLimits::default()).unwrap();
    let (mut replay, receipt) = archive.recompute(&fresh, 10, sample_budget()).unwrap();
    assert_eq!(receipt.tokens_compared, 4); assert_eq!(receipt.sampled_tokens_compared, 1);
    same(&source, &replay);
    for _ in 0..8 {
        let position = source.position();
        let a = source.advance_sampled(position, sample_budget()).unwrap();
        let b = replay.advance_sampled(position, sample_budget()).unwrap();
        assert_eq!(a.choice, b.choice); same(&source, &replay);
    }
    assert!(SampledArchive::decode(&bytes, &base(), &start().policy, ArchiveLimits::default()).is_err());
    assert_eq!(fresh.restore_sampled_checkpoint(&checkpoint, 11,
        DecoderRestoreBudget { cache_values: checkpoint.numerical().cache().normalized_values() }).unwrap_err(), Error::Binding);
}

fn dead_gate_base() -> DecoderModel {
    let mut rows = base_tensors();
    for row in &mut rows {
        if row.name.ends_with(".mlp.up_proj.weight") || row.name.ends_with(".mlp.down_proj.weight") {
            row.bytes.fill(0);
        }
    }
    DecoderModel::from_safetensors(profile(), &encode(&rows)).unwrap().0
}
fn learned(model: &DecoderModel, training_model: &DecoderModel) -> ReplayableGeneration {
    let spec = GenerationSpec::new(vec![65, 66], 3, BTreeSet::new(), start()).unwrap();
    model.replayable_monitored_generation(21, 201, spec, training::policy(training_model, false),
        GenerationBudget::default(), GenerationTelemetryBudget::default()).unwrap()
}
#[test]
fn learned_archives_bind_unused_adapter_parameters_even_before_the_first_token() {
    let base = dead_gate_base();
    let a = adapt(&base, 1, true, &["gate_proj"]);
    let b = adapt(&base, 1, false, &["gate_proj"]);
    assert_eq!(a.profile(), b.profile());
    // Up/down are zero: different gate weights cannot affect ANY logits or
    // captured Q/K/V/residuals. Recipe refusal must compare unused parameter bits.
    same_trace(&a, &b);
    for cut in [0, 2, 4] {
        let mut source = learned(&a, &a);
        for position in 0..cut { source.advance(position).unwrap(); }
        let bytes = source.checkpoint(CheckpointLimits::default()).unwrap()
            .encode_archive(GenerationArchiveLimits::default()).unwrap();
        let wrong = learned(&b, &a);
        assert!(wrong.decode_archive(&bytes, GenerationArchiveLimits::default()).is_err());
        assert_eq!(wrong.generation().position(), 0);
        let blueprint = learned(&a, &a);
        let archive = blueprint.decode_archive(&bytes, GenerationArchiveLimits::default()).unwrap();
        let (mut restored, receipt) = archive.replay(ReplayBudget::default()).unwrap();
        assert_eq!(receipt.recomputation, source.generation().work());
        assert_eq!(receipt.telemetry_recomputation, source.generation().telemetry_work());
        assert_eq!(restored.checkpoint(CheckpointLimits::default()).unwrap()
            .encode_archive(GenerationArchiveLimits::default()).unwrap(), bytes);
        source.run_to_stop().unwrap(); restored.run_to_stop().unwrap();
        assert_eq!(source.generation().accepted_tokens(), restored.generation().accepted_tokens());
        assert_eq!(source.generation().samples(), restored.generation().samples());
        assert_eq!(source.generation().sampler_state(), restored.generation().sampler_state());
        assert_eq!(source.generation().telemetry_work(), restored.generation().telemetry_work());
    }
}
