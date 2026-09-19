//! Synthetic parameters, but real original inference, monitors and tokenization.
//! These fixtures prove plumbing/negative boundaries, not detector calibration.
use super::*;
use crate::action::consequence::activation::monitor::{RefinementBudget, RefinementMonitor};
use crate::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledDecoder;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{ByteBpe, Merge, TokenBytes};
use crate::action::consequence::activation::probe::LinearProbe;
use crate::action::consequence::activation::tensor::kv::decoder::{
    DecoderIdentity, DecoderLayerWeights, DecoderModel, DecoderShape,
};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::{SamplingPolicy, SamplingStart};
use crate::action::consequence::oversight::helper_workers::wire::decode_request;
use std::collections::BTreeMap;

pub(super) fn expected() -> InputProfileBinding {
    InputProfileBinding { profile_id: 7, profile_bytes: b"native categorical fixture".to_vec(),
        model_epoch: 0, tokenizer_epoch: 0, policy_epoch: 0 }
}

/// Independent literal original request frame, not a fabricated WorkerInput.
pub(super) fn frame(prompt: &[u8], profile: &InputProfileBinding) -> Vec<u8> {
    let mut out = b"FAHW1".to_vec(); out.extend_from_slice(&0_u32.to_be_bytes());
    out.extend_from_slice(&9_u64.to_be_bytes()); out.extend_from_slice(&[7; 32]);
    out.extend_from_slice(&6_u16.to_be_bytes()); out.extend_from_slice(b"native");
    out.extend_from_slice(&64_u16.to_be_bytes());
    for number in [profile.profile_id, profile.model_epoch, profile.tokenizer_epoch, profile.policy_epoch] {
        out.extend_from_slice(&number.to_be_bytes());
    }
    for bytes in [profile.profile_bytes.as_slice(), prompt] {
        out.extend_from_slice(&(bytes.len() as u32).to_be_bytes()); out.extend_from_slice(bytes);
    }
    out.extend_from_slice(&1_u16.to_be_bytes());
    out.extend_from_slice(&0_u32.to_be_bytes()); out.extend_from_slice(&(prompt.len() as u32).to_be_bytes());
    out.push(1); // Question part, covering every submitted byte.
    out.extend_from_slice(&1_u16.to_be_bytes());
    out.extend_from_slice(&17_u64.to_be_bytes()); out.extend_from_slice(&22_u64.to_be_bytes());
    let count = (out.len() - 9) as u32; out[5..9].copy_from_slice(&count.to_be_bytes()); out
}
pub(super) fn input(prompt: &[u8]) -> WorkerInput { decode_request(&frame(prompt, &expected())).unwrap() }

// Each emitted spelling is reachable through an independently built merge chain.
fn word(vocabulary: &mut Vec<TokenBytes>, merges: &mut Vec<Merge>, bytes: &[u8]) -> u32 {
    let mut left = u32::from(bytes[0]);
    for end in 2..=bytes.len() {
        let prefix = &bytes[..end];
        if let Some(existing) = vocabulary.iter().position(|item| {
            matches!(item, TokenBytes::Content(value) if value == prefix)
        }) { left = existing as u32; continue; }
        let id = vocabulary.len() as u32;
        vocabulary.push(TokenBytes::Content(prefix.to_vec()));
        merges.push(Merge { left, right: u32::from(bytes[end - 1]), result: id });
        left = id;
    }
    left
}

