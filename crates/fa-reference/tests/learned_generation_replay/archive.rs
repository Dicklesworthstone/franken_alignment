//! Bytes cross an owner-lifetime boundary; original generation does the replay.
use super::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::replay::archive::{
    ArchiveLimits, GenerationArchive, ARCHIVE_HEADER_BYTES,
};

fn bytes(run: &ReplayableGeneration) -> Vec<u8> {
    run.checkpoint(CheckpointLimits::default()).unwrap().encode_archive(ArchiveLimits::default()).unwrap()
}
fn word(bytes: &[u8], at: usize) -> u64 { u64::from_be_bytes(bytes[at..at + 8].try_into().unwrap()) }
fn state_at(bytes: &[u8]) -> usize { ARCHIVE_HEADER_BYTES + word(bytes, 8) as usize }

#[test]
fn archive_survives_dropping_every_original_owner_at_each_generation_cut() {
    let model = fixture::model(fixture::profile(16));
    let spec = spec(&model, vec![4, 0, 3], 5, BTreeSet::new(), 4);
    let mut continuous = run(&model, spec.clone(), false, GenerationTelemetryBudget::default());
    continuous.run_to_stop().unwrap();
    for cut in 0..=8 {
        let mut original = run(&model, spec.clone(), false, GenerationTelemetryBudget::default());
        for position in 0..cut { original.advance(position).unwrap(); }
        let work = original.generation().work();
        let encoded = bytes(&original);
        drop(original);
        // Rebuild model parameters, fitted codec and monitor. No old Rc recipe
        // or typed checkpoint is used to decode the stored bytes.
        let rebuilt = fixture::model(fixture::profile(16));
        let intended = run(&rebuilt, spec.clone(), false, GenerationTelemetryBudget::default());
        let archive = GenerationArchive::decode(&encoded, &intended, ArchiveLimits::default()).unwrap();
        assert_eq!(archive.positions(), cut as usize);
        assert_eq!(archive.encoded_bytes(), encoded.len());
        let (mut restored, receipt) = archive.replay(ReplayBudget::default()).unwrap();
        assert_eq!(receipt.recomputation, work);
        assert_eq!(bytes(&restored), encoded);
        assert_eq!(intended.generation().position(), 0);
        assert_eq!(intended.generation().work().admitted_tokens, 0);
        restored.run_to_stop().unwrap(); equivalent(&continuous, &restored);
    }
}

#[test]
fn parsed_archive_cannot_release_a_partial_reconstruction() {
    let model = alarm_model(); let spec = spec(&model, vec![0], 3, BTreeSet::new(), 1);
    let mut original = run(&model, spec, false, GenerationTelemetryBudget::default());
    for position in 0..3 { original.advance(position).unwrap(); }
    let encoded = bytes(&original);
    let archive = GenerationArchive::decode(&encoded, &original, ArchiveLimits::default()).unwrap();
    assert!(matches!(archive.begin_replay(ReplayBudget::default()).unwrap().finish(), Err(Error::Incomplete)));
    for quantum in [1, 2, usize::MAX] {
        let mut replay = archive.begin_replay(ReplayBudget::default()).unwrap();
        assert_eq!(replay.advance(0).unwrap(), ReplayStatus::Pending { compared: 0, remaining: 3 });
        assert!(replay.receipt().is_none());
        while replay.status() != ReplayStatus::Verified { replay.advance(quantum).unwrap(); }
        let receipt = *replay.receipt().unwrap();
        assert_eq!(replay.advance(0).unwrap(), ReplayStatus::Verified);
        assert_eq!(replay.receipt(), Some(&receipt));
        let (restored, _) = replay.finish().unwrap(); equivalent(&original, &restored);
    }
}

