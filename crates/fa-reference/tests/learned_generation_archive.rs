//! Original numerical execution and complete learned audits, not model safety.
#[path = "support/decoder_fixture.rs"] mod fixture;
use fa_reference::action::consequence::activation::monitor::{MonitorOutcome, learned::{
    LearnedMonitorBudget, LearnedRefinementMonitor, model::{KvTap, LearnedAuditBudget,
    LearnedAuditPreparationBudget, LearnedModelMonitor}}};
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderIdentity, DecoderModel, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS,
    monitoring::{LearnedDecoderPolicy, LearnedStreamRetention},
    sampling::{SamplingPolicy, SamplingStart, SampleBudget, SamplingBudget,
        monitored::{GenerationBudget, GenerationSpec, GenerationStatus, GenerationStop, GenerationTelemetryBudget},
        replay::{CheckpointLimits, ReplayBudget, ReplayableGeneration, ReplayStatus}},
};
use fa_reference::action::consequence::activation::tensor::kv::experiment::KvSide;
use fa_reference::action::consequence::activation::tensor::kv::model::learned::{LearnedKvCodec, LearnedKvPolicy, FitBudget};
use fa_reference::Error;
use std::collections::{BTreeMap, BTreeSet};

fn inference() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
fn policy(model: &DecoderModel, alarm: bool) -> LearnedDecoderPolicy {
    let source = model.recompute(11, &[0, 1], inference()).unwrap().cache_image().unwrap();
    let codec = LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(101, source)]), FitBudget::default()).unwrap();
    let mut taps = BTreeMap::new();
    for (layer, contract) in model.cache_profile().layers() {
        for (side, tensor) in [(KvSide::Key, contract.keys()), (KvSide::Value, contract.values())] {
            let mut weights = vec![0.0; tensor.dimensions()];
            let threshold = if alarm && *layer == 2 && side == KvSide::Value {
                weights[1] = 1.0; 0.5
            } else { 1.0 };
            let probe = LinearProbe::new(1, 1, tensor.profile(), &weights, 0.0, threshold).unwrap();
            taps.insert(KvTap { layer: *layer, side },
                LearnedRefinementMonitor::new(vec![probe], LearnedMonitorBudget::default()).unwrap());
        }
    }
    let monitor = LearnedModelMonitor::new(model.cache_profile().clone(), taps, LearnedAuditBudget::default()).unwrap();
    LearnedDecoderPolicy::new(codec, monitor, LearnedStreamRetention::All,
        LearnedAuditPreparationBudget::default(), inference()).unwrap()
}
fn spec(model: &DecoderModel, prompt: Vec<u32>, count: usize, stops: BTreeSet<u32>, top_k: usize) -> GenerationSpec {
    GenerationSpec::new(prompt, count, stops, SamplingStart {
        policy: SamplingPolicy::new(7, 2, model.profile().shape().vocabulary, 0.8, top_k, 1.0).unwrap(),
        stream: 71, seed: 173,
    }).unwrap()
}
fn run(model: &DecoderModel, spec: GenerationSpec, alarm: bool, telemetry: GenerationTelemetryBudget) -> ReplayableGeneration {
    model.replayable_monitored_generation(21, 201, spec, policy(model, alarm),
        GenerationBudget::default(), telemetry).unwrap()
}
fn bits(values: &[f32]) -> Vec<u32> { values.iter().map(|value| value.to_bits()).collect() }
fn equivalent(left: &ReplayableGeneration, right: &ReplayableGeneration) {
    let a = left.generation(); let b = right.generation();
    assert_eq!(a.accepted_tokens(), b.accepted_tokens());
    assert_eq!(a.samples(), b.samples());
    assert_eq!(a.sampler_state().encode(), b.sampler_state().encode());
    assert_eq!(a.status(), b.status());
    assert_eq!(a.work(), b.work());
    assert_eq!(a.telemetry_work(), b.telemetry_work());
    assert_eq!(a.budget(), b.budget());
    assert_eq!(a.telemetry_budget(), b.telemetry_budget());
    assert_eq!(a.accepted_logits().map(bits), b.accepted_logits().map(bits));
    assert_eq!(a.accepted_cache_image().unwrap().encode().unwrap(), b.accepted_cache_image().unwrap().encode().unwrap());
}

use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::replay::archive::{
    ArchiveLimits, ARCHIVE_HEADER_BYTES,
};

