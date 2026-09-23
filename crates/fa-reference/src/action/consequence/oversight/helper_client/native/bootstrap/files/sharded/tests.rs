//! Real shard files and ORIGINAL inference, with explicitly synthetic weights.
use super::*;
use super::super::{NativeFileStage, MAX_ASSET_READ_BYTES};
use super::super::super::tests::{Fixture, budget};
use super::super::super::super::{NativeEvaluationError, NativeEvaluationStatus};
use super::super::super::super::tests::input;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::GenerationFinish;
use crate::action::consequence::activation::tensor::kv::decoder::safetensors::{WeightError, TensorIssue, OutputHead};
use crate::action::consequence::activation::tensor::kv::decoder::safetensors::reader::MAX_WEIGHT_READ_CALLS;
use crate::round::Verdict;
use std::fs;
use std::io::{self, Cursor};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory {
    root: PathBuf,
    assets: [PathBuf; 5],
    paths: BTreeMap<String, PathBuf>,
    bodies: BTreeMap<String, Vec<u8>>,
    index: Vec<u8>,
    fixture: Fixture,
}
impl Directory {
    fn new(alarm: bool) -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-native-shards-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap();
        let assets = ["config.json", "tokenizer.fa", "monitor.json", "sampler.json", "index.json"]
            .map(|name| root.join(name));
        let fixture = Fixture::new(alarm);
        // Independent partition and encoding: labels deliberately differ from
        // actual filenames, so joining an index label to root cannot work.
        let mut bodies = BTreeMap::new(); let mut paths = BTreeMap::new(); let mut mapping = Vec::new();
        for (partition, label, path) in [(0, "first.safetensors", "parameters-a.bin"),
            (1, "second.safetensors", "parameters-b.bin")]
        {
            let mut data = Vec::new(); let mut entries = Vec::new();
            for (i, (name, shape, values)) in fixture.tensors.iter().enumerate() {
                if i % 2 != partition { continue; }
                let start = data.len();
                for value in values { data.extend_from_slice(&value.to_le_bytes()); }
                let dimensions = shape.iter().map(usize::to_string).collect::<Vec<_>>().join(",");
                entries.push(format!("\"{name}\":{{\"dtype\":\"F32\",\"shape\":[{dimensions}],\"data_offsets\":[{start},{}]}}", data.len()));
                mapping.push(format!("\"{name}\":\"{label}\""));
            }
            let mut header = format!("{{{}}}", entries.join(",")).into_bytes();
            while !header.len().is_multiple_of(8) { header.push(b' '); }
            let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
            bytes.extend_from_slice(&header); bytes.extend_from_slice(&data);
            let path = root.join(path); fs::write(&path, &bytes).unwrap();
            bodies.insert(label.to_owned(), bytes); paths.insert(label.to_owned(), path);
        }
        let index = format!("{{\"weight_map\":{{{}}}}}", mapping.join(",")).into_bytes();
        for (path, bytes) in assets.iter().zip([
            fixture.configuration.as_slice(), &fixture.tokenizer, &fixture.monitoring, &fixture.sampling, &index,
        ]) { fs::write(path, bytes).unwrap(); }
        Self { root, assets, paths, bodies, index, fixture }
    }
    fn request(&self) -> NativeShardFileBootstrap<'_> {
        NativeShardFileBootstrap { policy: &self.fixture.policy, stream: 12,
            files: NativeHelperShardFiles { configuration: &self.assets[0], tokenizer: &self.assets[1],
                monitoring: &self.assets[2], sampling: &self.assets[3], index: &self.assets[4], shards: &self.paths },
            limits: NativeHelperFileLimits::default(), index_bytes: MAX_WEIGHT_INDEX_BYTES }
    }
    fn asset_bytes(&self) -> usize {
        self.fixture.configuration.len() + self.fixture.tokenizer.len() + self.fixture.monitoring.len()
            + self.fixture.sampling.len() + self.index.len()
    }
    fn weight_bytes(&self) -> usize { self.bodies.values().map(Vec::len).sum() }
    fn readers(&self) -> BTreeMap<String, Cursor<Vec<u8>>> {
        self.bodies.iter().map(|(label, bytes)| (label.clone(), Cursor::new(bytes.clone()))).collect()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.root) { eprintln!("native shard cleanup: {error}"); }
    }
}
fn assets() -> NativeAssetReadBudget {
    NativeAssetReadBudget::for_shards(MAX_SHARDED_ASSET_READ_BYTES, MAX_ASSET_READ_CALLS).unwrap()
}
fn header_bytes(bytes: &[u8]) -> usize { 8 + u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize }

#[test]
fn shard_files_preserve_original_inference_work_and_independent_source_receipts() {
    for (prompt, verdict) in [(b"?".as_slice(), Verdict::Allow), (b"!", Verdict::Deny)] {
        let root = Directory::new(false); let mut asset_work = assets(); let mut weight_work = budget();
        let (mut loaded, receipt) = NativeEvaluator::from_llama_shard_files(root.request(), &mut asset_work, &mut weight_work).unwrap();
        let (mut direct, expected_receipt) = NativeEvaluator::read_llama_checkpoint_shards(
            root.fixture.bootstrap(), &root.index, &mut root.readers(), &mut budget()).unwrap();
        assert_eq!(receipt, expected_receipt);
        assert_eq!(receipt.weights.file_bytes, root.weight_bytes());
        assert_eq!(receipt.weights.index_bytes, root.index.len());
        assert_eq!(weight_work.usage().bytes_read, root.weight_bytes());
        assert_eq!(asset_work.usage().bytes_read, root.asset_bytes());
        assert_eq!(loaded.status(), NativeEvaluationStatus::AwaitingInput);
        assert_eq!(loaded.position(), 0); assert_eq!(loaded.sampled_draws(), 0);
        // Nothing needs the old filenames after a complete admitted bootstrap.
        for path in root.assets.iter().chain(root.paths.values()) { fs::remove_file(path).unwrap(); }
        let original = input(prompt);
        assert_eq!(loaded.evaluate(&original), Ok(verdict));
        assert_eq!(direct.evaluate(&original), Ok(verdict));
        assert_eq!(loaded.work(), direct.work());
        assert_eq!(loaded.report().unwrap().bytes(), direct.report().unwrap().bytes());
        assert_eq!(loaded.report().unwrap().prompt().source(), prompt);
    }
}

#[test]
fn shard_files_native_binding_preflight_precedes_index_and_weight_open() {
    let root = Directory::new(false);
    fs::remove_file(&root.assets[4]).unwrap();
    for path in root.paths.values() { fs::remove_file(path).unwrap(); }
    let mut corrupted = root.fixture.tokenizer.clone(); corrupted[8] ^= 1;
    fs::write(&root.assets[1], corrupted).unwrap(); let mut weights = budget();
    assert!(matches!(NativeEvaluator::from_llama_shard_files(root.request(), &mut assets(), &mut weights),
        Err(NativeShardFileBootstrapError::Bootstrap(NativeBootstrapError::Tokenizer(Error::Binding)))));
    assert_eq!(weights.usage().read_calls, 0);
    fs::write(&root.assets[1], &root.fixture.tokenizer).unwrap();
    // The near-identical valid profile gets as far as its actual missing index.
    assert!(matches!(NativeEvaluator::from_llama_shard_files(root.request(), &mut assets(), &mut weights),
        Err(NativeShardFileBootstrapError::Asset(NativeFileBootstrapError::Io {
            asset: NativeAsset::WeightIndex, stage: NativeFileStage::Metadata, kind: io::ErrorKind::NotFound }))));
}

#[test]
fn shard_files_bad_index_or_source_roster_cannot_discover_an_unregistered_path() {
    let root = Directory::new(false);
    assert!(NativeEvaluator::from_llama_shard_files(root.request(), &mut assets(), &mut budget()).is_ok());
    for path in root.paths.values() { fs::remove_file(path).unwrap(); }
    for mode in 0..5 {
        let mut paths = root.paths.clone(); let mut index = root.index.clone();
        match mode {
            0 => index = b"{\"weight_map\":{}}".to_vec(),
            1 => index = String::from_utf8(index).unwrap().replace("first.safetensors", "../escape.safetensors").into_bytes(),
            2 => { paths.remove("second.safetensors"); }
            3 => { paths.insert("unselected.safetensors".to_owned(), root.root.join("never-open")); }
            _ => index = b"{\"weight_map\":{},\"weight_map\":{}}".to_vec(),
        }
        fs::write(&root.assets[4], &index).unwrap();
        let mut request = root.request(); request.files.shards = &paths;
        let mut weights = budget();
        assert!(matches!(NativeEvaluator::from_llama_shard_files(request, &mut assets(), &mut weights),
            Err(NativeShardFileBootstrapError::Weights(WeightReadError::Refused(_)))));
        assert_eq!(weights.usage().read_calls, 0);
    }
    fs::write(&root.assets[4], &root.index).unwrap();
    assert!(matches!(NativeEvaluator::from_llama_shard_files(root.request(), &mut assets(), &mut budget()),
        Err(NativeShardFileBootstrapError::Shard { error: NativeFileBootstrapError::Io { kind: io::ErrorKind::NotFound, .. }, .. })));
}

