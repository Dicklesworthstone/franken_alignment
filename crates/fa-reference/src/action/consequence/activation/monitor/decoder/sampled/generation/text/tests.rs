use super::*;
use super::super::{GenerationFinish, MAX_SAMPLING_ENTRIES};
use super::super::tokenizer::{Merge, TokenBytes};
use crate::action::consequence::activation::monitor::{RefinementBudget, RefinementMonitor};
use crate::action::consequence::activation::probe::LinearProbe;
use crate::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderIdentity, DecoderLayerWeights, DecoderModel, DecoderShape, MAX_DECODER_PRODUCTS,
};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::{SamplingPolicy, SamplingStart};
use std::collections::BTreeMap;

pub(super) fn profile() -> DecoderProfile {
    DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 }, DecoderShape { vocabulary: 260,
        hidden: 2, intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 32 },
        1e-5, 10000.0).unwrap()
}
pub(super) fn vocabulary() -> Vec<TokenBytes> {
    let mut tokens: Vec<_> = (0..=255).map(|byte| TokenBytes::Content(vec![byte])).collect();
    tokens.extend([TokenBytes::Control, TokenBytes::Control,
        TokenBytes::Content(b"ab".to_vec()), TokenBytes::Content("é".as_bytes().to_vec())]);
    tokens
}
pub(super) fn tokenizer() -> ByteBpe {
    ByteBpe::new(profile(), vocabulary(), vec![
        Merge { left: 97, right: 98, result: 258 },
        Merge { left: 0xc3, right: 0xa9, result: 259 },
    ]).unwrap()
}

// Actual native decoder, original exact residual probe and original sampler.
// Zero projection/MLP weights preserve the input embedding at each residual.
// A selected token's own embedding (not a fabricated review) causes any alarm.
pub(super) fn numerical(output_token: u32, alarm: Option<u32>, chain_to_alarm: bool) -> MonitoredSampledDecoder {
    let p = profile();
    let mut embeddings: Vec<f32> = (0..p.shape().vocabulary).flat_map(|_| [1.0, 0.0]).collect();
    if let Some(token) = alarm { embeddings[token as usize * 2] = 3.0; }
    let mut output = vec![0.0; 2 * p.shape().vocabulary];
    output[output_token as usize * 2] = 1.0;
    if chain_to_alarm {
        embeddings[output_token as usize * 2] = 0.0;
        embeddings[output_token as usize * 2 + 1] = 1.0;
        output[alarm.unwrap() as usize * 2 + 1] = 1.0;
    }
    let model = DecoderModel::new(p, embeddings, vec![DecoderLayerWeights {
        attention_norm: vec![1.0; 2], queries: vec![0.0; 4], keys: vec![0.0; 4], values: vec![0.0; 4],
        attention_output: vec![0.0; 4], feed_forward_norm: vec![1.0; 2],
        gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4],
    }], vec![1.0; 2], output).unwrap();
    let allowance = RefinementBudget { encoded_bytes: 100_000, probe_coordinates: 100_000 };
    let probe = LinearProbe::new(1, 1, model.residual_contract(1).unwrap().profile(), &[1.0, 0.0], 0.0, 2.0).unwrap();
    let monitors = BTreeMap::from([(1, RefinementMonitor::new(vec![probe], vec![23], allowance).unwrap())]);
    MonitoredSampledDecoder::new(model, 7, 11, monitors, allowance, SamplingStart {
        policy: SamplingPolicy::new(1, 1, 260, 1.0, 1, 1.0).unwrap(), stream: 9, seed: 42,
    }).unwrap()
}
pub(super) fn run(output: u32, alarm: Option<u32>) -> TextDecoder {
    TextDecoder::new(numerical(output, alarm, false), tokenizer()).unwrap()
}
pub(super) fn request(prompt: &[u8], new: usize) -> TextGenerationRequest {
    TextGenerationRequest { prompt: prompt.to_vec(), prefix_controls: vec![256],
        max_new_tokens: new, stop_tokens: vec![256, 257], tokenization: TokenizationBudget::default(),
        generation: GenerationBudget { scalar_products: MAX_DECODER_PRODUCTS, sampling_entries: MAX_SAMPLING_ENTRIES },
        max_output_bytes: 2 * new }
}