#[test]
fn exact_wire_and_retention_limits_accept_while_every_one_less_refuses() {
    let model = fixture::model(fixture::profile(16));
    let mut original = run(&model, spec(&model, vec![1, 2], 4, BTreeSet::new(), 0), false,
        GenerationTelemetryBudget::default());
    for position in 0..3 { original.advance(position).unwrap(); }
    let saved = original.checkpoint(CheckpointLimits::default()).unwrap();
    let encoded = saved.encode_archive(ArchiveLimits::default()).unwrap();
    let exact = ArchiveLimits { bytes: encoded.len(), recipe_bytes: word(&encoded, 8) as usize,
        state: CheckpointLimits { positions: 3, state_bytes: saved.state_bytes() } };
    assert_eq!(&encoded[..8], b"FALGA\0\0\x01");
    assert_eq!(word(&encoded, 16) as usize, saved.state_bytes());
    let at = state_at(&encoded);
    assert_eq!(word(&encoded, at), 63); // Independent mandatory-components field.
    assert_eq!(word(&encoded, at + 8) as usize, saved.state_bytes());
    assert_eq!(word(&encoded, at + 16), 3);
    assert_eq!(word(&encoded, at + 24), 1); // Generating, not a saved approval.
    assert_eq!(word(&encoded, at + 32), 0);
    assert_eq!(word(&encoded, at + 40), 3);
    assert_eq!(word(&encoded, at + 48), 1);
    assert_eq!(word(&encoded, at + 56), 6);
    assert_eq!(saved.encode_archive(exact).unwrap(), encoded);
    let parsed = GenerationArchive::decode(&encoded, &original, exact).unwrap();
    assert_eq!(parsed.recipe_bytes(), exact.recipe_bytes);
    assert_eq!(parsed.state_bytes(), exact.state.state_bytes);
    parsed.replay(ReplayBudget::default()).unwrap();
    for limited in [
        ArchiveLimits { bytes: exact.bytes - 1, ..exact },
        ArchiveLimits { recipe_bytes: exact.recipe_bytes - 1, ..exact },
        ArchiveLimits { state: CheckpointLimits { positions: 2, ..exact.state }, ..exact },
        ArchiveLimits { state: CheckpointLimits { state_bytes: exact.state.state_bytes - 1, ..exact.state }, ..exact },
    ] {
        assert!(matches!(saved.encode_archive(limited), Err(Error::Limit)));
        assert!(matches!(GenerationArchive::decode(&encoded, &original, limited), Err(Error::Limit)));
    }
}

#[test]
fn truncations_suffixes_components_and_hostile_lengths_refuse_without_advancing_owner() {
    let model = alarm_model();
    let original = run(&model, spec(&model, vec![0], 3, BTreeSet::new(), 1), false, GenerationTelemetryBudget::default());
    let encoded = bytes(&original);
    for end in 0..encoded.len() {
        assert!(GenerationArchive::decode(&encoded[..end], &original, ArchiveLimits::default()).is_err(), "prefix {end}");
    }
    let at = state_at(&encoded);
    for offset in [8, 16, at + 8, at + 16, at + 40, at + 48, at + 56, at + 64] {
        let mut bad = encoded.clone(); bad[offset..offset + 8].copy_from_slice(&u64::MAX.to_be_bytes());
        assert!(GenerationArchive::decode(&bad, &original, ArchiveLimits::default()).is_err(), "count {offset}");
    }
    for (offset, value) in [(0, b'X'), (7, 2), (at + 7, 62), (at + 31, 4), (at + 39, 1)] {
        let mut bad = encoded.clone(); bad[offset] = value;
        assert!(GenerationArchive::decode(&bad, &original, ArchiveLimits::default()).is_err());
    }
    let mut trailing = encoded.clone(); trailing.push(0);
    assert!(GenerationArchive::decode(&trailing, &original, ArchiveLimits::default()).is_err());
    assert_eq!(original.generation().position(), 0);
    GenerationArchive::decode(&encoded, &original, ArchiveLimits::default()).unwrap().replay(ReplayBudget::default()).unwrap();
}

