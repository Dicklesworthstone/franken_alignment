use super::*;
use crate::action::consequence::activation::monitor::{RefinementBudget, RefinementMonitor};
use crate::action::consequence::activation::monitor::decoder::MonitoringStatus;
use crate::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledDecoder;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationBudget, GenerationFinish, GenerationRequest, MAX_SAMPLING_ENTRIES,
};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::text::{TextDecoder, TextGenerationRequest};
use crate::action::consequence::activation::probe::LinearProbe;
use crate::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderLayerWeights, DecoderModel, MAX_DECODER_PRODUCTS,
};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::{SamplingPolicy, SamplingStart};
use std::io::{self, Cursor, Read};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-hf-text-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("HF text fixture cleanup: {error}"); }
    }
}

// Deterministic fixture parameters, but the ORIGINAL full decoder, all-layer
// capture, residual probe and sampler execute. No callback asserts quietness.
fn numerical(output_token: u32, alarm: Option<u32>) -> MonitoredSampledDecoder {
    let p = profile();
    let vocabulary = p.shape().vocabulary;
    let mut embeddings: Vec<f32> = (0..vocabulary).flat_map(|_| [1.0, 0.0]).collect();
    if let Some(token) = alarm { embeddings[token as usize * 2] = 3.0; }
    let mut output = vec![0.0; 2 * vocabulary];
    output[output_token as usize * 2] = 1.0;
    let model = DecoderModel::new(p, embeddings, vec![DecoderLayerWeights {
        attention_norm: vec![1.0; 2], queries: vec![0.0; 4], keys: vec![0.0; 4], values: vec![0.0; 4],
        attention_output: vec![0.0; 4], feed_forward_norm: vec![1.0; 2],
        gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4],
    }], vec![1.0; 2], output).unwrap();
    let allowance = RefinementBudget { encoded_bytes: 100_000, probe_coordinates: 100_000 };
    let probe = LinearProbe::new(1, 1, model.residual_contract(1).unwrap().profile(), &[1.0, 0.0], 0.0, 2.0).unwrap();
    let monitors = BTreeMap::from([(1, RefinementMonitor::new(vec![probe], vec![23], allowance).unwrap())]);
    MonitoredSampledDecoder::new(model, 7, 11, monitors, allowance, SamplingStart {
        policy: SamplingPolicy::new(1, 1, vocabulary, 1.0, 1, 1.0).unwrap(), stream: 9, seed: 42,
    }).unwrap()
}
fn request(prompt: &[u8], new: usize) -> TextGenerationRequest {
    TextGenerationRequest { prompt: prompt.to_vec(), prefix_controls: vec![], max_new_tokens: new,
        stop_tokens: vec![], tokenization: TokenizationBudget::default(),
        generation: GenerationBudget { scalar_products: MAX_DECODER_PRODUCTS, sampling_entries: MAX_SAMPLING_ENTRIES },
        max_output_bytes: 4 * new }
}
fn text(output: u32, alarm: Option<u32>) -> TextDecoder {
    let json = document(false);
    TextDecoder::from_huggingface_reader(numerical(output, alarm), &mut Cursor::new(json.as_bytes()), json.len()).unwrap()
}

