//! Original fit replay from real files, then the original learned generation path.
#[allow(dead_code)]
#[path = "support/restart_model.rs"]
mod support;
use fa_reference::Error;
use fa_reference::action::consequence::activation::{
    monitor::learned::{LearnedMonitorBudget, LearnedRefinementMonitor,
        model::{KvTap, LearnedAuditBudget, LearnedAuditPreparationBudget, LearnedModelMonitor}},
    probe::LinearProbe,
    tensor::kv::{experiment::KvSide,
        decoder::{DecoderBudget, DecoderModel, MAX_DECODER_PRODUCTS,
            monitoring::{LearnedDecoderPolicy, LearnedStreamRetention},
            sampling::{SamplingPolicy, SamplingStart, monitored::{GenerationBudget,
                GenerationSpec, GenerationTelemetryBudget}}},
        model::{ModelKvImage, learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy,
            replay::{LearnedKvFitCheckpoint, archive::{LearnedKvFitArchive, LearnedKvFitBinding,
                MAX_FIT_ARCHIVE_BYTES, reader::{FitArchiveReadBudget, FitArchiveReadError}}}}}},
};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Cursor, Read};

fn inference() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
fn corpus(model: &DecoderModel, changed: bool) -> BTreeMap<u64, ModelKvImage> {
    BTreeMap::from([
        (101, model.recompute(11, if changed { &[2, 2] } else { &[0, 1] }, inference()).unwrap().cache_image().unwrap()),
        (102, model.recompute(12, &[1, 0, 2], inference()).unwrap().cache_image().unwrap()),
    ])
}
fn saved(model: &DecoderModel, changed: bool) -> (LearnedKvCodec, LearnedKvFitCheckpoint) {
    LearnedKvCodec::fit_with_checkpoint(LearnedKvPolicy::new(7, 2, 1, 8).unwrap(),
        &corpus(model, changed), FitBudget::default()).unwrap()
}
fn sections(bytes: &[u8], binding: &LearnedKvFitBinding) -> (usize, usize) {
    // Independent test framing, not a call into the production decoder.
    let witness_length_at = 80 + binding.sources.values().map(|d| 24 + d.descriptor_len()).sum::<usize>();
    let length = u64::from_be_bytes(bytes[witness_length_at..witness_length_at + 8].try_into().unwrap()) as usize;
    (witness_length_at + 8, witness_length_at + 8 + length)
}
fn parsed(bytes: &[u8], binding: &LearnedKvFitBinding) -> LearnedKvFitArchive {
    LearnedKvFitArchive::decode(bytes, binding, MAX_FIT_ARCHIVE_BYTES).unwrap()
}

#[test]
fn fit_archive_has_independent_header_and_exact_roundtrip_without_refitting_at_decode() {
    let model = support::model(); let (codec, saved) = saved(&model, false);
    let binding = saved.binding(); let bytes = saved.encode_archive(MAX_FIT_ARCHIVE_BYTES).unwrap();
    let mut golden = b"FAKVFIT\x01".to_vec();
    for n in [7_u64, 2, 1, 8] { golden.extend_from_slice(&n.to_be_bytes()); }
    assert_eq!(&bytes[..40], golden.as_slice());
    assert!(matches!(saved.encode_archive(bytes.len() - 1), Err(Error::Limit)));
    assert_eq!(saved.encode_archive(bytes.len()).unwrap(), bytes);
    assert!(matches!(LearnedKvFitArchive::decode(&bytes, &binding, bytes.len() - 1), Err(Error::Limit)));
    let archive = LearnedKvFitArchive::decode(&bytes, &binding, bytes.len()).unwrap();
    let (actual, replayed) = archive.replay(FitBudget::default()).unwrap();
    assert_eq!(actual.fit_report(), codec.fit_report());
    assert_eq!(replayed.binding(), binding);
    assert_eq!(replayed.encode_archive(MAX_FIT_ARCHIVE_BYTES).unwrap(), bytes);
}

#[test]
fn fit_archive_rejects_all_truncations_tails_and_configuration_substitution() {
    let model = support::model(); let (_, saved) = saved(&model, false);
    let binding = saved.binding(); let bytes = saved.encode_archive(MAX_FIT_ARCHIVE_BYTES).unwrap();
    for end in 0..bytes.len() {
        assert!(LearnedKvFitArchive::decode(&bytes[..end], &binding, MAX_FIT_ARCHIVE_BYTES).is_err());
    }
    let mut tail = bytes.clone(); tail.push(0);
    assert!(LearnedKvFitArchive::decode(&tail, &binding, MAX_FIT_ARCHIVE_BYTES).is_err());
    for which in 0..4 {
        let mut bad = bytes.clone();
        match which {
            0 => bad[0] ^= 1,
            1 => bad[72..80].copy_from_slice(&u64::MAX.to_be_bytes()),
            2 => bad[80..88].copy_from_slice(&102_u64.to_be_bytes()),
            _ => bad[88..96].copy_from_slice(&u64::MAX.to_be_bytes()),
        }
        assert!(LearnedKvFitArchive::decode(&bad, &binding, MAX_FIT_ARCHIVE_BYTES).is_err());
    }
    let mut wrong = binding.clone(); wrong.policy = LearnedKvPolicy::new(7, 3, 1, 8).unwrap();
    assert!(matches!(LearnedKvFitArchive::decode(&bytes, &wrong, MAX_FIT_ARCHIVE_BYTES), Err(Error::Binding)));
    wrong = binding.clone(); wrong.sources.remove(&101);
    assert!(LearnedKvFitArchive::decode(&bytes, &wrong, MAX_FIT_ARCHIVE_BYTES).is_err());
    assert!(parsed(&bytes, &binding).replay(FitBudget::default()).is_ok());
}

