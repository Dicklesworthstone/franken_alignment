//! Real learned inference, fitted codec/probes and filesystem failure boundaries.
//! The tiny model is a numerical oracle, not evidence of detector effectiveness.
use super::*;
use crate::action::consequence::activation::monitor::{MonitorOutcome, learned::{
    LearnedMonitorBudget, LearnedRefinementMonitor, model::{KvTap, LearnedAuditBudget,
    LearnedAuditPreparationBudget, LearnedModelMonitor}}};
use crate::action::consequence::activation::probe::LinearProbe;
use crate::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderIdentity, DecoderLayerWeights, DecoderModel, DecoderProfile, DecoderShape,
    MAX_DECODER_PRODUCTS, monitoring::{LearnedDecoderPolicy, LearnedStreamRetention},
    sampling::{SamplingPolicy, SamplingStart, monitored::{GenerationBudget, GenerationSpec,
        GenerationStop, GenerationTelemetryBudget}},
};
use crate::action::consequence::activation::tensor::kv::experiment::KvSide;
use crate::action::consequence::activation::tensor::kv::model::learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const ZERO: GenerationFileFloor = GenerationFileFloor { revision: 0, position: 0 };
const STAGES: [GenerationFileIo; 5] = [GenerationFileIo::Stage, GenerationFileIo::Write,
    GenerationFileIo::FileSync, GenerationFileIo::Rename, GenerationFileIo::DirectorySync];
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        Self(std::env::temp_dir().join(format!("fa-file-generation-{}-{time}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))))
    }
    fn bytes(&self) -> Vec<u8> { fs::read(self.0.join(storage::CANONICAL)).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if self.0.exists() && let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("generation cleanup: {error}"); }
    }
}
fn recipe(seed: u64, alarm: bool, telemetry: GenerationTelemetryBudget) -> ReplayableGeneration {
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 }, DecoderShape { vocabulary: 3, hidden: 2,
        intermediate: 2, layers: 2, query_heads: 1, cache_heads: 1, context: 16 }, 0.00001, 10000.0).unwrap();
    let layer = DecoderLayerWeights { attention_norm: vec![1.0; 2], queries: vec![0.0; 4],
        keys: vec![0.0; 4], values: vec![1.0, 0.0, 0.0, 1.0], attention_output: vec![0.0; 4],
        feed_forward_norm: vec![1.0; 2], gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4] };
    let model = DecoderModel::new(profile, vec![1.0, 0.0, -1.0, 0.0, 0.0, 1.0],
        vec![layer.clone(), layer], vec![1.0; 2], vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0]).unwrap();
    let inference = DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS };
    let training = model.recompute(11, &[0, 1], inference).unwrap().cache_image().unwrap();
    let codec = LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(101, training)]), FitBudget::default()).unwrap();
    let mut taps = BTreeMap::new();
    for (layer, contract) in model.cache_profile().layers() {
        for (side, tensor) in [(KvSide::Key, contract.keys()), (KvSide::Value, contract.values())] {
            let mut weights = vec![0.0; tensor.dimensions()];
            let threshold = if alarm && *layer == 2 && side == KvSide::Value { weights[1] = 1.0; 0.5 } else { 1.0 };
            let probe = LinearProbe::new(1, 1, tensor.profile(), &weights, 0.0, threshold).unwrap();
            taps.insert(KvTap { layer: *layer, side },
                LearnedRefinementMonitor::new(vec![probe], LearnedMonitorBudget::default()).unwrap());
        }
    }
    let monitor = LearnedModelMonitor::new(model.cache_profile().clone(), taps, LearnedAuditBudget::default()).unwrap();
    let policy = LearnedDecoderPolicy::new(codec, monitor, LearnedStreamRetention::All,
        LearnedAuditPreparationBudget::default(), inference).unwrap();
    let spec = GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
        policy: SamplingPolicy::new(1, 1, 3, 0.8, 1, 1.0).unwrap(), stream: 71, seed,
    }).unwrap();
    model.replayable_monitored_generation(21, 201, spec, policy, GenerationBudget::default(), telemetry).unwrap()
}
fn quiet() -> ReplayableGeneration { recipe(173, false, GenerationTelemetryBudget::default()) }
fn create(root: &Directory) -> FileGeneration {
    FileGeneration::create(&root.0, quiet(), ArchiveLimits::default()).unwrap()
}
fn same(a: &LearnedGeneration, b: &LearnedGeneration) {
    assert_eq!(a.status(), b.status()); assert_eq!(a.accepted_tokens(), b.accepted_tokens());
    assert_eq!(a.samples(), b.samples()); assert_eq!(a.sampler_state().encode(), b.sampler_state().encode());
    assert_eq!(a.work(), b.work()); assert_eq!(a.telemetry_work(), b.telemetry_work());
    assert_eq!(a.budget(), b.budget()); assert_eq!(a.telemetry_budget(), b.telemetry_budget());
    let bits = |values: &[f32]| values.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
    assert_eq!(a.accepted_logits().map(bits), b.accepted_logits().map(bits));
    assert_eq!(a.accepted_cache_image().unwrap().encode().unwrap(), b.accepted_cache_image().unwrap().encode().unwrap());
}
fn pending(root: &Directory) {
    OpenOptions::new().write(true).create_new(true).mode(0o600).open(root.0.join(storage::PENDING))
        .unwrap().write_all(b"unacknowledged staging").unwrap();
}

