//! Synthetic weights, original inference and actual SafeTensors framing.
//! These tests compare exact execution, not pretrained quality or authentication.
use super::*;
use super::super::{DecoderBudget, DecoderIdentity, DecoderShape, MAX_DECODER_PRODUCTS};
use super::pretrained::{CheckpointError, CheckpointFileLimits, ConfigIssue, LlamaConfig};
use super::reader::{WeightReadBudget, WeightReadError, WeightReadStage, MAX_WEIGHT_READ_CALLS};
use std::io::{self, Cursor, Read};

const EMBED: [f32; 6] = [1.0, 0.0, 0.0, 1.0, -1.0, 0.5];
const MATRIX: [f32; 4] = [0.25, -0.5, 0.5, 0.25];
fn profile() -> DecoderProfile {
    DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 }, DecoderShape {
        vocabulary: 3, hidden: 2, intermediate: 2, layers: 1,
        query_heads: 1, cache_heads: 1, context: 8,
    }, 1e-5, 10000.0).unwrap()
}
fn config(tied: Option<&str>) -> Vec<u8> {
    let flag = tied.map_or_else(String::new, |value| format!(",\"tie_word_embeddings\":{value}"));
    format!(concat!("{{\"model_type\":\"llama\",\"vocab_size\":3,\"hidden_size\":2,",
        "\"intermediate_size\":2,\"num_hidden_layers\":1,\"num_attention_heads\":1,",
        "\"max_position_embeddings\":8,\"rms_norm_eps\":1e-5{}}}"), flag).into_bytes()
}
fn original() -> DecoderModel {
    DecoderModel::new(profile(), EMBED.to_vec(), vec![DecoderLayerWeights {
        attention_norm: vec![1.0; 2], queries: MATRIX.to_vec(), keys: MATRIX.to_vec(),
        values: MATRIX.to_vec(), attention_output: MATRIX.to_vec(), feed_forward_norm: vec![1.0; 2],
        gate: MATRIX.to_vec(), up: MATRIX.to_vec(), down: MATRIX.to_vec(),
    }], vec![1.0; 2], EMBED.to_vec()).unwrap()
}
#[derive(Clone)]
struct Raw { name: String, shape: Vec<usize>, dtype: &'static str, bytes: Vec<u8> }
fn raw(name: &str, shape: &[usize], values: &[f32]) -> Raw {
    Raw { name: name.to_owned(), shape: shape.to_vec(), dtype: "F32",
        bytes: values.iter().flat_map(|v| v.to_le_bytes()).collect() }
}
fn tensors(head: bool) -> Vec<Raw> {
    // Literal inventory, independent of the production inventory generator.
    let mut values = vec![raw(EMBEDDINGS, &[3, 2], &EMBED), raw("model.norm.weight", &[2], &[1.0; 2])];
    if head { values.push(raw(OUTPUT_HEAD, &[3, 2], &EMBED)); }
    for name in ["input_layernorm.weight", "post_attention_layernorm.weight"] {
        values.push(raw(&format!("model.layers.0.{name}"), &[2], &[1.0; 2]));
    }
    for name in ["self_attn.q_proj.weight", "self_attn.k_proj.weight", "self_attn.v_proj.weight",
        "self_attn.o_proj.weight", "mlp.gate_proj.weight", "mlp.up_proj.weight", "mlp.down_proj.weight"]
    { values.push(raw(&format!("model.layers.0.{name}"), &[2, 2], &MATRIX)); }
    values
}
fn archive(values: &[Raw]) -> Vec<u8> {
    let mut payload = Vec::new(); let mut entries = Vec::new();
    for value in values {
        let start = payload.len(); payload.extend_from_slice(&value.bytes);
        let shape = value.shape.iter().map(usize::to_string).collect::<Vec<_>>().join(",");
        entries.push(format!("\"{}\":{{\"dtype\":\"{}\",\"shape\":[{}],\"data_offsets\":[{},{}]}}",
            value.name, value.dtype, shape, start, payload.len()));
    }
    let mut header = format!("{{{}}}", entries.join(",")).into_bytes();
    while !header.len().is_multiple_of(8) { header.push(b' '); }
    let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
    bytes.extend_from_slice(&header); bytes.extend_from_slice(&payload); bytes
}
fn bits(values: &[f32]) -> Vec<u32> { values.iter().map(|value| value.to_bits()).collect() }
fn compare(model: &DecoderModel) {
    let mut actual = model.session(7).unwrap(); let mut expected = original().session(7).unwrap();
    let budget = DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS };
    for (position, token) in [0, 1, 2, 2, 0, 1].into_iter().enumerate() {
        let a = actual.advance(position as u64, token, budget).unwrap();
        let b = expected.advance(position as u64, token, budget).unwrap();
        assert_eq!(bits(&a.logits), bits(&b.logits)); assert_eq!(a.work, b.work);
        assert_eq!(actual.cache_image().unwrap().encode().unwrap(), expected.cache_image().unwrap().encode().unwrap());
    }
    assert_eq!(actual.work(), expected.work());
}
fn load(bytes: &[u8], tied: Option<&str>) -> Result<(DecoderModel, pretrained::PretrainedReceipt), CheckpointError> {
    DecoderModel::from_llama_safetensors(profile().identity(), 8, &config(tied), bytes)
}
fn allowance(bytes: usize) -> WeightReadBudget { WeightReadBudget::new(bytes, MAX_WEIGHT_READ_CALLS).unwrap() }

