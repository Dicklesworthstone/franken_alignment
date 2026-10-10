//! Exercise both original learned recipe comparison and durable sampled replay.
use super::*;
#[allow(dead_code)]
#[path = "../support/learned_text_model.rs"]
mod training;
#[allow(dead_code)]
#[path = "../support/file_decoder.rs"]
mod durable;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{
    monitored::{GenerationBudget, GenerationSpec, GenerationTelemetryBudget},
    replay::{CheckpointLimits, ReplayBudget, ReplayableGeneration,
        archive::ArchiveLimits as GenerationArchiveLimits},
};
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileOversight, decoder::FileDecoderConfig,
};
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use std::collections::BTreeSet;

fn learned(scale: RotaryScaling) -> ReplayableGeneration {
    let m = model(scale);
    // Identical codec and zero probes for every profile: only the numerical
    // rotary contract changes in the causal recipe-substitution test.
    let policy = training::policy(&model(RotaryScaling::None), false);
    let spec = GenerationSpec::new(vec![65, 66], 3, BTreeSet::new(), start()).unwrap();
    m.replayable_monitored_generation(21, 201, spec, policy,
        GenerationBudget::default(), GenerationTelemetryBudget::default()).unwrap()
}

#[test]
fn learned_archives_bind_even_empty_scaled_recipes_then_repeat_original_audits() {
    for scale in scaling() {
        for cut in [0, 2, 4] {
            let mut original = learned(scale);
            for position in 0..cut { original.advance(position).unwrap(); }
            let checkpoint = original.checkpoint(CheckpointLimits::default()).unwrap();
            let bytes = checkpoint.encode_archive(GenerationArchiveLimits::default()).unwrap();
            for wrong in [RotaryScaling::None, RotaryScaling::linear(5.0).unwrap(),
                RotaryScaling::llama3(8.0, 1.0, 4.0, 256).unwrap()] {
                let candidate = learned(wrong);
                assert!(candidate.decode_archive(&bytes, GenerationArchiveLimits::default()).is_err());
                assert_eq!(candidate.generation().position(), 0);
            }
            let blueprint = learned(scale);
            let archive = blueprint.decode_archive(&bytes, GenerationArchiveLimits::default()).unwrap();
            let (mut replayed, receipt) = archive.replay(ReplayBudget::default()).unwrap();
            assert_eq!(receipt.recomputation, original.generation().work());
            assert_eq!(receipt.telemetry_recomputation, original.generation().telemetry_work());
            assert_eq!(replayed.checkpoint(CheckpointLimits::default()).unwrap()
                .encode_archive(GenerationArchiveLimits::default()).unwrap(), bytes);
            original.run_to_stop().unwrap(); replayed.run_to_stop().unwrap();
            let a = original.generation(); let b = replayed.generation();
            assert_eq!(a.accepted_tokens(), b.accepted_tokens()); assert_eq!(a.samples(), b.samples());
            assert_eq!(a.sampler_state(), b.sampler_state()); assert_eq!(a.work(), b.work());
            assert_eq!(a.telemetry_work(), b.telemetry_work());
            assert_eq!(a.accepted_cache_image().unwrap().encode().unwrap(), b.accepted_cache_image().unwrap().encode().unwrap());
        }
    }
}

fn file_config(scale: RotaryScaling) -> FileDecoderConfig {
    FileDecoderConfig::new(durable::numerical_profile().with_rotary_scaling(scale).unwrap(),
        durable::data::weights(), durable::data::monitor(3.0), durable::data::sampling(),
        5, DecoderBindingLimits::default()).unwrap()
}

#[cfg(unix)]
#[test]
fn durable_scaled_profile_replays_and_requires_fresh_resume_before_publication() {
    for scale in scaling() {
        let root = durable::Directory::new(); let config = file_config(scale);
        let (mut host, _) = FileOversight::create(root.store(), durable::profile()).unwrap();
        host.enable_decoder(host.revision(), config.clone()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        durable::forced(&mut host, 0);
        durable::sampled(&mut host);
        let before = host.decoder_inspection().unwrap().numerical;
        let revision = host.revision(); let path = root.store().join("delivery.bin");
        let bytes = std::fs::read(&path).unwrap(); drop(host);
        // This fixture has zero Q/K, so all profiles produce the same numbers.
        // Rejection therefore tests exact configuration custody, not a lucky
        // mismatch in an already observed tensor or sampled token.
        for wrong in [RotaryScaling::None, RotaryScaling::linear(5.0).unwrap(),
            RotaryScaling::llama3(8.0, 1.0, 4.0, 256).unwrap()] {
            assert!(FileOversight::open_with_decoder(root.store(), durable::profile(), &file_config(wrong)).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
        let (mut host, human) = FileOversight::open_with_decoder(root.store(), durable::profile(), &config).unwrap();
        assert_eq!(host.revision(), revision + 1);
        assert_eq!(host.decoder_inspection().unwrap().numerical, before);
        assert!(host.decoder_inspection().unwrap().paused); assert!(!host.clock_ready());
        assert!(host.propose(host.revision(), 1, durable::spec(&host, b"scaled"), durable::snapshot()).is_err());
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        host.resume_decoder(host.revision(), before.actor_revision, before.position).unwrap();
        durable::sampled(&mut host);
        let keys = durable::ready(&mut host, &human, 1, b"scaled");
        durable::dispatch(&mut host, &keys);
        assert_eq!(host.publish_checked(host.revision(), 1, Some(&keys.inputs),
            durable::snapshot(), ElapsedTick(2)).unwrap().outcome,
            EndpointOutcome::Executed { resulting_version: 2 });
        host.reconcile(host.revision(), 1).unwrap();
        assert_eq!(host.inspect().executions, 1);
    }
}
