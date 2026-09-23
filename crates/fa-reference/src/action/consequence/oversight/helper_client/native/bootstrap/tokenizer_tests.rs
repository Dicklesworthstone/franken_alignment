//! Real checkpoint/reader/file paths; synthetic weights, not trained-model evidence.
use super::*;
use super::tests::{Fixture, budget};
use super::super::{NativeEvaluationError, NativeEvaluationStatus};
use super::super::tests::input;
use files::{NativeAsset, NativeAssetReadBudget, NativeFileBootstrap, NativeFileBootstrapError,
    NativeHelperFiles, NativeHelperFileLimits, MAX_ASSET_READ_BYTES, MAX_ASSET_READ_CALLS};
use files::sharded::{NativeHelperShardFiles, NativeShardFileBootstrap,
    NativeShardFileBootstrapError, MAX_SHARDED_ASSET_READ_BYTES};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::GenerationFinish;
use crate::action::consequence::activation::tensor::kv::decoder::safetensors::shards::MAX_WEIGHT_INDEX_BYTES;
use crate::round::Verdict;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const JSON: NativeTokenizerFormat = NativeTokenizerFormat::HuggingFaceRawByteLevel;

fn quote(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}
fn tokenizer_json() -> Vec<u8> {
    // Independent fixed vocabulary/merge spelling for the existing numerical
    // fixture. No production serializer supplies the external JSON expectation.
    let mut vocab = (0_u16..=255).map(|byte| {
        let code = match byte {
            0..=32 => u32::from(byte) + 256,
            33..=126 | 161..=172 | 174..=255 => u32::from(byte),
            127..=160 => u32::from(byte) + 162,
            173 => 323,
            _ => unreachable!("byte"),
        };
        format!("{}:{byte}", quote(&char::from_u32(code).unwrap().to_string()))
    }).collect::<Vec<_>>();
    for (offset, word) in ["al", "all", "allo", "allow", "de", "den", "deny"].iter().enumerate() {
        vocab.push(format!("{}:{}", quote(word), 256 + offset));
    }
    format!(concat!(
        "{{\"version\":\"1.0\",\"truncation\":null,\"padding\":null,",
        "\"added_tokens\":[{{\"id\":263,\"content\":\"<eos>\",\"single_word\":false,",
        "\"lstrip\":false,\"rstrip\":false,\"normalized\":false,\"special\":true}}],",
        "\"normalizer\":null,\"pre_tokenizer\":{{\"type\":\"ByteLevel\",",
        "\"add_prefix_space\":false,\"trim_offsets\":false,\"use_regex\":false}},",
        "\"post_processor\":null,\"decoder\":{{\"type\":\"ByteLevel\",",
        "\"add_prefix_space\":false,\"trim_offsets\":false,\"use_regex\":false}},",
        "\"model\":{{\"type\":\"BPE\",\"dropout\":null,\"unk_token\":null,",
        "\"continuing_subword_prefix\":null,\"end_of_word_suffix\":null,",
        "\"fuse_unk\":false,\"byte_fallback\":false,\"ignore_merges\":false,",
        "\"vocab\":{{{}}},\"merges\":[[\"a\",\"l\"],[\"al\",\"l\"],",
        "[\"all\",\"o\"],[\"allo\",\"w\"],[\"d\",\"e\"],[\"de\",\"n\"],[\"den\",\"y\"]]}}}}"
    ), vocab.join(",")).into_bytes()
}