#[test]
fn text_ingestion_matches_native_original_ids_work_and_reviewed_generation() {
    let mut text = run(65, None); let mut native = numerical(65, None, false);
    let input = request(b"ab", 3);
    let expected = native.generate(0, GenerationRequest { prompt: vec![256, 258], max_new_tokens: 3,
        stop_tokens: input.stop_tokens.clone(), budget: input.generation }).unwrap();
    let report = text.generate(0, input).unwrap();
    assert_eq!(report.prompt().source(), b"ab");
    assert_eq!(report.prompt().tokens(), &[258]);
    assert_eq!(report.prompt().spans(), &[0..2]);
    assert_eq!(report.prefix_controls(), &[256]);
    assert_eq!(report.generation().requested_prompt_tokens(), 2);
    assert_eq!(report.generation().reviewed_prompt_tokens(), 2);
    assert_eq!(report.generation().tokens(), expected.tokens());
    assert_eq!(report.generation().work(), expected.work());
    assert_eq!(report.generation().finish(), GenerationFinish::TokenLimit);
    assert_eq!(report.bytes().unwrap(), b"AAA");
    assert_eq!(report.utf8().unwrap(), "AAA");
    assert_eq!(text.position(), 5);
    assert_eq!(text.decoder_work(), native.decoder_work());
    assert_eq!(text.monitoring_work(), native.monitoring_work());
    assert_eq!(text.sampled_draws(), 3);
}

#[test]
fn sampled_control_is_reviewed_and_consumed_before_being_omitted() {
    let mut text = run(257, None);
    let report = text.generate(0, request(b"<eos>", 3)).unwrap();
    assert_eq!(report.prompt().source(), b"<eos>");
    assert_eq!(report.prompt().tokens().len(), 5); // no special-spelling recognition
    assert_eq!(report.generation().reviewed_prompt_tokens(), 6);
    assert_eq!(report.generation().finish(), GenerationFinish::StopToken);
    assert_eq!(report.bytes().unwrap(), &[]);
    assert!(report.generation().tokens().is_empty());
    assert_eq!(text.sampled_draws(), 1);
    assert_eq!(text.position(), 7);
}

#[test]
fn a_held_generated_token_never_enters_decoded_bytes_or_a_second_draw() {
    let mut text = run(33, Some(33));
    let observation = text.observation();
    let report = text.generate(0, request(b"ab", 3)).unwrap();
    assert_eq!(report.generation().finish(), GenerationFinish::Held);
    assert!(report.bytes().unwrap().is_empty());
    assert!(report.generation().tokens().is_empty());
    assert_eq!(text.sampled_draws(), 1);
    assert_eq!(text.status(), MonitoringStatus::Held);
    assert!(observation.capture().is_err());
    let before = text.decoder_work();
    assert_eq!(text.generate(text.position(), request(b"ab", 1)).unwrap_err().error, Error::WrongState);
    assert_eq!(text.decoder_work(), before);
    assert_eq!(text.sampled_draws(), 1);
}

#[test]
fn a_later_hold_preserves_only_the_previously_released_quiet_prefix() {
    let decoder = numerical(66, Some(33), true);
    let mut text = TextDecoder::new(decoder, tokenizer()).unwrap();
    let report = text.generate(0, request(b"ab", 3)).unwrap();
    assert_eq!(report.generation().finish(), GenerationFinish::Held);
    assert_eq!(report.generation().tokens(), &[66]);
    assert_eq!(report.bytes().unwrap(), b"B");
    assert_eq!(text.sampled_draws(), 2); // the withheld alarm still consumed its draw
    assert_eq!(text.position(), 4);
    assert_eq!(text.status(), MonitoringStatus::Held);
}

#[test]
fn prompt_alarm_keeps_the_whole_encoded_input_but_never_starts_sampling() {
    let mut text = run(65, Some(33));
    let report = text.generate(0, request(b"!ab", 2)).unwrap();
    assert_eq!(report.prompt().source(), b"!ab");
    assert_eq!(report.prompt().tokens(), &[33, 258]);
    assert_eq!(report.generation().requested_prompt_tokens(), 3);
    assert_eq!(report.generation().reviewed_prompt_tokens(), 1); // only explicit BOS
    assert_eq!(report.generation().finish(), GenerationFinish::Held);
    assert!(report.bytes().unwrap().is_empty());
    assert_eq!(text.sampled_draws(), 0);
}

#[test]
fn invalid_utf8_is_retained_exactly_and_valid_multibyte_tokens_decode_exactly() {
    let mut invalid = run(255, None);
    let report = invalid.generate(0, request(b"ab", 2)).unwrap();
    assert_eq!(report.bytes().unwrap(), &[255, 255]);
    assert!(matches!(report.utf8(), Err(TextOutputError::InvalidUtf8(_))));
    assert_eq!(report.generation().tokens(), &[255, 255]);
    let mut valid = run(259, None);
    let report = valid.generate(0, request("é".as_bytes(), 2)).unwrap();
    assert_eq!(report.prompt().tokens(), &[259]);
    assert_eq!(report.bytes().unwrap(), "éé".as_bytes());
    assert_eq!(report.utf8().unwrap(), "éé");
}