#[test]
fn tied_missing_and_redundant_heads_match_the_original_dense_decoder() {
    for stored in [false, true] {
        let bytes = archive(&tensors(stored));
        let (model, receipt) = load(&bytes, Some("true")).unwrap(); compare(&model);
        assert_eq!(receipt.configuration.output_head(), OutputHead::TiedEmbeddings);
        assert!(!receipt.configuration.defaulted_fields().contains("tie_word_embeddings"));
        assert_eq!(receipt.weights.tensors.contains_key(OUTPUT_HEAD), stored);
        assert_eq!(receipt.weights.data_bytes, if stored { 184 } else { 160 });
        assert_eq!(receipt.weights.normalized_bytes, 184); // actual expanded matrices
        assert_eq!(receipt.weights.file_bytes, bytes.len());
    }
}

#[test]
fn tied_configuration_is_explicit_and_does_not_relax_default_loading() {
    let missing = archive(&tensors(false)); let full = archive(&tensors(true));
    for flag in [None, Some("false")] {
        let decoded = LlamaConfig::decode(profile().identity(), 8, &config(flag)).unwrap();
        assert_eq!(decoded.output_head(), OutputHead::Independent);
        assert_eq!(decoded.defaulted_fields().contains("tie_word_embeddings"), flag.is_none());
        assert_eq!(load(&missing, flag).unwrap_err(), CheckpointError::Weights(WeightError::Inventory));
        compare(&load(&full, flag).unwrap().0);
    }
    assert_eq!(DecoderModel::from_safetensors(profile(), &missing).unwrap_err(), WeightError::Inventory);
    for flag in ["null", "1", "\"true\""] {
        assert!(matches!(LlamaConfig::decode(profile().identity(), 8, &config(Some(flag))),
            Err(CheckpointError::Configuration { issue: ConfigIssue::Type, .. })));
    }
}

#[test]
fn tied_duplicate_values_cannot_silently_override_either_matrix() {
    for word in [2.0_f32.to_bits(), (-0.0_f32).to_bits()] {
        let mut values = tensors(true);
        let head = values.iter_mut().find(|value| value.name == OUTPUT_HEAD).unwrap();
        head.bytes[4..8].copy_from_slice(&word.to_le_bytes()); // embedding coordinate is +0
        let bytes = archive(&values);
        assert!(load(&bytes, Some("false")).is_ok()); // a genuinely independent head is legal
        assert_eq!(load(&bytes, Some("true")).unwrap_err(),
            CheckpointError::Weights(issue(OUTPUT_HEAD, TensorIssue::TiedValues)));
        let mut budget = allowance(bytes.len() + 1);
        assert_eq!(DecoderModel::read_safetensors_with_output_head(profile(), &mut Cursor::new(&bytes),
            &mut budget, OutputHead::TiedEmbeddings).unwrap_err(),
            WeightReadError::Refused(issue(OUTPUT_HEAD, TensorIssue::TiedValues)));
        assert_eq!(budget.usage().bytes_read, bytes.len());
    }
}

#[test]
fn tied_f16_bf16_duplicates_compare_normalized_original_bits() {
    for (dtype, words) in [
        ("F16", [0x3c00_u16, 0, 0, 0x3c00, 0xbc00, 0x3800]),
        ("BF16", [0x3f80_u16, 0, 0, 0x3f80, 0xbf80, 0x3f00]),
    ] {
        let mut values = tensors(true);
        let head = values.iter_mut().find(|value| value.name == OUTPUT_HEAD).unwrap();
        head.dtype = dtype; head.bytes = words.iter().flat_map(|word| word.to_le_bytes()).collect();
        let (model, receipt) = load(&archive(&values), Some("true")).unwrap(); compare(&model);
        assert_eq!(receipt.weights.data_bytes, 172);
        assert_eq!(receipt.weights.normalized_bytes, 184);
    }
}

#[test]
fn tied_inventory_and_finite_checks_still_precede_model_construction() {
    for removed in [EMBEDDINGS, "model.norm.weight", "model.layers.0.mlp.down_proj.weight"] {
        let mut values = tensors(false); values.retain(|value| value.name != removed);
        assert_eq!(load(&archive(&values), Some("true")).unwrap_err(), CheckpointError::Weights(WeightError::Inventory));
    }
    for stored in [false, true] {
        let mut values = tensors(stored);
        values[0].bytes[..4].copy_from_slice(&f32::NAN.to_le_bytes());
        assert_eq!(load(&archive(&values), Some("true")).unwrap_err(),
            CheckpointError::Weights(issue(EMBEDDINGS, TensorIssue::NonFinite)));
    }
    let mut values = tensors(true);
    values.iter_mut().find(|value| value.name == OUTPUT_HEAD).unwrap().shape = vec![2, 3];
    assert_eq!(load(&archive(&values), Some("true")).unwrap_err(),
        CheckpointError::Weights(issue(OUTPUT_HEAD, TensorIssue::Shape)));
}