fn shards(fixture: &Fixture) -> (Vec<u8>, BTreeMap<String, Cursor<Vec<u8>>>) {
    let mut map = Vec::new();
    let mut sources = BTreeMap::new();
    for (part, tensors) in [&fixture.tensors[..3], &fixture.tensors[3..]].into_iter().enumerate() {
        let label = format!("part-{part}.safetensors");
        let mut header = Vec::new(); let mut body = Vec::new();
        for (name, shape, values) in tensors {
            let start = body.len();
            for value in values { body.extend_from_slice(&value.to_le_bytes()); }
            header.push(format!("{}:{{\"dtype\":\"F32\",\"shape\":{shape:?},\"data_offsets\":[{start},{}]}}",
                quote(name), body.len()));
            map.push(format!("{}:{}", quote(name), quote(&label)));
        }
        let mut header = format!("{{{}}}", header.join(",")).into_bytes();
        while !header.len().is_multiple_of(8) { header.push(b' '); }
        let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
        bytes.extend_from_slice(&header); bytes.extend_from_slice(&body);
        sources.insert(label, Cursor::new(bytes));
    }
    (format!("{{\"weight_map\":{{{}}}}}", map.join(",")).into_bytes(), sources)
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Assets { root: PathBuf, fixture: Fixture, paths: [PathBuf; 6], shards: BTreeMap<String, PathBuf> }
impl Assets {
    fn new(alarm: bool) -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-tokenizer-startup-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap();
        let mut fixture = Fixture::new(alarm); fixture.tokenizer = tokenizer_json();
        let (index, sources) = shards(&fixture);
        let paths = ["config", "tokenizer", "monitor", "sampling", "weights", "index"].map(|name| root.join(name));
        for (path, bytes) in paths.iter().zip([fixture.configuration.clone(), fixture.tokenizer.clone(),
            fixture.monitoring.clone(), fixture.sampling.clone(), fixture.weights(), index]) {
            fs::write(path, bytes).unwrap();
        }
        let mut paths_by_label = BTreeMap::new();
        for (ordinal, (label, bytes)) in sources.into_iter().enumerate() {
            // Actual paths deliberately differ from labels.
            let path = root.join(format!("registered-{ordinal}"));
            fs::write(&path, bytes.into_inner()).unwrap(); paths_by_label.insert(label, path);
        }
        Self { root, fixture, paths, shards: paths_by_label }
    }
    fn single(&self) -> NativeFileBootstrap<'_> {
        NativeFileBootstrap { policy: &self.fixture.policy, stream: 12,
            files: NativeHelperFiles { configuration: &self.paths[0], tokenizer: &self.paths[1],
                monitoring: &self.paths[2], sampling: &self.paths[3], weights: &self.paths[4] },
            limits: NativeHelperFileLimits::default() }
    }
    fn sharded(&self) -> NativeShardFileBootstrap<'_> {
        NativeShardFileBootstrap { policy: &self.fixture.policy, stream: 12,
            files: NativeHelperShardFiles { configuration: &self.paths[0], tokenizer: &self.paths[1],
                monitoring: &self.paths[2], sampling: &self.paths[3], index: &self.paths[5], shards: &self.shards },
            limits: NativeHelperFileLimits::default(), index_bytes: MAX_WEIGHT_INDEX_BYTES }
    }
}
impl Drop for Assets {
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.root) { eprintln!("tokenizer startup cleanup: {error}"); } }
}
fn assets() -> NativeAssetReadBudget { NativeAssetReadBudget::new(MAX_ASSET_READ_BYTES, MAX_ASSET_READ_CALLS).unwrap() }
fn shard_assets() -> NativeAssetReadBudget { NativeAssetReadBudget::for_shards(MAX_SHARDED_ASSET_READ_BYTES, MAX_ASSET_READ_CALLS).unwrap() }

#[test]
fn tokenizer_startup_json_and_native_streams_share_original_inference_and_work() {
    for (prompt, verdict) in [(b"?".as_slice(), Verdict::Allow), (b"!", Verdict::Deny)] {
        let mut fixture = Fixture::new(false); let weights = fixture.weights();
        let (mut native, _) = NativeEvaluator::read_llama_checkpoint(fixture.bootstrap(), &mut Cursor::new(&weights), &mut budget()).unwrap();
        fixture.tokenizer = tokenizer_json();
        let (mut json, receipt) = NativeEvaluator::read_llama_checkpoint_with_tokenizer_format(
            fixture.bootstrap(), &mut Cursor::new(&weights), &mut budget(), JSON).unwrap();
        assert_eq!(json.status(), NativeEvaluationStatus::AwaitingInput);
        assert_eq!(json.position(), 0); assert_eq!(json.sampled_draws(), 0);
        assert_eq!(receipt.weights.file_bytes, weights.len());
        assert_eq!(native.evaluate(&input(prompt)), Ok(verdict));
        assert_eq!(json.evaluate(&input(prompt)), Ok(verdict));
        assert_eq!(json.work(), native.work());
        assert_eq!(json.report().unwrap().generation().tokens(), native.report().unwrap().generation().tokens());
    }
}

