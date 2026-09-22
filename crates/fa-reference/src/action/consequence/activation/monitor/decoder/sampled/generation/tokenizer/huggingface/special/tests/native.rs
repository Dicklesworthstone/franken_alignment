//! Synthetic weights, real native helper inference and original verdict parsing.
use super::*;
use crate::action::consequence::activation::monitor::{RefinementBudget, RefinementMonitor};
use crate::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledDecoder;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationBudget, GenerationFinish, MAX_SAMPLING_ENTRIES,
};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::text::TextDecoder;
use crate::action::consequence::activation::probe::LinearProbe;
use crate::action::consequence::activation::tensor::kv::decoder::{
    DecoderLayerWeights, DecoderModel, MAX_DECODER_PRODUCTS,
};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::{SamplingPolicy, SamplingStart};
use crate::action::consequence::oversight::helper_client::native::{
    NativeEvaluator, NativeHelperPolicy, NativeEvaluationError, NativeEvaluationStatus,
};
use crate::action::consequence::oversight::helper_workers::wire::{WorkerInput, decode_request};
use crate::full_input::InputProfileBinding;
use crate::round::Verdict;

fn expected() -> InputProfileBinding {
    InputProfileBinding { profile_id: 7, profile_bytes: b"imported named-control fixture".to_vec(),
        model_epoch: 0, tokenizer_epoch: 0, policy_epoch: 0 }
}

// Independent original wire frame; do not fabricate a private WorkerInput.
fn input(prompt: &[u8]) -> WorkerInput {
    let profile = expected();
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
    out.push(1); // Exact Question part, not an invented native judgment.
    out.extend_from_slice(&1_u16.to_be_bytes());
    out.extend_from_slice(&17_u64.to_be_bytes()); out.extend_from_slice(&22_u64.to_be_bytes());
    let count = (out.len() - 9) as u32; out[5..9].copy_from_slice(&count.to_be_bytes());
    decode_request(&out).unwrap()
}

// '?' computes 'allow', '!' computes 'deny'. Both answer embeddings then choose
// EOS. An alarm changes actual residuals/probe thresholds, not an asserted vote.
fn numerical(alarm: u8) -> (MonitoredSampledDecoder, NativeHelperPolicy) {
    let p = profile(264); let count = p.shape().vocabulary;
    let mut embeddings = vec![0.0; count * 2];
    for id in 0..count { embeddings[id * 2] = 1.0; }
    embeddings[byte_id(b'!') as usize * 2] = 0.0;
    embeddings[byte_id(b'!') as usize * 2 + 1] = 1.0;
    for id in [259, 262] { embeddings[id * 2] = -1.0; embeddings[id * 2 + 1] = -1.0; }
    if alarm == 2 { embeddings[263 * 2] = -2.0; embeddings[263 * 2 + 1] = -2.0; }
    let mut output = vec![0.0; count * 2];
    output[259 * 2] = 10.0; output[262 * 2 + 1] = 10.0;
    output[263 * 2] = -10.0; output[263 * 2 + 1] = -10.0;
    let model = DecoderModel::new(p.clone(), embeddings, vec![DecoderLayerWeights {
        attention_norm: vec![1.0; 2], queries: vec![0.0; 4], keys: vec![0.0; 4], values: vec![0.0; 4],
        attention_output: vec![0.0; 4], feed_forward_norm: vec![1.0; 2], gate: vec![0.0; 4],
        up: vec![0.0; 4], down: vec![0.0; 4],
    }], vec![1.0; 2], output).unwrap();
    let allowance = RefinementBudget { encoded_bytes: 100_000, probe_coordinates: 100_000 };
    let threshold = match alarm { 1 => 0.5, 2 => 3.0, _ => 100.0 };
    let probe = LinearProbe::new(1, 1, model.residual_contract(1).unwrap().profile(),
        &[-1.0, -1.0], 0.0, threshold).unwrap();
    let monitor = RefinementMonitor::new(vec![probe], vec![23], allowance).unwrap();
    let native = MonitoredSampledDecoder::new(model, 12, 13,
        BTreeMap::from([(1, monitor)]), allowance, SamplingStart {
            policy: SamplingPolicy::new(1, 1, count, 1.0, 1, 1.0).unwrap(), stream: 10, seed: 11,
        }).unwrap();
    let policy = NativeHelperPolicy { input_profile: expected(), decoder_profile: p,
        max_new_tokens: 2, stop_tokens: vec![263], tokenization: TokenizationBudget::default(),
        generation: GenerationBudget { scalar_products: MAX_DECODER_PRODUCTS, sampling_entries: MAX_SAMPLING_ENTRIES },
        max_output_bytes: 32 };
    (native, policy)
}
fn text(alarm: u8) -> (TextDecoder, NativeHelperPolicy) {
    let (native, policy) = numerical(alarm);
    let t = import(&document(&[added(263, "<eos>")], &[]), 264).unwrap();
    // The original native archive path must retain the control classification
    // AND input spelling before the existing evaluator admits the policy.
    let t = ByteBpe::from_bytes(&profile(264), &t.to_bytes().unwrap()).unwrap();
    (TextDecoder::new(native, t).unwrap(), policy)
}