#[test]
fn fit_archive_never_imports_modified_result_words_or_accepts_changed_training_as_the_old_fit() {
    let model = support::model(); let (_, saved) = saved(&model, false);
    let binding = saved.binding(); let bytes = saved.encode_archive(MAX_FIT_ARCHIVE_BYTES).unwrap();
    let (_, alternate) = crate::saved(&model, true);
    assert_eq!(alternate.binding(), binding); // same identity/shape, different actual input
    let alternate_bytes = alternate.encode_archive(MAX_FIT_ARCHIVE_BYTES).unwrap();
    let (witness, payload) = sections(&bytes, &binding);
    let (_, other_payload) = sections(&alternate_bytes, &binding);
    assert_ne!(&bytes[witness..payload], &alternate_bytes[witness..other_payload]);
    let mut changed_result = bytes.clone(); changed_result[payload - 1] ^= 1;
    assert!(matches!(parsed(&changed_result, &binding).replay(FitBudget::default()), Err(Error::Binding)));
    let mut changed_input = bytes.clone();
    changed_input[payload..].copy_from_slice(&alternate_bytes[other_payload..]);
    assert!(matches!(parsed(&changed_input, &binding).replay(FitBudget::default()), Err(Error::Binding)));
    // A genuinely different fit remains usable under its own original result;
    // matching metadata alone is explicitly NOT cryptographic authenticity.
    assert!(parsed(&alternate_bytes, &binding).replay(FitBudget::default()).is_ok());
    assert!(parsed(&bytes, &binding).replay(FitBudget::default()).is_ok());
}

#[test]
fn fit_archive_reader_conserves_bytes_calls_interruptions_and_eof_capacity() {
    struct Fragmented { input: Cursor<Vec<u8>>, interrupted: bool }
    impl Read for Fragmented {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            if !self.interrupted { self.interrupted = true; return Err(io::ErrorKind::Interrupted.into()); }
            let n = output.len().min(7); self.input.read(&mut output[..n])
        }
    }
    let model = support::model(); let (_, saved) = saved(&model, false);
    let binding = saved.binding(); let bytes = saved.encode_archive(MAX_FIT_ARCHIVE_BYTES).unwrap();
    let mut source = Fragmented { input: Cursor::new(bytes.clone()), interrupted: false };
    let mut budget = FitArchiveReadBudget::new(bytes.len() + 1, 10000).unwrap();
    let archive = LearnedKvFitArchive::read(&mut source, &binding, &mut budget).unwrap();
    assert_eq!(budget.consumed_bytes(), bytes.len()); assert_eq!(budget.remaining_bytes(), 1);
    assert_eq!(budget.used_calls(), bytes.len().div_ceil(7) + 2); // interruption + EOF
    assert!(archive.replay(FitBudget::default()).is_ok());
    let mut no_probe = FitArchiveReadBudget::new(bytes.len(), 10000).unwrap();
    assert!(matches!(LearnedKvFitArchive::read(&mut Cursor::new(&bytes), &binding, &mut no_probe),
        Err(FitArchiveReadError::Contract(Error::Limit))));
    assert_eq!(no_probe.consumed_bytes(), bytes.len()); assert_eq!(no_probe.remaining_bytes(), 0);
    let before = no_probe.used_calls();
    assert!(LearnedKvFitArchive::read(&mut Cursor::new(&bytes), &binding, &mut no_probe).is_err());
    assert_eq!(no_probe.used_calls(), before); // cannot issue an unbudgeted read
    let mut no_calls = FitArchiveReadBudget::new(bytes.len() + 1, 1).unwrap();
    assert!(matches!(LearnedKvFitArchive::read(&mut Cursor::new(&bytes), &binding, &mut no_calls),
        Err(FitArchiveReadError::Contract(Error::Limit))));
    assert_eq!(no_calls.used_calls(), 1); assert!(no_calls.consumed_bytes() > 0);
}