#[test]
fn tokenizer_startup_wrong_format_never_falls_back_or_consumes_weights() {
    for json in [false, true] {
        let mut fixture = Fixture::new(false);
        if json { fixture.tokenizer = tokenizer_json(); }
        let wrong = if json { NativeTokenizerFormat::NativeArchive } else { JSON };
        let mut source = Cursor::new(fixture.weights()); let mut usage = budget();
        assert!(matches!(NativeEvaluator::read_llama_checkpoint_with_tokenizer_format(
            fixture.bootstrap(), &mut source, &mut usage, wrong), Err(NativeBootstrapError::Tokenizer(_))));
        assert_eq!(source.position(), 0); assert_eq!(usage.usage().read_calls, 0);
        if json { assert!(NativeEvaluator::read_llama_checkpoint(fixture.bootstrap(), &mut source, &mut usage).is_err()); }
        let correct = if json { JSON } else { NativeTokenizerFormat::NativeArchive };
        assert!(NativeEvaluator::read_llama_checkpoint_with_tokenizer_format(
            fixture.bootstrap(), &mut source, &mut usage, correct).is_ok());
    }
}

#[test]
fn tokenizer_startup_sharded_readers_preserve_named_controls_and_byte_spans() {
    let mut fixture = Fixture::new(false); fixture.tokenizer = tokenizer_json();
    let (index, mut sources) = shards(&fixture); let mut usage = budget();
    let (mut worker, receipt) = NativeEvaluator::read_llama_checkpoint_shards_with_tokenizer_format(
        fixture.bootstrap(), &index, &mut sources, &mut usage, JSON).unwrap();
    assert_eq!(receipt.weights.shards.len(), 2);
    assert_eq!(receipt.weights.file_bytes, usage.usage().bytes_read);
    assert_eq!(worker.evaluate(&input(b"<eos>?")), Ok(Verdict::Allow));
    let report = worker.report().unwrap();
    assert_eq!(report.prompt().source(), b"<eos>?");
    assert_eq!(report.prompt().tokens(), &[263, 63]);
    assert_eq!(report.prompt().spans(), &[0..5, 5..6]);
    assert!(report.prefix_controls().is_empty());
    assert_eq!(report.generation().reviewed_prompt_tokens(), 2);
    assert_eq!(worker.sampled_draws(), 2);
}

#[test]
fn tokenizer_startup_files_and_shards_do_not_depend_on_source_paths_after_loading() {
    let root = Assets::new(false); let mut first_assets = assets(); let mut second_assets = shard_assets();
    let (mut single, _) = NativeEvaluator::from_llama_files_with_tokenizer_format(root.single(), &mut first_assets, &mut budget(), JSON).unwrap();
    let (mut sharded, _) = NativeEvaluator::from_llama_shard_files_with_tokenizer_format(root.sharded(), &mut second_assets, &mut budget(), JSON).unwrap();
    let expected = root.fixture.configuration.len() + root.fixture.tokenizer.len() + root.fixture.monitoring.len() + root.fixture.sampling.len();
    assert_eq!(first_assets.usage().bytes_read, expected);
    assert_eq!(second_assets.usage().bytes_read, expected + fs::metadata(&root.paths[5]).unwrap().len() as usize);
    for path in root.paths.iter().chain(root.shards.values()) { fs::remove_file(path).unwrap(); }
    assert_eq!(single.evaluate(&input(b"<eos>!")), Ok(Verdict::Deny));
    assert_eq!(sharded.evaluate(&input(b"<eos>!")), Ok(Verdict::Deny));
    assert_eq!(single.work(), sharded.work());
}