#[test]
fn every_file_cut_replays_original_generation_and_continues_with_spent_budgets() {
    let mut uninterrupted = quiet(); uninterrupted.run_to_stop().unwrap();
    for cut in 0..=4 {
        let root = Directory::new(); let mut host = create(&root); let mut oracle = quiet();
        for position in 0..cut {
            let event = host.advance(position, position).unwrap(); oracle.advance(position).unwrap();
            assert_eq!(event.checkpoint.position, position + 1);
            assert_eq!(event.event.accepted().unwrap().token, oracle.generation().accepted_tokens()[position as usize]);
            same(host.generation().unwrap(), oracle.generation());
        }
        let floor = host.last_commit().floor(); drop(host);
        let (mut host, receipt) = FileGeneration::open(&root.0, &quiet(), ArchiveLimits::default(), ReplayBudget::default(), floor).unwrap();
        assert_eq!(receipt.positions as u64, cut); same(host.generation().unwrap(), oracle.generation());
        for position in cut..4 { host.advance(position, position).unwrap(); }
        same(host.generation().unwrap(), uninterrupted.generation());
        assert_eq!(host.generation().unwrap().status(), GenerationStatus::Finished(GenerationStop::TokenLimit));
        let bytes = root.bytes();
        assert_eq!(host.advance(4, 4).unwrap_err(), GenerationFileError::Contract(Error::WrongState));
        assert_eq!(root.bytes(), bytes);
    }
}

#[test]
fn exclusive_owner_and_stale_predecessors_never_run_or_mutate_the_model() {
    let root = Directory::new(); let mut host = create(&root);
    assert!(matches!(FileGeneration::open(&root.0, &quiet(), ArchiveLimits::default(), ReplayBudget::default(), ZERO), Err(GenerationFileError::Busy)));
    for (revision, position) in [(1, 0), (0, 1), (u64::MAX, u64::MAX)] {
        let bytes = root.bytes();
        assert_eq!(host.advance(revision, position).unwrap_err(), GenerationFileError::Contract(Error::Stale));
        assert_eq!(root.bytes(), bytes); assert!(host.failure().is_none());
        same(host.generation().unwrap(), quiet().generation());
    }
    host.advance(0, 0).unwrap();
    let already_advanced = { let mut run = quiet(); run.advance(0).unwrap(); run };
    let other = Directory::new();
    assert!(matches!(FileGeneration::create(&other.0, already_advanced, ArchiveLimits::default()), Err(GenerationFileError::Contract(Error::WrongState))));
    assert!(!other.0.exists());
}

#[test]
fn recovery_checks_recipe_floors_and_exact_state_before_cleaning_staging() {
    let root = Directory::new(); let mut host = create(&root); host.advance(0, 0).unwrap(); drop(host);
    pending(&root); let bytes = root.bytes();
    let wrong = recipe(174, false, GenerationTelemetryBudget::default());
    assert!(FileGeneration::open(&root.0, &wrong, ArchiveLimits::default(), ReplayBudget::default(), ZERO).is_err());
    assert_eq!(root.bytes(), bytes); assert!(root.0.join(storage::PENDING).exists());
    for minimum in [GenerationFileFloor { revision: 2, position: 0 }, GenerationFileFloor { revision: 0, position: 2 }] {
        assert!(matches!(FileGeneration::open(&root.0, &quiet(), ArchiveLimits::default(), ReplayBudget::default(), minimum), Err(GenerationFileError::Contract(Error::Stale))));
        assert_eq!(root.bytes(), bytes); assert!(root.0.join(storage::PENDING).exists());
    }
    let mut corrupted = bytes.clone(); *corrupted.last_mut().unwrap() ^= 1;
    fs::write(root.0.join(storage::CANONICAL), corrupted).unwrap();
    assert!(FileGeneration::open(&root.0, &quiet(), ArchiveLimits::default(), ReplayBudget::default(), ZERO).is_err());
    assert!(root.0.join(storage::PENDING).exists());
    fs::write(root.0.join(storage::CANONICAL), &bytes).unwrap();
    let (host, _) = FileGeneration::open(&root.0, &quiet(), ArchiveLimits::default(), ReplayBudget::default(), ZERO).unwrap();
    assert_eq!(host.last_commit().position, 1); assert!(!root.0.join(storage::PENDING).exists());
    assert_eq!(root.bytes(), bytes);
}