// Each variant is independently admissible under the ORIGINAL constructors.
// Empty-prefix comparisons expose recipe substitutions that output parity alone
// could not detect. No saved flag, integer ID or Debug string is used as binding.
fn configured(model: &DecoderModel, variant: u8) -> ReplayableGeneration {
    let training = model.recompute(if variant == 9 { 12 } else { 11 }, &[0, 1], inference()).unwrap().cache_image().unwrap();
    let codec = LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, if variant == 10 { 7 } else { 8 }).unwrap(),
        &BTreeMap::from([(if variant == 8 { 102 } else { 101 }, training)]), FitBudget::default()).unwrap();
    let mut taps = BTreeMap::new();
    for (layer, contract) in model.cache_profile().layers() {
        for (side, tensor) in [(KvSide::Key, contract.keys()), (KvSide::Value, contract.values())] {
            let mut weights = vec![0.0; tensor.dimensions()];
            if variant == 1 { weights[0] = -0.0; }
            let probe = LinearProbe::new(1, 1, tensor.profile(), &weights,
                if variant == 2 { -0.0 } else { 0.0 }, if variant == 3 { 2.0 } else { 1.0 }).unwrap();
            let mut budget = LearnedMonitorBudget::default(); if variant == 4 { budget.refinements -= 1; }
            taps.insert(KvTap { layer: *layer, side }, LearnedRefinementMonitor::new(vec![probe], budget).unwrap());
        }
    }
    let mut audit = LearnedAuditBudget::default(); if variant == 5 { audit.rows -= 1; }
    let monitor = LearnedModelMonitor::new(model.cache_profile().clone(), taps, audit).unwrap();
    let mut preparation = LearnedAuditPreparationBudget::default(); if variant == 7 { preparation.compression.work_units -= 1; }
    let policy = LearnedDecoderPolicy::new(codec, monitor,
        if variant == 6 { LearnedStreamRetention::None } else { LearnedStreamRetention::All }, preparation, inference()).unwrap();
    let mut sampling = SamplingStart { policy: SamplingPolicy::new(7, 2, model.profile().shape().vocabulary, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173 };
    if variant == 11 { sampling.seed += 1; }
    let spec = GenerationSpec::new(vec![if variant == 12 { 1 } else { 0 }],
        if variant == 13 { 2 } else { 3 }, if variant == 14 { BTreeSet::from([2]) } else { BTreeSet::new() }, sampling).unwrap();
    let mut budget = GenerationBudget::default(); if variant == 15 { budget.decoder_products -= 1; }
    let mut telemetry = GenerationTelemetryBudget::default(); if variant == 16 { telemetry.monitor_encoded_bytes -= 1; }
    model.replayable_monitored_generation(if variant == 17 { 22 } else { 21 },
        if variant == 18 { 202 } else { 201 }, spec, policy, budget, telemetry).unwrap()
}

#[test]
fn identical_ids_cannot_substitute_any_retained_recipe_component_even_at_zero_tokens() {
    let model = alarm_model(); let original = configured(&model, 0); let encoded = bytes(&original);
    GenerationArchive::decode(&encoded, &configured(&model, 0), ArchiveLimits::default()).unwrap();
    for variant in 1..=18 {
        let altered = configured(&model, variant);
        let own = bytes(&altered);
        GenerationArchive::decode(&own, &altered, ArchiveLimits::default()).unwrap();
        assert!(GenerationArchive::decode(&encoded, &altered, ArchiveLimits::default()).is_err(), "variant {variant}");
        assert_eq!(altered.generation().position(), 0);
    }
}

#[test]
fn actual_unused_model_parameters_are_bound_independently_of_the_model_name() {
    let original_model = alarm_model(); let original = configured(&original_model, 0); let encoded = bytes(&original);
    for change in [0, 1] {
        let profile = original_model.profile().clone(); let mut layers = fixture::zero_layers(&profile);
        for layer in &mut layers { layer.values = vec![1.0, 0.0, 0.0, 1.0]; }
        let mut embeddings = vec![1.0, 0.0, -1.0, 0.0, 0.0, 1.0];
        let mut output = vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        if change == 0 { embeddings[5] = 2.0; } else { output[5] = -0.0; }
        let model = DecoderModel::new(profile, embeddings, layers, vec![1.0; 2], output).unwrap();
        assert_eq!(model.profile(), original_model.profile());
        let intended = configured(&model, 0);
        GenerationArchive::decode(&bytes(&intended), &intended, ArchiveLimits::default()).unwrap();
        assert!(matches!(GenerationArchive::decode(&encoded, &intended, ArchiveLimits::default()), Err(Error::Binding)));
    }
}