#[test]
fn fit_archive_reader_keeps_io_failure_and_rejected_binding_distinct_from_a_codec() {
    struct Broken { first: bool }
    impl Read for Broken {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            if self.first { self.first = false; out[0] = 70; return Ok(1); }
            Err(io::ErrorKind::PermissionDenied.into())
        }
    }
    struct Untouched;
    impl Read for Untouched {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> { panic!("invalid expectation reached source") }
    }
    let model = support::model(); let (_, saved) = saved(&model, false); let binding = saved.binding();
    let mut budget = FitArchiveReadBudget::new(1000, 20).unwrap();
    assert!(matches!(LearnedKvFitArchive::read(&mut Broken { first: true }, &binding, &mut budget),
        Err(FitArchiveReadError::Io(io::ErrorKind::PermissionDenied))));
    assert_eq!(budget.consumed_bytes(), 1); assert_eq!(budget.used_calls(), 2);
    let mut invalid = binding; invalid.sources.clear();
    assert!(LearnedKvFitArchive::read(&mut Untouched, &invalid, &mut budget).is_err());
    assert_eq!(budget.consumed_bytes(), 1); assert_eq!(budget.used_calls(), 2);
}

fn monitoring(model: &DecoderModel, codec: LearnedKvCodec, hold: bool) -> LearnedDecoderPolicy {
    let mut taps = BTreeMap::new();
    for (layer, contract) in model.cache_profile().layers() {
        for (side, tensor) in [(KvSide::Key, contract.keys()), (KvSide::Value, contract.values())] {
            let weights = vec![0.0; tensor.dimensions()];
            let probe = LinearProbe::new(1, 1, tensor.profile(), &weights, 0.0, if hold { -1.0 } else { 1.0 }).unwrap();
            taps.insert(KvTap { layer: *layer, side },
                LearnedRefinementMonitor::new(vec![probe], LearnedMonitorBudget::default()).unwrap());
        }
    }
    let monitor = LearnedModelMonitor::new(model.cache_profile().clone(), taps, LearnedAuditBudget::default()).unwrap();
    LearnedDecoderPolicy::new(codec, monitor, LearnedStreamRetention::All,
        LearnedAuditPreparationBudget::default(), inference()).unwrap()
}

#[test]
fn file_replayed_codec_drives_original_monitored_sampling_and_cannot_supply_a_quiet_verdict() {
    let model = support::model(); let (original, saved) = saved(&model, false);
    let binding = saved.binding(); let bytes = saved.encode_archive(MAX_FIT_ARCHIVE_BYTES).unwrap();
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let path = std::env::temp_dir().join(format!("fa-fit-{}-{nonce}.bin", std::process::id()));
    std::fs::write(&path, &bytes).unwrap();
    let mut file = std::fs::File::open(&path).unwrap();
    let mut io_budget = FitArchiveReadBudget::new(bytes.len() + 1, 10000).unwrap();
    let archive = LearnedKvFitArchive::read(&mut file, &binding, &mut io_budget).unwrap();
    drop(file); std::fs::remove_file(&path).unwrap();
    let (loaded, _) = archive.replay(FitBudget::default()).unwrap();
    let spec = GenerationSpec::new(vec![2, 0], 3, BTreeSet::new(), SamplingStart {
        policy: SamplingPolicy::new(1, 1, 3, 0.8, 3, 1.0).unwrap(), stream: 91, seed: 173,
    }).unwrap();
    let estimate = model.estimate_monitored_generation(&spec).unwrap();
    let budget = GenerationBudget { decoder_products: estimate.decoder.scalar_products().unwrap(),
        vocabulary_scores: estimate.vocabulary_scores };
    let mut reference = model.monitored_generation_with_telemetry(21, 201, spec.clone(),
        monitoring(&model, original, false), budget, GenerationTelemetryBudget::default()).unwrap();
    let mut actual = model.monitored_generation_with_telemetry(21, 201, spec.clone(),
        monitoring(&model, loaded.clone(), false), budget, GenerationTelemetryBudget::default()).unwrap();
    while reference.status().is_active() {
        let position = reference.position();
        assert!(reference.advance(position).unwrap().accepted().is_some());
        assert!(actual.advance(position).unwrap().accepted().is_some());
        assert_eq!(actual.sampler_state(), reference.sampler_state());
        assert_eq!(actual.generated_tokens(), reference.generated_tokens());
        assert_eq!(actual.work(), reference.work()); assert_eq!(actual.status(), reference.status());
        assert_eq!(actual.telemetry_work(), reference.telemetry_work());
        assert_eq!(support::logits(actual.accepted_logits().unwrap()), support::logits(reference.accepted_logits().unwrap()));
        support::same_cache(&actual.accepted_cache_image().unwrap(), &reference.accepted_cache_image().unwrap());
    }
    let mut held = model.monitored_generation_with_telemetry(31, 301, spec,
        monitoring(&model, loaded, true), budget, GenerationTelemetryBudget::default()).unwrap();
    assert!(held.advance(0).unwrap().accepted().is_none());
    assert!(!held.status().is_active()); assert!(held.generated_tokens().is_empty());
}
