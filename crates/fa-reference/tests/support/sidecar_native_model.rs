//! Synthetic categorical model executed by the ORIGINAL native helper stack.
//! Its actual final prompt byte changes the answer; no supplied verdict enters.
use fa_reference::action::consequence::activation::monitor::{RefinementBudget, RefinementMonitor};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::{MonitoredSampledDecoder,
    generation::{GenerationBudget, MAX_SAMPLING_ENTRIES,
        text::TextDecoder, tokenizer::{ByteBpe, Merge, TokenBytes, TokenizationBudget}}};
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderIdentity,
    DecoderLayerWeights, DecoderModel, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS,
    sampling::{SamplingPolicy, SamplingStart}};
use fa_reference::action::consequence::oversight::helper_client::native::{NativeEvaluator, NativeHelperPolicy};
use fa_reference::full_input::InputProfileBinding;
use std::collections::BTreeMap;

fn word(vocabulary: &mut Vec<TokenBytes>, merges: &mut Vec<Merge>, bytes: &[u8]) -> u32 {
    let mut left = u32::from(bytes[0]);
    for end in 2..=bytes.len() {
        let prefix = &bytes[..end];
        if let Some(existing) = vocabulary.iter().position(|item| matches!(item,
            TokenBytes::Content(value) if value == prefix)) { left = existing as u32; continue; }
        let id = vocabulary.len() as u32;
        vocabulary.push(TokenBytes::Content(prefix.to_vec()));
        merges.push(Merge { left, right: u32::from(bytes[end - 1]), result: id });
        left = id;
    }
    left
}
pub fn helper(profile: InputProfileBinding, spelling: &[u8], alarm: bool) -> NativeEvaluator {
    let mut vocabulary: Vec<_> = (0..=255).map(|byte| TokenBytes::Content(vec![byte])).collect();
    let mut merges = Vec::new();
    let answer = word(&mut vocabulary, &mut merges, spelling);
    let deny = word(&mut vocabulary, &mut merges, b"deny");
    let stop = vocabulary.len() as u32; vocabulary.push(TokenBytes::Control);
    let count = vocabulary.len();
    let decoder_profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2,
        model_generation: 3, tokenizer_generation: 4, profile_generation: 5 }, DecoderShape {
        vocabulary: count, hidden: 2, intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1,
        context: 4096 }, 0.00001, 10000.0).unwrap();
    let tokenizer = ByteBpe::new(decoder_profile.clone(), vocabulary, merges).unwrap();
    let mut embeddings = vec![0.0; count * 2];
    for id in 0..count { embeddings[id * 2] = 1.0; }
    embeddings[usize::from(b'!') * 2] = 0.0;
    embeddings[usize::from(b'!') * 2 + 1] = 1.0;
    for id in [answer, deny] {
        embeddings[id as usize * 2] = -1.0; embeddings[id as usize * 2 + 1] = -1.0;
    }
    let mut output = vec![0.0; count * 2];
    output[answer as usize * 2] = 10.0;
    output[deny as usize * 2 + 1] = 10.0;
    output[stop as usize * 2] = -10.0; output[stop as usize * 2 + 1] = -10.0;
    let model = DecoderModel::new(decoder_profile.clone(), embeddings, vec![DecoderLayerWeights {
        attention_norm: vec![1.0; 2], queries: vec![0.0; 4], keys: vec![0.0; 4], values: vec![0.0; 4],
        attention_output: vec![0.0; 4], feed_forward_norm: vec![1.0; 2], gate: vec![0.0; 4],
        up: vec![0.0; 4], down: vec![0.0; 4],
    }], vec![1.0; 2], output).unwrap();
    let budget = RefinementBudget { encoded_bytes: 1_000_000, probe_coordinates: 1_000_000 };
    let probe = LinearProbe::new(1, 1, model.residual_contract(1).unwrap().profile(),
        &[-1.0, -1.0], 0.0, if alarm { 0.5 } else { 100.0 }).unwrap();
    let monitor = RefinementMonitor::new(vec![probe], vec![23], budget).unwrap();
    let run = MonitoredSampledDecoder::new(model, 12, 13, BTreeMap::from([(1, monitor)]), budget,
        SamplingStart { policy: SamplingPolicy::new(1, 1, count, 1.0, 1, 1.0).unwrap(), stream: 10, seed: 11 }).unwrap();
    NativeEvaluator::new(TextDecoder::new(run, tokenizer).unwrap(), NativeHelperPolicy {
        input_profile: profile, decoder_profile, max_new_tokens: 2, stop_tokens: vec![stop],
        tokenization: TokenizationBudget::default(), generation: GenerationBudget {
            scalar_products: MAX_DECODER_PRODUCTS, sampling_entries: MAX_SAMPLING_ENTRIES }, max_output_bytes: 128,
    }).unwrap()
}