struct Fragmented { source: Cursor<Vec<u8>>, interrupted: bool }
impl Read for Fragmented {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if !self.interrupted { self.interrupted = true; return Err(io::ErrorKind::Interrupted.into()); }
        let count = output.len().min(3); self.source.read(&mut output[..count])
    }
}
#[test]
fn tied_fragmented_reader_preserves_receipts_budget_and_execution() {
    for stored in [false, true] {
        let bytes = archive(&tensors(stored));
        let (_, expected) = load(&bytes, Some("true")).unwrap();
        let mut source = Fragmented { source: Cursor::new(bytes.clone()), interrupted: false };
        let mut budget = allowance(bytes.len() + 1);
        let (model, receipt) = DecoderModel::read_llama_safetensors(profile().identity(), 8,
            &config(Some("true")), &mut source, &mut budget).unwrap();
        assert_eq!(receipt, expected); compare(&model);
        assert_eq!(budget.usage().bytes_read, bytes.len());
        assert!(budget.usage().read_calls > bytes.len() / 3);
        let mut short = allowance(bytes.len());
        assert_eq!(DecoderModel::read_safetensors_with_output_head(profile(), &mut Cursor::new(&bytes),
            &mut short, OutputHead::TiedEmbeddings).unwrap_err(), WeightReadError::Refused(WeightError::Limit));
    }
}

#[test]
fn tied_readers_require_complete_payload_and_real_eof() {
    let bytes = archive(&tensors(false));
    for length in [0, 7, bytes.len() - 1] {
        let mut budget = allowance(bytes.len() + 1);
        assert!(DecoderModel::read_safetensors_with_output_head(profile(), &mut Cursor::new(&bytes[..length]),
            &mut budget, OutputHead::TiedEmbeddings).is_err());
    }
    let mut extra = bytes.clone(); extra.push(0);
    let mut budget = allowance(extra.len() + 1);
    assert_eq!(DecoderModel::read_safetensors_with_output_head(profile(), &mut Cursor::new(&extra),
        &mut budget, OutputHead::TiedEmbeddings).unwrap_err(), WeightReadError::Refused(WeightError::Header));
    struct LateFailure(Cursor<Vec<u8>>);
    impl Read for LateFailure {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            if self.0.position() as usize == self.0.get_ref().len() { return Err(io::ErrorKind::TimedOut.into()); }
            self.0.read(output)
        }
    }
    let mut budget = allowance(bytes.len() + 1);
    assert_eq!(DecoderModel::read_safetensors_with_output_head(profile(), &mut LateFailure(Cursor::new(bytes.clone())),
        &mut budget, OutputHead::TiedEmbeddings).unwrap_err(),
        WeightReadError::Io { stage: WeightReadStage::EndOfFile, kind: io::ErrorKind::TimedOut });
    assert_eq!(budget.usage().bytes_read, bytes.len());
}

#[test]
fn tied_invalid_configuration_never_enters_the_weight_reader() {
    struct Never;
    impl Read for Never {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> { panic!("bad configuration read weights") }
    }
    let mut budget = allowance(1024);
    assert!(matches!(DecoderModel::read_llama_safetensors(profile().identity(), 8, &config(Some("null")),
        &mut Never, &mut budget), Err(CheckpointError::Configuration { issue: ConfigIssue::Type, .. })));
    assert_eq!(budget.usage().read_calls, 0); assert_eq!(budget.usage().bytes_read, 0);
}

#[test]
fn tied_regular_file_loading_survives_removal_and_keeps_exact_size_limits() {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) { if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("tied weights cleanup: {error}"); } }
    }
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let directory = Directory(std::env::temp_dir().join(format!("fa-tied-{}-{stamp}-{}",
        std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))));
    std::fs::create_dir(&directory.0).unwrap();
    let configuration = config(Some("true")); let bytes = archive(&tensors(false));
    let cfg = directory.0.join("config.json"); let weights = directory.0.join("weights.safetensors");
    std::fs::write(&cfg, &configuration).unwrap(); std::fs::write(&weights, &bytes).unwrap();
    let limits = CheckpointFileLimits { config_bytes: configuration.len(), weight_bytes: bytes.len() };
    assert_eq!(DecoderModel::from_llama_files(profile().identity(), 8, &cfg, &weights,
        CheckpointFileLimits { weight_bytes: bytes.len() - 1, ..limits }).unwrap_err(), CheckpointError::Limit);
    let (model, receipt) = DecoderModel::from_llama_files(profile().identity(), 8, &cfg, &weights, limits).unwrap();
    assert_eq!(receipt.weights.data_bytes, 160); drop(directory); compare(&model);
}