// alarm=1 holds on the categorical token; alarm=2 holds on the terminal control.
// A '?' prompt selects the requested word; '!' selects a genuinely different
// computed 'deny' token. No caller-supplied verdict enters native evaluation.
pub(super) fn decoder(spelling: &[u8], alarm: u8) -> (TextDecoder, NativeHelperPolicy) {
    let mut vocabulary: Vec<_> = (0..=255).map(|byte| TokenBytes::Content(vec![byte])).collect();
    let mut merges = Vec::new();
    let response = word(&mut vocabulary, &mut merges, spelling);
    let alternate = word(&mut vocabulary, &mut merges, b"deny");
    let stop = vocabulary.len() as u32; vocabulary.push(TokenBytes::Control);
    let count = vocabulary.len();
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 }, DecoderShape { vocabulary: count, hidden: 2,
        intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 1024 }, 0.00001, 10000.0).unwrap();
    let tokenizer = ByteBpe::new(profile.clone(), vocabulary, merges).unwrap();
    let mut embeddings = vec![0.0; count * 2];
    for id in 0..count { embeddings[id * 2] = 1.0; }
    embeddings[usize::from(b'!') * 2] = 0.0;
    embeddings[usize::from(b'!') * 2 + 1] = 1.0;
    for id in [response, alternate] {
        embeddings[id as usize * 2] = -1.0; embeddings[id as usize * 2 + 1] = -1.0;
    }
    if alarm == 2 { embeddings[stop as usize * 2] = -2.0; embeddings[stop as usize * 2 + 1] = -2.0; }
    let mut output = vec![0.0; count * 2];
    output[response as usize * 2] = 10.0;
    output[alternate as usize * 2 + 1] = 10.0;
    output[stop as usize * 2] = -10.0; output[stop as usize * 2 + 1] = -10.0;
    let model = DecoderModel::new(profile.clone(), embeddings, vec![DecoderLayerWeights {
        attention_norm: vec![1.0; 2], queries: vec![0.0; 4], keys: vec![0.0; 4], values: vec![0.0; 4],
        attention_output: vec![0.0; 4], feed_forward_norm: vec![1.0; 2], gate: vec![0.0; 4],
        up: vec![0.0; 4], down: vec![0.0; 4],
    }], vec![1.0; 2], output).unwrap();
    let allowance = RefinementBudget { encoded_bytes: 100_000, probe_coordinates: 100_000 };
    let threshold = match alarm { 1 => 0.5, 2 => 3.0, _ => 100.0 };
    let probe = LinearProbe::new(1, 1, model.residual_contract(1).unwrap().profile(),
        &[-1.0, -1.0], 0.0, threshold).unwrap();
    let monitor = RefinementMonitor::new(vec![probe], vec![23], allowance).unwrap();
    let sampling = SamplingStart { policy: SamplingPolicy::new(1, 1, count, 1.0, 1, 1.0).unwrap(), stream: 10, seed: 11 };
    let native = MonitoredSampledDecoder::new(model, 12, 13, BTreeMap::from([(1, monitor)]), allowance, sampling).unwrap();
    let policy = NativeHelperPolicy { input_profile: expected(), decoder_profile: profile,
        max_new_tokens: 2, stop_tokens: vec![stop], tokenization: TokenizationBudget::default(),
        generation: GenerationBudget { scalar_products: MAX_DECODER_PRODUCTS, sampling_entries: MAX_SAMPLING_ENTRIES },
        max_output_bytes: 128 };
    (TextDecoder::new(native, tokenizer).unwrap(), policy)
}
pub(super) fn evaluator(spelling: &[u8], alarm: u8) -> NativeEvaluator {
    let (decoder, policy) = decoder(spelling, alarm); NativeEvaluator::new(decoder, policy).unwrap()
}

#[test]
fn all_four_verdicts_come_from_original_inference_with_exact_prompt_and_metadata() {
    for (word, verdict) in [(b"allow".as_slice(), Verdict::Allow), (b"hold", Verdict::Hold),
        (b"deny", Verdict::Deny), (b"abstain", Verdict::Abstain)] {
        let mut worker = evaluator(word, 0);
        let original = input(b"\x00raw\xff\n?");
        assert_eq!(worker.evaluate(&original), Ok(verdict));
        assert_eq!(worker.status(), NativeEvaluationStatus::Judged(verdict));
        assert_eq!(worker.input(), Some(&original));
        let report = worker.report().unwrap();
        assert_eq!(report.prompt().source(), original.actual_input().submitted_bytes());
        assert_eq!(report.bytes().unwrap(), word);
        assert!(report.prefix_controls().is_empty());
        assert_eq!(report.generation().finish(), GenerationFinish::StopToken);
        assert_eq!(report.generation().reviewed_prompt_tokens(), report.generation().requested_prompt_tokens());
        assert_eq!(worker.sampled_draws(), 2);
    }
}

#[test]
fn changing_actual_model_input_changes_the_computed_verdict() {
    let mut allow = evaluator(b"allow", 0); let mut deny = evaluator(b"allow", 0);
    assert_eq!(allow.evaluate(&input(b"?")), Ok(Verdict::Allow));
    assert_eq!(deny.evaluate(&input(b"!")), Ok(Verdict::Deny));
    assert_ne!(allow.report().unwrap().prompt().tokens(), deny.report().unwrap().prompt().tokens());
}

#[test]
fn wrong_input_profile_or_model_profile_cannot_run_native_work() {
    let mut worker = evaluator(b"allow", 0);
    let mut profile = expected(); profile.policy_epoch += 1;
    let wrong = decode_request(&frame(b"?", &profile)).unwrap();
    assert_eq!(worker.evaluate(&wrong), Err(NativeEvaluationError::Contract(Error::Binding)));
    assert_eq!(worker.position(), 0); assert_eq!(worker.sampled_draws(), 0);
    assert!(worker.report().is_none());
    assert_eq!(worker.evaluate(&input(b"?")), Err(NativeEvaluationError::Contract(Error::WrongState)));
    let (decoder, mut policy) = decoder(b"allow", 0);
    let p = decoder.profile(); let mut id = p.identity(); id.tokenizer_generation += 1;
    policy.decoder_profile = DecoderProfile::new(id, p.shape(), p.epsilon(), p.theta()).unwrap();
    assert_eq!(NativeEvaluator::new(decoder, policy).unwrap_err(), Error::Binding);
}