#[test]
fn shard_files_index_and_aggregate_file_ceilings_have_exact_positive_controls() {
    let root = Directory::new(false);
    for index_limit in [false, true] {
        for shortage in [0, 1] {
            let mut request = root.request();
            if index_limit { request.index_bytes = root.index.len() - shortage; }
            else { request.limits.weight_bytes = root.weight_bytes() - shortage; }
            let mut weight_work = budget();
            let result = NativeEvaluator::from_llama_shard_files(request, &mut assets(), &mut weight_work);
            if shortage == 0 { assert!(result.is_ok()); }
            else {
                let expected = if index_limit { NativeAsset::WeightIndex } else { NativeAsset::Weights };
                assert!(matches!(result, Err(NativeShardFileBootstrapError::Asset(NativeFileBootstrapError::Limit(asset))) if asset == expected));
                assert_eq!(weight_work.usage().read_calls, 0);
            }
        }
    }
}

#[test]
fn shard_files_original_reader_checks_actual_declared_total_before_any_scalar() {
    let root = Directory::new(false);
    for shortage in [0, 1] {
        let mut readers = root.readers(); let mut usage = budget();
        let result = DecoderModel::read_safetensors_shards_bounded(root.fixture.policy.decoder_profile.clone(),
            &root.index, &mut readers, &mut usage, root.weight_bytes() - shortage, OutputHead::Independent);
        if shortage == 0 {
            assert!(result.is_ok()); assert_eq!(usage.usage().bytes_read, root.weight_bytes());
        } else {
            assert!(matches!(result, Err(WeightReadError::Refused(WeightError::Limit))));
            for (label, reader) in &readers {
                assert_eq!(reader.position(), header_bytes(&root.bodies[label]) as u64);
            }
        }
    }
}

#[test]
fn shard_files_late_invalid_directory_cannot_consume_earlier_tensor_bodies() {
    let root = Directory::new(false);
    let original = &root.bodies["second.safetensors"];
    let end = header_bytes(original); let mut corrupt = original.clone();
    let header = String::from_utf8(original[8..end].to_vec()).unwrap()
        .replacen("\"dtype\":\"F32\"", "\"dtype\":\"U32\"", 1);
    assert_ne!(header.as_bytes(), &original[8..end]);
    corrupt[8..end].copy_from_slice(header.as_bytes());
    fs::write(&root.paths["second.safetensors"], corrupt).unwrap();
    let mut usage = budget();
    assert!(matches!(NativeEvaluator::from_llama_shard_files(root.request(), &mut assets(), &mut usage),
        Err(NativeShardFileBootstrapError::Weights(WeightReadError::Refused(
            WeightError::Tensor { issue: TensorIssue::Encoding, .. })))));
    let headers: usize = root.bodies.values().map(|bytes| header_bytes(bytes)).sum();
    assert_eq!(usage.usage().bytes_read, headers);
    fs::write(&root.paths["second.safetensors"], original).unwrap();
    assert!(NativeEvaluator::from_llama_shard_files(root.request(), &mut assets(), &mut usage).is_ok());
    assert_eq!(usage.usage().bytes_read, headers + root.weight_bytes());
}

#[test]
fn shard_files_truncation_trailing_and_nonfinite_failures_keep_consumed_read_work() {
    for mode in 0..3 {
        let root = Directory::new(false); let mut corrupt = root.bodies["second.safetensors"].clone();
        match mode {
            0 => { corrupt.pop(); }
            1 => corrupt.push(0),
            _ => {
                let start = header_bytes(&corrupt);
                corrupt[start..start + 4].copy_from_slice(&f32::INFINITY.to_le_bytes());
            }
        }
        fs::write(&root.paths["second.safetensors"], &corrupt).unwrap();
        let mut weights = budget(); let mut asset_work = assets();
        for _ in 0..2 {
            let before = weights.usage(); let before_assets = asset_work.usage();
            assert!(matches!(NativeEvaluator::from_llama_shard_files(root.request(), &mut asset_work, &mut weights),
                Err(NativeShardFileBootstrapError::Weights(_))));
            assert!(weights.usage().bytes_read > before.bytes_read);
            assert!(weights.usage().read_calls > before.read_calls);
            assert!(asset_work.usage().bytes_read > before_assets.bytes_read);
        }
    }
}

#[test]
fn shard_files_regular_file_checks_cover_index_and_every_declared_shard() {
    let root = Directory::new(false);
    for path in std::iter::once(&root.assets[4]).chain(root.paths.values()) {
        let original = fs::read(path).unwrap();
        fs::remove_file(path).unwrap(); fs::create_dir(path).unwrap();
        let mut weights = budget();
        let error = NativeEvaluator::from_llama_shard_files(root.request(), &mut assets(), &mut weights).unwrap_err();
        assert!(matches!(error, NativeShardFileBootstrapError::Asset(NativeFileBootstrapError::NotRegular(_))
            | NativeShardFileBootstrapError::Shard { error: NativeFileBootstrapError::NotRegular(_), .. }));
        assert_eq!(weights.usage().read_calls, 0);
        fs::remove_dir(path).unwrap();
        #[cfg(unix)] {
            let target = root.root.join("link-target"); fs::write(&target, &original).unwrap();
            std::os::unix::fs::symlink(&target, path).unwrap();
            assert!(NativeEvaluator::from_llama_shard_files(root.request(), &mut assets(), &mut weights).is_err());
            assert_eq!(weights.usage().read_calls, 0); fs::remove_file(path).unwrap();
        }
        fs::write(path, original).unwrap();
    }
    assert!(NativeEvaluator::from_llama_shard_files(root.request(), &mut assets(), &mut budget()).is_ok());
}