fn framed() -> (ReplayableGeneration, Vec<u8>) {
    let model = fixture::model(fixture::profile(16));
    let mut owner = run(&model, spec(&model, vec![4, 0, 3], 5, BTreeSet::new(), 4), false,
        GenerationTelemetryBudget::default());
    for position in 0..5 { owner.advance(position).unwrap(); }
    let bytes = owner.checkpoint(CheckpointLimits::default()).unwrap().encode_archive(ArchiveLimits::default()).unwrap();
    (owner, bytes)
}

#[test]
fn portable_roundtrip_recreates_every_cut_with_independent_native_construction() {
    for cut in 0..=8 {
        let model = fixture::model(fixture::profile(16));
        let planned = spec(&model, vec![4, 0, 3], 5, BTreeSet::new(), 4);
        let mut original = run(&model, planned.clone(), false, GenerationTelemetryBudget::default());
        for position in 0..cut { original.advance(position).unwrap(); }
        let checkpoint = original.checkpoint(CheckpointLimits::default()).unwrap();
        let bytes = checkpoint.encode_archive(ArchiveLimits::default()).unwrap();
        let old_work = original.generation().work();
        let old_telemetry = original.generation().telemetry_work();
        drop(checkpoint); drop(original); drop(model);
        // Rebuild parameters AND fit the codec again; no source Recipe/Rc survives.
        let fresh_model = fixture::model(fixture::profile(16));
        let blueprint = run(&fresh_model, planned, false, GenerationTelemetryBudget::default());
        let archive = blueprint.decode_archive(&bytes, ArchiveLimits::default()).unwrap();
        assert_eq!(archive.layout().positions, cut as usize);
        assert_eq!(blueprint.generation().position(), 0);
        let mut replay = archive.begin_replay(ReplayBudget::default()).unwrap();
        if cut != 0 {
            assert_eq!(replay.advance(0).unwrap(), ReplayStatus::Pending { compared: 0, remaining: cut as usize });
            assert!(replay.receipt().is_none());
        }
        while replay.status() != ReplayStatus::Verified { replay.advance(1).unwrap(); }
        let (mut recovered, receipt) = replay.finish().unwrap();
        assert_eq!(receipt.recomputation, old_work);
        assert_eq!(receipt.telemetry_recomputation, old_telemetry);
        assert_eq!(recovered.generation().work(), old_work);
        assert_eq!(recovered.generation().telemetry_work(), old_telemetry);
        assert_eq!(recovered.checkpoint(CheckpointLimits::default()).unwrap().encode_archive(ArchiveLimits::default()).unwrap(), bytes);
        let mut control = blueprint;
        control.run_to_stop().unwrap(); recovered.run_to_stop().unwrap();
        equivalent(&control, &recovered);
        let config = recovered.generation().spec();
        let mut oracle = fresh_model.sampled_session(21, config.sampling().clone()).unwrap();
        for (position, token) in config.prompt().iter().enumerate() {
            oracle.advance_forced(position as u64, *token, inference()).unwrap();
        }
        let mut choices = Vec::new();
        for position in 3..8 {
            choices.push(oracle.advance_sampled(position, SampleBudget {
                decoder: inference(), sampling: SamplingBudget { vocabulary: 6 },
            }).unwrap().choice);
        }
        assert_eq!(recovered.generation().samples(), choices);
        assert_eq!(recovered.generation().sampler_state(), oracle.sampler_state());
        assert_eq!(recovered.generation().accepted_tokens(), oracle.tokens());
        assert_eq!(bits(recovered.generation().accepted_logits().unwrap()), bits(oracle.logits().unwrap()));
        assert_eq!(recovered.generation().accepted_cache_image().unwrap().encode().unwrap(), oracle.cache_image().unwrap().encode().unwrap());
    }
}