#[test]
fn well_framed_tampered_expectations_never_become_a_verified_owner() {
    let model = fixture::model(fixture::profile(16));
    let mut original = run(&model, spec(&model, vec![1, 2], 4, BTreeSet::new(), 0), false, GenerationTelemetryBudget::default());
    for position in 0..3 { original.advance(position).unwrap(); }
    let encoded = bytes(&original); let at = state_at(&encoded);
    let sample = at + 352 + 3 * 4; let logits = sample + 68;
    for offset in [at + 119, at + 183, at + 351, sample + 27, logits + 3, encoded.len() - 1] {
        let mut bad = encoded.clone(); bad[offset] ^= 1;
        let archive = GenerationArchive::decode(&bad, &original, ArchiveLimits::default()).unwrap();
        let mut replay = archive.begin_replay(ReplayBudget::default()).unwrap();
        assert_eq!(replay.advance(usize::MAX), Err(Error::Binding), "offset {offset}");
        assert!(replay.receipt().is_none());
        assert_eq!(replay.advance(0), Err(Error::Binding));
        assert!(matches!(replay.finish(), Err(Error::Binding)));
    }
    GenerationArchive::decode(&encoded, &original, ArchiveLimits::default()).unwrap().replay(ReplayBudget::default()).unwrap();
}

#[test]
fn archived_recovery_preserves_spent_telemetry_and_cannot_switch_to_a_larger_budget() {
    let model = fixture::model(fixture::profile(16)); let spec = spec(&model, vec![1, 2], 4, BTreeSet::new(), 0);
    let mut measure = run(&model, spec.clone(), false, GenerationTelemetryBudget::default()); measure.advance(0).unwrap();
    let spent = measure.generation().telemetry_work().source_check_encoded_bytes;
    let budget = GenerationTelemetryBudget { source_check_encoded_bytes: spent, ..GenerationTelemetryBudget::default() };
    let mut original = run(&model, spec.clone(), false, budget); original.advance(0).unwrap();
    let encoded = bytes(&original); drop(original);
    let wider = run(&model, spec.clone(), false, GenerationTelemetryBudget::default());
    assert!(matches!(GenerationArchive::decode(&encoded, &wider, ArchiveLimits::default()), Err(Error::Binding)));
    let intended = run(&model, spec, false, budget);
    let (mut restored, _) = GenerationArchive::decode(&encoded, &intended, ArchiveLimits::default()).unwrap()
        .replay(ReplayBudget::default()).unwrap();
    assert_eq!(restored.generation().telemetry_work().source_check_encoded_bytes, spent);
    assert_eq!(restored.advance(1).unwrap_err(), Error::Limit);
    assert!(matches!(restored.checkpoint(CheckpointLimits::default()), Err(Error::WrongState)));
}

#[test]
fn historical_bytes_cannot_clear_an_alarm_or_turn_a_completed_stop_into_new_work() {
    let model = alarm_model(); let spec = spec(&model, vec![0], 4, BTreeSet::from([2]), 1);
    let mut guarded = run(&model, spec.clone(), true, GenerationTelemetryBudget::default());
    guarded.advance(0).unwrap(); let encoded = bytes(&guarded);
    assert_eq!(guarded.advance(1).unwrap().status(), GenerationStatus::Held(MonitorOutcome::Alarm));
    let (mut replayed, _) = GenerationArchive::decode(&encoded, &guarded, ArchiveLimits::default()).unwrap()
        .replay(ReplayBudget::default()).unwrap();
    assert_eq!(guarded.generation().status(), GenerationStatus::Held(MonitorOutcome::Alarm));
    assert_eq!(replayed.advance(1).unwrap().status(), GenerationStatus::Held(MonitorOutcome::Alarm));
    equivalent(&guarded, &replayed);
    let mut quiet = run(&model, spec, false, GenerationTelemetryBudget::default()); quiet.run_to_stop().unwrap();
    let (mut stopped, _) = GenerationArchive::decode(&bytes(&quiet), &quiet, ArchiveLimits::default()).unwrap()
        .replay(ReplayBudget::default()).unwrap();
    assert_eq!(stopped.generation().status(), GenerationStatus::Finished(GenerationStop::StopToken(2)));
    assert_eq!(stopped.advance(2).unwrap_err(), Error::WrongState); equivalent(&quiet, &stopped);
}