#[test]
fn shard_files_loaded_hold_and_invalid_monitor_cannot_fall_back_to_quiet_inference() {
    let root = Directory::new(true);
    let (mut worker, _) = NativeEvaluator::from_llama_shard_files(root.request(), &mut assets(), &mut budget()).unwrap();
    assert_eq!(worker.evaluate(&input(b"?")), Err(NativeEvaluationError::Incomplete(GenerationFinish::Held)));
    assert_eq!(worker.sampled_draws(), 1); assert_eq!(worker.report().unwrap().bytes().unwrap(), b"");
    assert!(worker.evaluate(&input(b"!" )).is_err()); assert_eq!(worker.sampled_draws(), 1);
    fs::write(&root.assets[2], b"{}").unwrap(); let mut usage = budget();
    assert!(matches!(NativeEvaluator::from_llama_shard_files(root.request(), &mut assets(), &mut usage),
        Err(NativeShardFileBootstrapError::Bootstrap(NativeBootstrapError::Sampling(_)))));
    assert_eq!(usage.usage().bytes_read, root.weight_bytes());
}

#[test]
fn shard_files_index_uses_the_same_asset_allowance_and_cannot_refill_an_exhausted_one() {
    let root = Directory::new(false);
    for spare in [0, 1] {
        let mut usage = NativeAssetReadBudget::for_shards(root.asset_bytes() + spare, MAX_ASSET_READ_CALLS).unwrap();
        let mut weights = budget();
        let result = NativeEvaluator::from_llama_shard_files(root.request(), &mut usage, &mut weights);
        assert_eq!(result.is_ok(), spare == 1); assert_eq!(usage.usage().bytes_read, root.asset_bytes());
        if spare == 0 {
            assert_eq!(weights.usage().read_calls, 0); let before = usage.usage();
            assert!(NativeEvaluator::from_llama_shard_files(root.request(), &mut usage, &mut weights).is_err());
            assert_eq!(usage.usage(), before); assert_eq!(weights.usage().read_calls, 0);
        }
    }
    assert!(NativeAssetReadBudget::new(MAX_ASSET_READ_BYTES + 1, 1).is_err());
    assert!(NativeAssetReadBudget::for_shards(MAX_SHARDED_ASSET_READ_BYTES, 1).is_ok());
    assert!(NativeAssetReadBudget::for_shards(MAX_SHARDED_ASSET_READ_BYTES + 1, 1).is_err());
}

#[test]
fn shard_files_bad_bounds_and_empty_sources_refuse_before_any_asset_read() {
    let root = Directory::new(false); let empty = BTreeMap::new();
    for mode in 0..4 {
        let mut request = root.request();
        match mode {
            0 => request.stream = 0,
            1 => request.index_bytes = 0,
            2 => request.limits.weight_bytes = 0,
            _ => request.files.shards = &empty,
        }
        let mut usage = assets(); let mut weights = budget();
        assert!(NativeEvaluator::from_llama_shard_files(request, &mut usage, &mut weights).is_err());
        assert_eq!(usage.usage().read_calls, 0); assert_eq!(weights.usage().read_calls, 0);
    }
}

#[test]
fn shard_files_weight_allowance_includes_all_shards_and_original_eof_probes() {
    let root = Directory::new(false);
    for spare in [0, 1] {
        let mut weights = WeightReadBudget::new(root.weight_bytes() + spare, MAX_WEIGHT_READ_CALLS).unwrap();
        let result = NativeEvaluator::from_llama_shard_files(root.request(), &mut assets(), &mut weights);
        assert_eq!(result.is_ok(), spare == 1);
    }
    let mut weights = WeightReadBudget::new(root.weight_bytes() + 1, 1).unwrap();
    assert!(NativeEvaluator::from_llama_shard_files(root.request(), &mut assets(), &mut weights).is_err());
    assert_eq!(weights.usage().read_calls, 1); let before = weights.usage();
    assert!(NativeEvaluator::from_llama_shard_files(root.request(), &mut assets(), &mut weights).is_err());
    assert_eq!(weights.usage(), before);
}

mod tied;