#[test]
fn hf_real_file_import_drives_original_monitored_generation_with_original_ids() {
    let root = Directory::new();
    let path = root.0.join("tokenizer.json");
    let json = document(false);
    std::fs::write(&path, json.as_bytes()).unwrap();
    let mut file = std::fs::File::open(&path).unwrap();
    let mut run = TextDecoder::from_huggingface_reader(numerical(id(b'A'), None), &mut file, json.len()).unwrap();
    drop(file);
    std::fs::remove_file(&path).unwrap();
    assert_eq!(run.position(), 0);
    assert_eq!(run.sampled_draws(), 0);
    assert_eq!(run.monitoring_work().frame_reviews, 0);
    let input = request(" abc é".as_bytes(), 3);
    let mut native = numerical(id(b'A'), None);
    // These expected prompt IDs are manual, not produced by the importer.
    let expected = native.generate(0, GenerationRequest { prompt: vec![260, 49, 261], max_new_tokens: 3,
        stop_tokens: vec![], budget: input.generation }).unwrap();
    let report = run.generate(0, input).unwrap();
    assert_eq!(report.prompt().tokens(), &[260, 49, 261]);
    assert_eq!(report.prompt().source(), " abc é".as_bytes());
    assert_eq!(report.bytes().unwrap(), b"AAA");
    assert_eq!(report.generation().tokens(), expected.tokens());
    assert_eq!(report.generation().work(), expected.work());
    assert_eq!(report.generation().finish(), GenerationFinish::TokenLimit);
    assert_eq!(run.decoder_work(), native.decoder_work());
    assert_eq!(run.monitoring_work(), native.monitoring_work());
    assert_eq!(run.sampled_draws(), 3);
    assert_eq!(run.position(), 6);
    let archive = run.tokenizer().to_bytes().unwrap();
    let retained = ByteBpe::from_bytes(&profile(), &archive).unwrap();
    assert_eq!(encode(&retained, b" abc"), vec![260]);
}

#[test]
fn hf_import_cannot_release_a_held_sample_or_reroll_after_a_hold() {
    let mut quiet = text(id(b'!'), None);
    assert_eq!(quiet.generate(0, request(b"abc", 2)).unwrap().bytes().unwrap(), b"!!");
    let mut held = text(id(b'!'), Some(id(b'!')));
    let observation = held.observation();
    let report = held.generate(0, request(b"abc", 2)).unwrap();
    assert_eq!(report.generation().finish(), GenerationFinish::Held);
    assert!(report.generation().tokens().is_empty());
    assert!(report.bytes().unwrap().is_empty());
    assert_eq!(held.sampled_draws(), 1);
    assert_eq!(held.status(), MonitoringStatus::Held);
    assert!(observation.capture().is_err());
    let before = held.decoder_work();
    assert_eq!(held.generate(held.position(), request(b"abc", 1)).unwrap_err().error, Error::WrongState);
    assert_eq!(held.sampled_draws(), 1);
    assert_eq!(held.decoder_work(), before);
}

#[test]
fn hf_admission_does_not_truncate_over_context_or_underfunded_prompt() {
    let mut run = text(261, None);
    let mut input = request(b"abc", 1);
    input.generation.scalar_products = 0;
    assert_eq!(run.generate(0, input).unwrap_err().error, Error::Limit);
    assert_eq!(run.position(), 0);
    assert_eq!(run.sampled_draws(), 0);
    assert_eq!(run.monitoring_work().frame_reviews, 0);
    assert_eq!(run.generate(0, request(&b"abc".repeat(33), 0)).unwrap_err().error, Error::Limit);
    assert_eq!(run.position(), 0);
    let accepted = run.generate(0, request(&b"abc".repeat(32), 0)).unwrap();
    assert_eq!(accepted.generation().requested_prompt_tokens(), 32);
    assert_eq!(accepted.generation().reviewed_prompt_tokens(), 32);
    assert_eq!(run.position(), 32);
    assert_eq!(run.sampled_draws(), 0);
}

#[test]
fn hf_constructor_refuses_existing_or_held_history_before_reader_io() {
    struct Uncalled;
    impl Read for Uncalled {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> { panic!("no reader call for an ineligible owner") }
    }
    let json = document(false);
    let mut fresh = text(id(b'A'), None);
    assert_eq!(fresh.generate(0, request(b"abc", 1)).unwrap().bytes().unwrap(), b"A");
    for alarm in [None, Some(id(b'!'))] {
        let mut decoder = numerical(id(b'A'), alarm);
        decoder.advance_forced(0, id(b'!'), DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap();
        assert_eq!(TextDecoder::from_huggingface_reader(decoder, &mut Uncalled, json.len()).unwrap_err().kind(),
            io::ErrorKind::InvalidInput);
    }
}