#[test]
fn exact_limits_and_manual_header_agree_without_accepting_truncation_or_suffixes() {
    let (owner, bytes) = framed();
    let checkpoint = owner.checkpoint(CheckpointLimits::default()).unwrap();
    let layout = checkpoint.archive_layout(ArchiveLimits::default()).unwrap();
    let mut header = b"FALGA\0\0\x01".to_vec();
    header.extend_from_slice(&(layout.recipe_bytes as u64).to_be_bytes());
    header.extend_from_slice(&(layout.state_bytes as u64).to_be_bytes());
    assert_eq!(&bytes[..ARCHIVE_HEADER_BYTES], header);
    assert_eq!(layout.encoded_bytes, bytes.len());
    let exact = ArchiveLimits { bytes: bytes.len(), recipe_bytes: layout.recipe_bytes,
        state: CheckpointLimits { positions: 5, state_bytes: checkpoint.state_bytes() } };
    assert_eq!(checkpoint.encode_archive(exact).unwrap(), bytes);
    owner.decode_archive(&bytes, exact).unwrap().replay(ReplayBudget::default()).unwrap();
    for limits in [
        ArchiveLimits { bytes: exact.bytes - 1, ..exact },
        ArchiveLimits { recipe_bytes: exact.recipe_bytes - 1, ..exact },
        ArchiveLimits { state: CheckpointLimits { positions: 4, ..exact.state }, ..exact },
        ArchiveLimits { state: CheckpointLimits { state_bytes: exact.state.state_bytes - 1, ..exact.state }, ..exact },
    ] {
        assert!(matches!(checkpoint.encode_archive(limits), Err(Error::Limit)));
        assert!(matches!(owner.decode_archive(&bytes, limits), Err(Error::Limit)));
    }
    for end in 0..bytes.len() {
        assert!(owner.decode_archive(&bytes[..end], exact).is_err(), "prefix {end}");
    }
    let mut trailing = bytes.clone(); trailing.push(0);
    assert!(owner.decode_archive(&trailing, ArchiveLimits::default()).is_err());
    for offset in [8, 16] {
        let mut bad = bytes.clone(); bad[offset..offset + 8].copy_from_slice(&u64::MAX.to_be_bytes());
        assert!(matches!(owner.decode_archive(&bad, ArchiveLimits::default()), Err(Error::Limit)));
    }
}

#[test]
fn every_recipe_byte_is_binding_material_even_before_any_token_exists() {
    let model = fixture::model(fixture::profile(16));
    let owner = run(&model, spec(&model, vec![1, 2], 3, BTreeSet::new(), 0), false,
        GenerationTelemetryBudget::default());
    let checkpoint = owner.checkpoint(CheckpointLimits::default()).unwrap();
    let layout = checkpoint.archive_layout(ArchiveLimits::default()).unwrap();
    let bytes = checkpoint.encode_archive(ArchiveLimits::default()).unwrap();
    owner.decode_archive(&bytes, ArchiveLimits::default()).unwrap().replay(ReplayBudget::default()).unwrap();
    // Zero-position state cannot expose a model/monitor substitution through
    // final logits. Corrupt each separate recipe byte, keeping all framing intact.
    for offset in ARCHIVE_HEADER_BYTES..ARCHIVE_HEADER_BYTES + layout.recipe_bytes {
        let mut changed = bytes.clone(); changed[offset] ^= 1;
        assert!(matches!(owner.decode_archive(&changed, ArchiveLimits::default()), Err(Error::Binding)), "byte {offset}");
    }
    assert_eq!(owner.generation().position(), 0);
    assert_eq!(owner.generation().sampler_state().draws(), 0);
}

#[test]
fn plausible_state_mutations_never_become_installed_cache_rng_or_balances() {
    let (owner, bytes) = framed();
    let layout = owner.checkpoint(CheckpointLimits::default()).unwrap().archive_layout(ArchiveLimits::default()).unwrap();
    let state = ARCHIVE_HEADER_BYTES + layout.recipe_bytes;
    // Independently specified PUBLISHED V1 layout: header/counts(72),
    // work(96), telemetry(88), sampler(96), five IDs(20), two samples(136),
    // vocabulary logits and cache. No second archive encoding is introduced.
    let token_start = state + 352;
    let sample_start = token_start + 20;
    let logit_start = sample_start + 136;
    for offset in [state + 7, state + 24 + 7, state + 72 + 7, state + 168 + 7,
        state + 256 + 95, token_start + 3,
        sample_start + 4 + 16 + 7, sample_start + 4 + 24 + 7,
        logit_start + 3, bytes.len() - 1] {
        let mut altered = bytes.clone(); altered[offset] ^= 1;
        match owner.decode_archive(&altered, ArchiveLimits::default()) {
            Err(_) => {},
            Ok(parsed) => assert!(parsed.replay(ReplayBudget::default()).is_err(), "state byte {offset}"),
        }
    }
    // A finite rounded zero is structural comparison data, not consent or an
    // opportunity to change the native sampler's actual probability.
    let mut zero_probability = bytes.clone();
    zero_probability[sample_start + 28..sample_start + 36].fill(0);
    let parsed = owner.decode_archive(&zero_probability, ArchiveLimits::default()).unwrap();
    assert!(parsed.replay(ReplayBudget::default()).is_err());
    // The paired original still verifies, and decoding never changes its owner.
    let (recovered, _) = owner.decode_archive(&bytes, ArchiveLimits::default()).unwrap()
        .replay(ReplayBudget::default()).unwrap();
    equivalent(&owner, &recovered);
}