#[test]
fn tokenizer_startup_unsupported_json_and_stop_policy_refuse_before_index_or_weight_access() {
    for case in 0..3 {
        let mut root = Assets::new(false);
        match case {
            0 => { fs::write(&root.paths[1], String::from_utf8(tokenizer_json()).unwrap().replace("\"use_regex\":false", "\"use_regex\":true")).unwrap(); }
            1 => root.fixture.policy.stop_tokens = vec![63],
            _ => { fs::write(&root.paths[1], b"{}").unwrap(); }
        }
        fs::remove_file(&root.paths[4]).unwrap(); fs::remove_file(&root.paths[5]).unwrap();
        let mut usage = budget();
        assert!(matches!(NativeEvaluator::from_llama_files_with_tokenizer_format(root.single(), &mut assets(), &mut usage, JSON),
            Err(NativeFileBootstrapError::Bootstrap(_))));
        assert!(matches!(NativeEvaluator::from_llama_shard_files_with_tokenizer_format(root.sharded(), &mut shard_assets(), &mut usage, JSON),
            Err(NativeShardFileBootstrapError::Bootstrap(_))));
        assert_eq!(usage.usage().read_calls, 0);
    }
}

#[test]
fn tokenizer_startup_json_file_limits_and_budget_refusal_retain_original_accounting() {
    let root = Assets::new(false);
    for shortage in [0, 1] {
        let mut single = root.single(); single.limits.tokenizer_bytes = root.fixture.tokenizer.len() - shortage;
        let mut usage = budget();
        let result = NativeEvaluator::from_llama_files_with_tokenizer_format(single, &mut assets(), &mut usage, JSON);
        if shortage == 0 { assert!(result.is_ok()); }
        else { assert!(matches!(result, Err(NativeFileBootstrapError::Limit(NativeAsset::Tokenizer)))); assert_eq!(usage.usage().read_calls, 0); }
        let mut request = root.sharded(); request.limits.tokenizer_bytes = root.fixture.tokenizer.len() - shortage;
        assert_eq!(NativeEvaluator::from_llama_shard_files_with_tokenizer_format(request, &mut shard_assets(), &mut budget(), JSON).is_ok(), shortage == 0);
    }
    let mut auxiliary = NativeAssetReadBudget::new(MAX_ASSET_READ_BYTES, 1).unwrap(); let mut weights = budget();
    assert!(NativeEvaluator::from_llama_files_with_tokenizer_format(root.single(), &mut auxiliary, &mut weights, JSON).is_err());
    let spent = auxiliary.usage();
    assert_eq!(spent.read_calls, 1); assert!(spent.bytes_read > 0);
    assert!(NativeEvaluator::from_llama_files_with_tokenizer_format(root.single(), &mut auxiliary, &mut weights, JSON).is_err());
    assert_eq!(auxiliary.usage(), spent); assert_eq!(weights.usage().read_calls, 0);
}

#[test]
fn tokenizer_startup_json_monitor_holds_and_partial_allow_never_become_votes() {
    for alarm in [false, true] {
        let mut root = Assets::new(alarm);
        if !alarm { root.fixture.policy.max_new_tokens = 1; }
        let (mut worker, _) = NativeEvaluator::from_llama_shard_files_with_tokenizer_format(root.sharded(), &mut shard_assets(), &mut budget(), JSON).unwrap();
        let finish = if alarm { GenerationFinish::Held } else { GenerationFinish::TokenLimit };
        assert_eq!(worker.evaluate(&input(b"?")), Err(NativeEvaluationError::Incomplete(finish)));
        assert_eq!(worker.sampled_draws(), 1);
        assert_eq!(worker.report().unwrap().bytes().unwrap(), if alarm { b"".as_slice() } else { b"allow" });
        let spent = worker.work(); assert!(worker.evaluate(&input(b"!" )).is_err()); assert_eq!(worker.work(), spent);
    }
}

#[test]
fn tokenizer_startup_bad_json_never_consumes_any_shard_reader() {
    let mut fixture = Fixture::new(false); fixture.tokenizer = b"{}".to_vec();
    let (index, mut sources) = shards(&fixture); let mut usage = budget();
    assert!(matches!(NativeEvaluator::read_llama_checkpoint_shards_with_tokenizer_format(
        fixture.bootstrap(), &index, &mut sources, &mut usage, JSON), Err(NativeBootstrapError::Tokenizer(_))));
    assert_eq!(usage.usage().read_calls, 0); assert!(sources.values().all(|source| source.position() == 0));
    fixture.tokenizer = tokenizer_json();
    assert!(NativeEvaluator::read_llama_checkpoint_shards_with_tokenizer_format(
        fixture.bootstrap(), &index, &mut sources, &mut usage, JSON).is_ok());
}
