//! Existing config negotiation and CLI file entry point consume streamed weights.
#[path = "support/weight_fixture.rs"]
#[allow(dead_code)]
mod fixture;
use fixture::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::pretrained::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::reader::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::WeightError;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Cursor, Read};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const CONFIG: &[u8] = include_bytes!("fixtures/decoder_llama_config.json");
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-configured-read-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("configured reader cleanup: {error}"); } }
}
fn budget() -> WeightReadBudget { WeightReadBudget::new(MAX_WEIGHT_READ_BYTES, MAX_WEIGHT_READ_CALLS).unwrap() }
fn work() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
fn bits(values: &[f32]) -> Vec<u32> { values.iter().map(|v| v.to_bits()).collect() }
fn compare(a: &DecoderModel, b: &DecoderModel) {
    let a = a.recompute(1, &[0, 3, 1, 5], work()).unwrap();
    let b = b.recompute(1, &[0, 3, 1, 5], work()).unwrap();
    assert_eq!(bits(a.logits().unwrap()), bits(b.logits().unwrap()));
    assert_eq!(a.cache_image().unwrap().encode().unwrap(), b.cache_image().unwrap().encode().unwrap());
}
struct OneByte(Cursor<Vec<u8>>);
impl Read for OneByte {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let n = output.len().min(1); self.0.read(&mut output[..n])
    }
}
struct MustNotRead;
impl Read for MustNotRead {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> { panic!("unadmitted configuration reached a weight source") }
}

#[test]
fn configured_memory_regular_file_and_fragmented_reader_keep_identical_receipts() {
    let directory = Directory::new(); let p = decoder::profile(16); let bytes = archive(&tensors(&p));
    let config = directory.0.join("config.json"); let weights = directory.0.join("weights.safetensors");
    fs::write(&config, CONFIG).unwrap(); fs::write(&weights, &bytes).unwrap();
    let (memory, expected) = DecoderModel::from_llama_safetensors(p.identity(), 16, CONFIG, &bytes).unwrap();
    let (file, receipt) = DecoderModel::from_llama_files(p.identity(), 16, &config, &weights,
        CheckpointFileLimits { config_bytes: CONFIG.len(), weight_bytes: bytes.len() }).unwrap();
    assert_eq!(receipt, expected); compare(&file, &memory);
    let mut source = OneByte(Cursor::new(bytes.clone())); let mut allowance = budget();
    let (fragmented, receipt) = DecoderModel::read_llama_safetensors(p.identity(), 16, CONFIG, &mut source, &mut allowance).unwrap();
    assert_eq!(receipt, expected); compare(&fragmented, &memory);
    assert_eq!(allowance.usage().bytes_read, bytes.len());
    assert_eq!(allowance.usage().read_calls, bytes.len() + 1);
}

#[test]
fn configured_shard_files_continue_the_original_checkpoint_after_source_removal() {
    let directory = Directory::new(); let p = decoder::profile(16); let ts = tensors(&p);
    let mut groups: BTreeMap<String, Vec<Tensor>> = BTreeMap::new();
    for (i, t) in ts.iter().enumerate() { groups.entry(format!("part-{}.safetensors", i % 3)).or_default().push(t.clone()); }
    let mappings = groups.iter().flat_map(|(file, ts)| ts.iter().map(move |t| format!("\"{}\":\"{file}\"", t.name))).collect::<Vec<_>>().join(",");
    let total: usize = ts.iter().map(|t| t.bytes.len()).sum();
    let index = format!("{{\"metadata\":{{\"total_size\":{total}}},\"weight_map\":{{{mappings}}}}}");
    let mut readers = BTreeMap::new();
    for (file, ts) in &groups {
        let path = directory.0.join(file); fs::write(&path, archive(ts)).unwrap();
        readers.insert(file.clone(), File::open(path).unwrap());
    }
    let mut allowance = budget();
    let (model, receipt) = DecoderModel::read_llama_shards(p.identity(), 16, CONFIG, index.as_bytes(), &mut readers, &mut allowance).unwrap();
    assert_eq!(receipt.configuration, LlamaConfig::decode(p.identity(), 16, CONFIG).unwrap());
    assert_eq!(receipt.weights.data_bytes, total);
    assert_eq!(receipt.weights.file_bytes, allowance.usage().bytes_read);
    compare(&model, &decoder::model(p));
    let mut original = model.recompute(1, &[0, 3, 1, 5], work()).unwrap();
    let checkpoint = original.checkpoint().unwrap();
    drop(readers); drop(directory); drop(groups);
    let (mut restored, _) = model.restore_checkpoint(&checkpoint, 2,
        DecoderRestoreBudget { cache_values: checkpoint.cache().normalized_values() }).unwrap();
    for _ in 0..8 {
        let a = original.advance_greedy(original.position(), work()).unwrap();
        let b = restored.advance_greedy(restored.position(), work()).unwrap();
        assert_eq!(a.token, b.token); assert_eq!(bits(&a.logits), bits(&b.logits));
    }
}

#[test]
fn unsupported_configuration_precedes_index_validation_and_all_weight_io() {
    let p = decoder::profile(16);
    let config = String::from_utf8(CONFIG.to_vec()).unwrap().replace("\"tie_word_embeddings\": false", "\"tie_word_embeddings\": true");
    assert_ne!(config.as_bytes(), CONFIG);
    let mut allowance = budget();
    assert!(matches!(DecoderModel::read_llama_safetensors(p.identity(), 16, config.as_bytes(), &mut MustNotRead, &mut allowance),
        Err(CheckpointError::Configuration { issue: ConfigIssue::Unsupported, .. })));
    let mut sources = BTreeMap::from([("../not-opened.safetensors".into(), MustNotRead)]);
    assert!(matches!(DecoderModel::read_llama_shards(p.identity(), 16, config.as_bytes(), b"bad index", &mut sources, &mut allowance),
        Err(CheckpointError::Configuration { issue: ConfigIssue::Unsupported, .. })));
    assert_eq!(allowance.usage(), WeightReadUsage::default());
}

#[test]
fn original_file_limits_and_truncation_classes_remain_enforced_by_streaming() {
    let directory = Directory::new(); let p = decoder::profile(16); let bytes = archive(&tensors(&p));
    let config = directory.0.join("config.json"); let weights = directory.0.join("weights.safetensors");
    fs::write(&config, CONFIG).unwrap(); fs::write(&weights, &bytes).unwrap();
    let limits = CheckpointFileLimits { config_bytes: CONFIG.len(), weight_bytes: bytes.len() };
    assert!(DecoderModel::from_llama_files(p.identity(), 16, &config, &weights, limits).is_ok());
    assert_eq!(DecoderModel::from_llama_files(p.identity(), 16, &config, &weights,
        CheckpointFileLimits { weight_bytes: bytes.len() - 1, ..limits }).unwrap_err(), CheckpointError::Limit);
    fs::write(&weights, [0; 7]).unwrap();
    assert_eq!(DecoderModel::from_llama_files(p.identity(), 16, &config, &weights, limits).unwrap_err(),
        CheckpointError::Weights(WeightError::Header));
    let mut trailing = bytes; trailing.push(0); fs::write(&weights, trailing).unwrap();
    assert_eq!(DecoderModel::from_llama_files(p.identity(), 16, &config, &weights, CheckpointFileLimits::default()).unwrap_err(),
        CheckpointError::Weights(WeightError::Header));
}