#[test]
fn transport_cannot_refill_spent_telemetry_or_remove_a_finished_stop() {
    let model = fixture::model(fixture::profile(16));
    let planned = spec(&model, vec![1, 2], 5, BTreeSet::new(), 0);
    let mut measured = run(&model, planned.clone(), false, GenerationTelemetryBudget::default());
    measured.advance(0).unwrap();
    let spent = measured.generation().telemetry_work().source_check_encoded_bytes;
    let telemetry = GenerationTelemetryBudget { source_check_encoded_bytes: spent, ..GenerationTelemetryBudget::default() };
    let mut original = run(&model, planned.clone(), false, telemetry);
    original.advance(0).unwrap();
    let bytes = original.checkpoint(CheckpointLimits::default()).unwrap().encode_archive(ArchiveLimits::default()).unwrap();
    let blueprint = run(&model, planned, false, telemetry);
    let (mut recovered, _) = blueprint.decode_archive(&bytes, ArchiveLimits::default()).unwrap()
        .replay(ReplayBudget::default()).unwrap();
    assert_eq!(original.advance(1).unwrap_err(), Error::Limit);
    assert_eq!(recovered.advance(1).unwrap_err(), Error::Limit);
    equivalent(&original, &recovered);
    assert!(matches!(recovered.checkpoint(CheckpointLimits::default()), Err(Error::WrongState)));
    let mut completed = measured;
    completed.run_to_stop().unwrap();
    let bytes = completed.checkpoint(CheckpointLimits::default()).unwrap().encode_archive(ArchiveLimits::default()).unwrap();
    let (mut recovered, _) = completed.decode_archive(&bytes, ArchiveLimits::default()).unwrap().replay(ReplayBudget::default()).unwrap();
    equivalent(&completed, &recovered);
    assert_eq!(recovered.advance(recovered.generation().position()).unwrap_err(), Error::WrongState);
    assert_eq!(recovered.run_to_stop().unwrap(), completed.generation().status());
}

fn alarm_model() -> DecoderModel {
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 }, DecoderShape { vocabulary: 3, hidden: 2,
        intermediate: 2, layers: 2, query_heads: 1, cache_heads: 1, context: 32 }, 0.00001, 10000.0).unwrap();
    let mut layers = fixture::zero_layers(&profile);
    for layer in &mut layers { layer.values = vec![1.0, 0.0, 0.0, 1.0]; }
    DecoderModel::new(profile, vec![1.0, 0.0, -1.0, 0.0, 0.0, 1.0], layers,
        vec![1.0; 2], vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0]).unwrap()
}


#[test]
fn archive_recovery_preserves_stop_tokens_and_cannot_clear_a_later_alarm() {
    for alarm in [false, true] {
        let model = alarm_model();
        let planned = spec(&model, vec![0], 4, BTreeSet::from([2]), 1);
        let mut original = run(&model, planned.clone(), alarm, GenerationTelemetryBudget::default());
        original.advance(0).unwrap();
        let before = original.checkpoint(CheckpointLimits::default()).unwrap().encode_archive(ArchiveLimits::default()).unwrap();
        let terminal = original.advance(1).unwrap();
        let expected = if alarm { GenerationStatus::Held(MonitorOutcome::Alarm) }
            else { GenerationStatus::Finished(GenerationStop::StopToken(2)) };
        assert_eq!(terminal.status(), expected);
        if alarm { assert!(matches!(original.checkpoint(CheckpointLimits::default()), Err(Error::WrongState))); }
        let (mut recovered, _) = original.decode_archive(&before, ArchiveLimits::default()).unwrap().replay(ReplayBudget::default()).unwrap();
        assert_eq!(original.generation().status(), expected);
        assert_eq!(recovered.advance(1).unwrap().status(), expected);
        equivalent(&original, &recovered);
        if !alarm {
            let bytes = original.checkpoint(CheckpointLimits::default()).unwrap().encode_archive(ArchiveLimits::default()).unwrap();
            let model = alarm_model();
            let independent = run(&model, planned, false, GenerationTelemetryBudget::default());
            let (mut terminal, _) = independent.decode_archive(&bytes, ArchiveLimits::default()).unwrap().replay(ReplayBudget::default()).unwrap();
            equivalent(&original, &terminal);
            assert_eq!(terminal.advance(2).unwrap_err(), Error::WrongState);
            assert_eq!(terminal.run_to_stop().unwrap(), expected);
        }
    }
}