#[test]
fn held_answer_never_becomes_a_vote_or_a_rerolled_sample() {
    let mut worker = evaluator(b"allow", 1);
    assert_eq!(worker.evaluate(&input(b"?")), Err(NativeEvaluationError::Incomplete(GenerationFinish::Held)));
    assert_eq!(worker.sampled_draws(), 1);
    assert_eq!(worker.report().unwrap().bytes().unwrap(), b"");
    let position = worker.position();
    assert!(worker.evaluate(&input(b"!" )).is_err());
    assert_eq!(worker.position(), position); assert_eq!(worker.sampled_draws(), 1);
}

#[test]
fn quiet_answer_followed_by_held_stop_is_still_not_a_complete_vote() {
    let mut worker = evaluator(b"allow", 2);
    assert_eq!(worker.evaluate(&input(b"?")), Err(NativeEvaluationError::Incomplete(GenerationFinish::Held)));
    assert_eq!(worker.report().unwrap().bytes().unwrap(), b"allow");
    assert_eq!(worker.sampled_draws(), 2);
}

#[test]
fn token_limit_and_sampling_exhaustion_cannot_promote_a_partial_word_or_answer() {
    for limited in [false, true] {
        let (decoder, mut policy) = decoder(b"allow", 0);
        let finish = if limited {
            policy.generation.sampling_entries = decoder.profile().shape().vocabulary as u64;
            GenerationFinish::BudgetExhausted
        } else { policy.max_new_tokens = 1; GenerationFinish::TokenLimit };
        let mut worker = NativeEvaluator::new(decoder, policy).unwrap();
        assert_eq!(worker.evaluate(&input(b"?")), Err(NativeEvaluationError::Incomplete(finish)));
        assert_eq!(worker.report().unwrap().bytes().unwrap(), b"allow");
        assert_eq!(worker.sampled_draws(), 1);
    }
}

#[test]
fn schema_never_trims_repairs_or_extracts_an_allow_from_prose() {
    for bytes in [b"allow\n".as_slice(), b"allow deny", b"ALLOW", b" allow", b"", b"allow\xff"] {
        assert_eq!(parse_verdict(bytes), Err(NativeEvaluationError::InvalidVerdict));
    }
    let mut worker = evaluator(b"allow\n", 0);
    assert_eq!(worker.evaluate(&input(b"?")), Err(NativeEvaluationError::InvalidVerdict));
    assert_eq!(worker.report().unwrap().bytes().unwrap(), b"allow\n");
    assert_eq!(worker.sampled_draws(), 2);
}

#[test]
fn full_prompt_admission_refuses_without_truncation_or_a_default_vote() {
    for too_long in [false, true] {
        let (decoder, mut policy) = decoder(b"allow", 0);
        if !too_long { policy.tokenization.input_bytes = 1; }
        let mut worker = NativeEvaluator::new(decoder, policy).unwrap();
        let prompt = if too_long { vec![b'?'; 1024] } else { b"??".to_vec() };
        assert!(matches!(worker.evaluate(&input(&prompt)), Err(NativeEvaluationError::Admission(_))));
        assert_eq!(worker.position(), 0); assert_eq!(worker.sampled_draws(), 0);
        assert!(worker.report().is_none());
    }
}

#[test]
fn content_stop_and_duplicate_control_cannot_hide_unexamined_suffixes() {
    for mode in 0..3 {
        let (decoder, mut policy) = decoder(b"allow", 0);
        let error = match mode {
            0 => { policy.stop_tokens = vec![u32::from(b'a')]; Error::Binding }
            1 => { policy.stop_tokens.push(policy.stop_tokens[0]); Error::Duplicate }
            _ => { policy.max_output_bytes = 1; Error::Limit }
        };
        assert_eq!(NativeEvaluator::new(decoder, policy).unwrap_err(), error);
    }
}

#[test]
fn success_and_interrupted_evaluation_have_no_second_inference_opportunity() {
    let mut worker = evaluator(b"allow", 0);
    assert_eq!(worker.evaluate(&input(b"?")), Ok(Verdict::Allow));
    let position = worker.position();
    assert_eq!(worker.evaluate(&input(b"!")), Err(NativeEvaluationError::Contract(Error::WrongState)));
    assert_eq!(worker.position(), position); assert_eq!(worker.sampled_draws(), 2);
    let mut interrupted = evaluator(b"allow", 0);
    interrupted.status = NativeEvaluationStatus::Evaluating;
    assert_eq!(interrupted.evaluate(&input(b"?")), Err(NativeEvaluationError::Contract(Error::WrongState)));
    assert_eq!(interrupted.position(), 0);
}