#[test]
fn invalid_controls_stops_and_output_capacity_refuse_before_any_inference() {
    let mut text = run(65, None);
    for case in 0..5 {
        let mut input = request(b"ab", 3);
        let expected = match case {
            0 => { input.prefix_controls = vec![97]; Error::Binding }
            1 => { input.stop_tokens = vec![257]; Error::Incomplete }
            2 => { input.stop_tokens = vec![256, 257, 257]; Error::Duplicate }
            3 => { input.max_output_bytes = 5; Error::Limit }
            4 => { input.stop_tokens.push(260); Error::Missing }
            _ => unreachable!(),
        };
        let failure = text.generate(0, input).unwrap_err();
        assert_eq!(failure.error, expected);
        assert_eq!(failure.tokenization, TokenizationWork::default());
        assert_eq!(text.position(), 0);
        assert_eq!(text.sampled_draws(), 0);
    }
    assert_eq!(text.generate(0, request(b"ab", 3)).unwrap().bytes().unwrap(), b"AAA");
}

#[test]
fn encoding_numerical_budget_and_stale_position_failures_do_not_advance_history() {
    let mut text = run(65, None);
    assert_eq!(text.generate(1, request(b"ab", 1)).unwrap_err().error, Error::Stale);
    let mut input = request(b"ab", 1); input.tokenization.pair_lookups = 0;
    let failure = text.generate(0, input).unwrap_err();
    assert_eq!(failure.error, Error::Limit);
    assert_eq!(failure.tokenization.input_bytes, 2);
    assert_eq!(failure.tokenization.pair_lookups, 0);
    let mut input = request(b"ab", 1); input.generation.scalar_products = 0;
    let failure = text.generate(0, input).unwrap_err();
    assert_eq!(failure.error, Error::Limit);
    assert_eq!(failure.tokenization.merges, 1);
    assert_eq!(text.position(), 0);
    assert_eq!(text.decoder_work().tokens, 0);
    assert_eq!(text.monitoring_work().frame_reviews, 0);
    assert_eq!(text.sampled_draws(), 0);
    assert!(text.generate(0, request(b"ab", 1)).is_ok());
}

#[test]
fn context_admission_uses_the_complete_encoded_prompt_not_a_truncated_prefix() {
    let mut text = run(65, None);
    // 32 merged content IDs plus explicit BOS exceed the 32-position context.
    let failure = text.generate(0, request(&b"ab".repeat(32), 0)).unwrap_err();
    assert_eq!(failure.error, Error::Limit);
    assert_eq!(failure.tokenization.input_bytes, 64);
    assert_eq!(text.position(), 0);
    let report = text.generate(0, request(&b"ab".repeat(31), 0)).unwrap();
    assert_eq!(report.generation().requested_prompt_tokens(), 32);
    assert_eq!(report.generation().reviewed_prompt_tokens(), 32);
    assert_eq!(report.generation().finish(), GenerationFinish::TokenLimit);
    assert_eq!(text.position(), 32);
    assert_eq!(text.sampled_draws(), 0);
}

#[test]
fn generation_budget_exhaustion_is_an_original_partial_report_not_an_empty_success() {
    let mut text = run(65, None);
    let mut input = request(b"ab", 3); input.generation.sampling_entries = 260;
    let report = text.generate(0, input).unwrap();
    assert_eq!(report.generation().finish(), GenerationFinish::BudgetExhausted);
    assert_eq!(report.bytes().unwrap(), b"A");
    assert_eq!(report.generation().work().attempted_samples, 1);
    assert_eq!(text.sampled_draws(), 1);
    assert_eq!(text.position(), 3);
}

#[test]
fn continuation_retains_original_kv_history_without_text_roundtrips_or_new_bos() {
    let mut text = run(65, None);
    text.generate(0, request(b"ab", 1)).unwrap();
    let mut continuation = request(&[], 2); continuation.prefix_controls.clear();
    let report = text.generate(3, continuation).unwrap();
    assert!(report.prompt().source().is_empty());
    assert!(report.prompt().tokens().is_empty());
    assert!(report.prefix_controls().is_empty());
    assert_eq!(report.generation().requested_prompt_tokens(), 0);
    assert_eq!(report.bytes().unwrap(), b"AA");
    assert_eq!(text.position(), 5);
    assert_eq!(text.decoder_work().tokens, 5);
    assert_eq!(text.sampled_draws(), 3);
}

#[test]
fn profile_mismatch_or_an_already_advanced_numerical_owner_cannot_attach() {
    let original = profile(); let mut identity = original.identity(); identity.tokenizer_generation += 1;
    let foreign = DecoderProfile::new(identity, original.shape(), original.epsilon(), original.theta()).unwrap();
    let bpe = tokenizer();
    let wrong = ByteBpe::new(foreign, vocabulary(), vec![
        Merge { left: 97, right: 98, result: 258 }, Merge { left: 0xc3, right: 0xa9, result: 259 },
    ]).unwrap();
    assert!(matches!(TextDecoder::new(numerical(65, None, false), wrong), Err(Error::Binding)));
    let mut advanced = numerical(65, None, false);
    advanced.advance_forced(0, 97, DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap();
    assert!(matches!(TextDecoder::new(advanced, bpe.clone()), Err(Error::WrongState)));
    assert!(TextDecoder::new(numerical(65, None, false), bpe).is_ok());
}