#[test]
fn all_write_barriers_preserve_the_actual_canonical_state_without_an_automatic_retry() {
    for skip in [0, 1] {
        for stage in STAGES {
            let root = Directory::new(); let mut host = create(&root);
            host.store.fail_once(stage, skip);
            assert!(matches!(host.advance(0, 0), Err(GenerationFileError::Io { operation, .. }) if operation == stage));
            assert_eq!(host.last_commit().position, 0); assert!(host.failure().is_some());
            assert!(matches!(host.generation(), Err(GenerationFileError::Unavailable)));
            assert_eq!(host.advance(0, 0).unwrap_err(), GenerationFileError::Unavailable);
            drop(host);
            let reopened = FileGeneration::open(&root.0, &quiet(), ArchiveLimits::default(), ReplayBudget::default(), ZERO);
            match (skip, stage) {
                (0, GenerationFileIo::DirectorySync) | (1, GenerationFileIo::Stage | GenerationFileIo::Write | GenerationFileIo::FileSync | GenerationFileIo::Rename) => {
                    assert!(matches!(reopened, Err(GenerationFileError::Interrupted { revision: 1, position: 0 })));
                }
                _ => {
                    let (host, receipt) = reopened.unwrap();
                    let position = if skip == 1 { 1 } else { 0 };
                    assert_eq!(host.last_commit().position, position); assert_eq!(receipt.positions as u64, position);
                    let mut oracle = quiet(); if position == 1 { oracle.advance(0).unwrap(); }
                    same(host.generation().unwrap(), oracle.generation());
                }
            }
        }
    }
}

#[test]
fn a_real_learned_alarm_cannot_reopen_the_last_quiet_prefix() {
    let root = Directory::new();
    let intended = recipe(173, true, GenerationTelemetryBudget::default());
    let mut host = FileGeneration::create(&root.0, recipe(173, true, GenerationTelemetryBudget::default()), ArchiveLimits::default()).unwrap();
    host.advance(0, 0).unwrap();
    assert_eq!(host.advance(1, 1).unwrap_err(), GenerationFileError::Stopped(GenerationStatus::Held(MonitorOutcome::Alarm)));
    assert_eq!(host.last_commit().position, 1); drop(host);
    assert!(matches!(FileGeneration::open(&root.0, &intended, ArchiveLimits::default(), ReplayBudget::default(), ZERO),
        Err(GenerationFileError::Interrupted { revision: 2, position: 1 })));
    // The near-identical quiet policy remains capable of producing this token.
    let other = Directory::new(); let mut host = create(&other);
    host.advance(0, 0).unwrap(); assert_eq!(host.advance(1, 1).unwrap().event.accepted().unwrap().token, 2);
}

#[test]
fn reopening_does_not_refill_exhausted_original_telemetry_allowances() {
    let mut measure = quiet(); measure.advance(0).unwrap();
    let spent = measure.generation().telemetry_work().source_check_encoded_bytes; assert!(spent > 0);
    let telemetry = GenerationTelemetryBudget { source_check_encoded_bytes: spent, ..GenerationTelemetryBudget::default() };
    let root = Directory::new(); let mut host = FileGeneration::create(&root.0, recipe(173, false, telemetry), ArchiveLimits::default()).unwrap();
    host.advance(0, 0).unwrap(); let floor = host.last_commit().floor(); drop(host);
    let (mut host, _) = FileGeneration::open(&root.0, &recipe(173, false, telemetry), ArchiveLimits::default(), ReplayBudget::default(), floor).unwrap();
    assert_eq!(host.generation().unwrap().telemetry_work().source_check_encoded_bytes, spent);
    assert_eq!(host.advance(1, 1).unwrap_err(), GenerationFileError::Contract(Error::Limit)); drop(host);
    assert!(matches!(FileGeneration::open(&root.0, &recipe(173, false, telemetry), ArchiveLimits::default(), ReplayBudget::default(), floor),
        Err(GenerationFileError::Interrupted { revision: 2, position: 1 })));
}

#[test]
fn strict_file_frames_limits_and_directory_binding_are_not_resume_permissions() {
    let root = Directory::new(); let host = create(&root); let bytes = root.bytes();
    for end in 0..bytes.len() { assert!(parse(&bytes[..end], host.store.identity(), ArchiveLimits::default(), ZERO).is_err()); }
    let mut trailing = bytes.clone(); trailing.push(0);
    assert!(parse(&trailing, host.store.identity(), ArchiveLimits::default(), ZERO).is_err());
    let mut bad = bytes.clone(); bad[24] = 2;
    assert!(parse(&bad, host.store.identity(), ArchiveLimits::default(), ZERO).is_err());
    let other = Directory::new(); let other_host = create(&other);
    assert!(parse(&bytes, other_host.store.identity(), ArchiveLimits::default(), ZERO).is_err());
    let tiny = Directory::new();
    assert!(FileGeneration::create(&tiny.0, quiet(), ArchiveLimits { recipe_bytes: 0, ..ArchiveLimits::default() }).is_err());
    assert!(!tiny.0.exists());
    let limited = Directory::new(); let limits = ArchiveLimits {
        state: super::super::super::CheckpointLimits { positions: 1, ..Default::default() }, ..Default::default()
    };
    let mut host = FileGeneration::create(&limited.0, quiet(), limits).unwrap();
    host.advance(0, 0).unwrap(); let before = limited.bytes();
    assert_eq!(host.advance(1, 1).unwrap_err(), GenerationFileError::Contract(Error::Limit));
    assert_eq!(limited.bytes(), before); assert!(host.failure().is_none());
}

mod recovery;