#[test]
fn hf_special_file_import_drives_real_helper_verdicts_after_source_removal() {
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("named-helper cleanup: {error}"); }
        }
    }
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let root = Directory(std::env::temp_dir().join(format!("fa-named-helper-{}-{stamp}-{}",
        std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))));
    std::fs::create_dir(&root.0).unwrap();
    for (prompt, expected_verdict, answer) in [(b"?".as_slice(), Verdict::Allow, b"allow".as_slice()),
        (b"!", Verdict::Deny, b"deny"), (b"<eos>?", Verdict::Allow, b"allow")]
    {
        let source = document(&[added(263, "<eos>")], &[]);
        let path = root.0.join("tokenizer.json"); std::fs::write(&path, &source).unwrap();
        let (native, policy) = numerical(0);
        let mut file = std::fs::File::open(&path).unwrap();
        let decoder = TextDecoder::from_huggingface_reader(native, &mut file, source.len()).unwrap();
        drop(file); std::fs::remove_file(path).unwrap();
        let mut worker = NativeEvaluator::new(decoder, policy).unwrap();
        let original = input(prompt);
        assert_eq!(worker.evaluate(&original), Ok(expected_verdict));
        assert_eq!(worker.status(), NativeEvaluationStatus::Judged(expected_verdict));
        assert_eq!(worker.input(), Some(&original));
        assert_eq!(worker.sampled_draws(), 2);
        let report = worker.report().unwrap();
        assert_eq!(report.bytes().unwrap(), answer);
        assert_eq!(report.prompt().source(), prompt);
        assert!(report.prefix_controls().is_empty());
        assert_eq!(report.generation().finish(), GenerationFinish::StopToken);
        let expected_ids = if prompt == b"<eos>?" { vec![263, byte_id(b'?')] }
            else { vec![byte_id(prompt[0])] };
        assert_eq!(report.prompt().tokens(), expected_ids);
        assert_eq!(report.generation().reviewed_prompt_tokens(), expected_ids.len());
    }
}

#[test]
fn hf_special_held_answer_or_held_eos_cannot_become_a_vote_or_reroll() {
    for alarm in [1, 2] {
        let (decoder, policy) = text(alarm); let mut worker = NativeEvaluator::new(decoder, policy).unwrap();
        assert_eq!(worker.evaluate(&input(b"?")), Err(NativeEvaluationError::Incomplete(GenerationFinish::Held)));
        let bytes: &[u8] = if alarm == 1 { b"" } else { b"allow" };
        assert_eq!(worker.report().unwrap().bytes().unwrap(), bytes);
        assert_eq!(worker.sampled_draws(), u64::from(alarm));
        let position = worker.position();
        assert_eq!(worker.evaluate(&input(b"!")), Err(NativeEvaluationError::Contract(Error::WrongState)));
        assert_eq!(worker.position(), position); assert_eq!(worker.sampled_draws(), u64::from(alarm));
    }
}

#[test]
fn hf_special_missing_terminal_budget_is_not_a_complete_allow() {
    for budget in [false, true] {
        let (decoder, mut policy) = text(0);
        let finish = if budget { policy.generation.sampling_entries = 264; GenerationFinish::BudgetExhausted }
            else { policy.max_new_tokens = 1; GenerationFinish::TokenLimit };
        let mut worker = NativeEvaluator::new(decoder, policy).unwrap();
        assert_eq!(worker.evaluate(&input(b"?")), Err(NativeEvaluationError::Incomplete(finish)));
        assert_eq!(worker.report().unwrap().bytes().unwrap(), b"allow");
        assert_eq!(worker.sampled_draws(), 1);
    }
}

#[test]
fn hf_special_import_does_not_relax_native_stop_policy_or_profile_admission() {
    let (decoder, mut policy) = text(0); policy.stop_tokens = vec![259];
    assert_eq!(NativeEvaluator::new(decoder, policy).unwrap_err(), Error::Binding);
    let (decoder, mut policy) = text(0); policy.stop_tokens.clear();
    assert_eq!(NativeEvaluator::new(decoder, policy).unwrap_err(), Error::InvalidInput);
    let (decoder, mut policy) = text(0); let p = &policy.decoder_profile;
    policy.decoder_profile = DecoderProfile::new(DecoderIdentity { tokenizer_generation: 99, ..p.identity() },
        p.shape(), p.epsilon(), p.theta()).unwrap();
    assert_eq!(NativeEvaluator::new(decoder, policy).unwrap_err(), Error::Binding);
    let (decoder, policy) = text(0); let mut worker = NativeEvaluator::new(decoder, policy).unwrap();
    assert_eq!(worker.evaluate(&input(b"?")), Ok(Verdict::Allow));
}
