use super::{Fixture, byte_id};
use fa_reference::action::consequence::activation::{
    monitor::{RefinementBudget, RefinementMonitor, decoder::sampled::{MonitoredSampledDecoder,
        generation::{GenerationBudget, GenerationFinish, MAX_SAMPLING_ENTRIES,
            text::{TextDecoder, TextGenerationRequest},
            tokenizer::{ByteBpe, PreTokenization, TokenizationBudget}}}},
    probe::LinearProbe,
    tensor::kv::decoder::{DecoderBudget, DecoderLayerWeights, DecoderModel, MAX_DECODER_PRODUCTS,
        sampling::{SamplingPolicy, SamplingStart}},
};
use fa_reference::action::consequence::oversight::{
    helper_client::native::{NativeEvaluator, NativeHelperPolicy, NativeEvaluationError, NativeEvaluationStatus},
    helper_workers::wire::{WorkerInput, decode_request},
};
use fa_reference::{Error, full_input::InputProfileBinding, round::Verdict};
use std::collections::BTreeMap;
use std::io::{self, Cursor, Read};

fn input_profile() -> InputProfileBinding {
    InputProfileBinding { profile_id: 7, profile_bytes: b"Unicode segmentation causal fixture".to_vec(),
        model_epoch: 0, tokenizer_epoch: 0, policy_epoch: 0 }
}

// Original independently framed WorkerInput; no private verdict/input constructor.
fn input(prompt: &[u8]) -> WorkerInput {
    let profile = input_profile();
    let mut bytes = b"FAHW1".to_vec(); bytes.extend_from_slice(&0_u32.to_be_bytes());
    bytes.extend_from_slice(&9_u64.to_be_bytes()); bytes.extend_from_slice(&[7; 32]);
    bytes.extend_from_slice(&6_u16.to_be_bytes()); bytes.extend_from_slice(b"native");
    bytes.extend_from_slice(&64_u16.to_be_bytes());
    for value in [profile.profile_id, profile.model_epoch, profile.tokenizer_epoch, profile.policy_epoch] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    for value in [profile.profile_bytes.as_slice(), prompt] {
        bytes.extend_from_slice(&(value.len() as u32).to_be_bytes()); bytes.extend_from_slice(value);
    }
    bytes.extend_from_slice(&1_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u32.to_be_bytes()); bytes.extend_from_slice(&(prompt.len() as u32).to_be_bytes());
    bytes.push(1);
    bytes.extend_from_slice(&1_u16.to_be_bytes());
    bytes.extend_from_slice(&17_u64.to_be_bytes()); bytes.extend_from_slice(&22_u64.to_be_bytes());
    let count = (bytes.len() - 9) as u32; bytes[5..9].copy_from_slice(&count.to_be_bytes());
    decode_request(&bytes).unwrap()
}

// The model is independent of tokenizer mode. Original `a1` selects `allow`;
// original final singleton `1` selects `deny`. Both answers then select EOS.
// A changed verdict therefore requires a real change in original prompt IDs.
fn numerical(f: &Fixture, alarm: u8) -> MonitoredSampledDecoder {
    let count = f.profile.shape().vocabulary;
    let allow = f.id(b"allow") as usize;
    let deny = f.id(b"deny") as usize;
    let eos = f.named_id(b"<eos>") as usize;
    let mut embeddings = [1.0, 0.0].repeat(count);
    embeddings[byte_id(b'1') as usize * 2] = 0.0;
    embeddings[byte_id(b'1') as usize * 2 + 1] = 1.0;
    for token in [allow, deny] { embeddings[token * 2] = -1.0; embeddings[token * 2 + 1] = -1.0; }
    if alarm == 2 { embeddings[eos * 2] = -2.0; embeddings[eos * 2 + 1] = -2.0; }
    let mut output = vec![0.0; count * 2];
    output[allow * 2] = 10.0; output[deny * 2 + 1] = 10.0;
    output[eos * 2] = -10.0; output[eos * 2 + 1] = -10.0;
    let model = DecoderModel::new(f.profile.clone(), embeddings, vec![DecoderLayerWeights {
        attention_norm: vec![1.0; 2], queries: vec![0.0; 4], keys: vec![0.0; 4], values: vec![0.0; 4],
        attention_output: vec![0.0; 4], feed_forward_norm: vec![1.0; 2], gate: vec![0.0; 4],
        up: vec![0.0; 4], down: vec![0.0; 4],
    }], vec![1.0; 2], output).unwrap();
    let budget = RefinementBudget { encoded_bytes: 100_000, probe_coordinates: 100_000 };
    let threshold = match alarm { 1 => 1.0, 2 => 3.0, _ => 100.0 };
    let probe = LinearProbe::new(1, 1, model.residual_contract(1).unwrap().profile(),
        &[-1.0, -1.0], 0.0, threshold).unwrap();
    let monitor = RefinementMonitor::new(vec![probe], vec![23], budget).unwrap();
    MonitoredSampledDecoder::new(model, 12, 13, BTreeMap::from([(1, monitor)]), budget,
        SamplingStart { policy: SamplingPolicy::new(1, 1, count, 1.0, 1, 1.0).unwrap(),
            stream: 10, seed: 11 }).unwrap()
}

fn policy(f: &Fixture) -> NativeHelperPolicy {
    NativeHelperPolicy { input_profile: input_profile(), decoder_profile: f.profile.clone(),
        max_new_tokens: 2, stop_tokens: f.names.keys().copied().collect(),
        tokenization: TokenizationBudget::default(),
        generation: GenerationBudget { scalar_products: MAX_DECODER_PRODUCTS, sampling_entries: MAX_SAMPLING_ENTRIES },
        max_output_bytes: 64 }
}

fn text(f: &Fixture, mode: PreTokenization, alarm: u8) -> TextDecoder {
    let json = f.json(mode);
    TextDecoder::from_huggingface_reader_with_pre_tokenization(numerical(f, alarm),
        &mut Cursor::new(json.as_bytes()), json.len(), mode).unwrap()
}

#[test]
fn unicode_bpe_boundary_changes_the_original_native_helper_verdict_causally() {
    let f = Fixture::new(true);
    for mode in [PreTokenization::RawBytes, PreTokenization::Gpt2Unicode17, PreTokenization::Llama3Unicode17] {
        let expected = if mode == PreTokenization::RawBytes { Verdict::Allow } else { Verdict::Deny };
        let answer: &[u8] = if expected == Verdict::Allow { b"allow" } else { b"deny" };
        let ids = if expected == Verdict::Allow { vec![f.id(b"a1")] }
            else { vec![byte_id(b'a'), byte_id(b'1')] };
        for archived in [false, true] {
            let decoder = if archived {
                let bytes = f.imported(mode).to_bytes().unwrap();
                let tokenizer = ByteBpe::from_bytes(&f.profile, &bytes).unwrap();
                TextDecoder::new(numerical(&f, 0), tokenizer).unwrap()
            } else { text(&f, mode, 0) };
            let mut worker = NativeEvaluator::new(decoder, policy(&f)).unwrap();
            assert_eq!(worker.evaluate(&input(b"a1")), Ok(expected));
            assert_eq!(worker.status(), NativeEvaluationStatus::Judged(expected));
            assert_eq!(worker.sampled_draws(), 2);
            let report = worker.report().unwrap();
            assert_eq!(report.prompt().tokens(), ids);
            assert_eq!(report.prompt().source(), b"a1");
            assert_eq!(report.generation().reviewed_prompt_tokens(), ids.len());
            assert_eq!(report.generation().finish(), GenerationFinish::StopToken);
            assert_eq!(report.bytes().unwrap(), answer);
        }
    }
}

#[test]
fn unicode_cooperative_native_admission_preserves_the_same_original_work_and_verdict() {
    use fa_reference::action::consequence::oversight::helper_client::native::NativeEvaluationWork;
    let f = Fixture::new(true);
    for mode in [PreTokenization::RawBytes, PreTokenization::Gpt2Unicode17, PreTokenization::Llama3Unicode17] {
        let packet = input(b"a1");
        let verdict = if mode == PreTokenization::RawBytes { Verdict::Allow } else { Verdict::Deny };
        let prompt_tokens = if mode == PreTokenization::RawBytes { 1 } else { 2 };
        let mut synchronous = NativeEvaluator::new(text(&f, mode, 0), policy(&f)).unwrap();
        assert_eq!(synchronous.evaluate(&packet), Ok(verdict));
        let mut cooperative = NativeEvaluator::new(text(&f, mode, 0), policy(&f)).unwrap();
        assert_eq!(cooperative.begin_cooperative(&packet).unwrap().status, NativeEvaluationStatus::Admitting);
        for _ in 0..32 {
            if cooperative.status() != NativeEvaluationStatus::Admitting { break; }
            let progress = cooperative.advance(0).unwrap();
            assert_eq!(progress.work, NativeEvaluationWork::default());
            assert!(cooperative.report().is_none());
        }
        assert_eq!(cooperative.status(), NativeEvaluationStatus::Running);
        assert_eq!(cooperative.progress().requested_prompt_tokens, prompt_tokens);
        assert!(cooperative.progress().admission_steps > 0);
        assert_eq!(cooperative.progress().tokenization, synchronous.progress().tokenization);
        for _ in 0..8 {
            if cooperative.status() != NativeEvaluationStatus::Running { break; }
            cooperative.advance(cooperative.position()).unwrap();
        }
        assert_eq!(cooperative.status(), NativeEvaluationStatus::Judged(verdict));
        assert_eq!(cooperative.work(), synchronous.work());
        assert_eq!(cooperative.input(), Some(&packet));
        let report = cooperative.report().unwrap();
        let expected = synchronous.report().unwrap();
        assert_eq!(report.prompt().source(), b"a1");
        assert_eq!(report.prompt().tokens(), expected.prompt().tokens());
        assert_eq!(report.prompt().spans(), expected.prompt().spans());
        assert_eq!(report.bytes(), expected.bytes());
        if mode != PreTokenization::RawBytes {
            let mut invalid = NativeEvaluator::new(text(&f, mode, 0), policy(&f)).unwrap();
            assert!(invalid.begin_cooperative(&input(b"a\xff1")).is_err());
            assert_eq!(invalid.work(), NativeEvaluationWork::default());
            assert_eq!(invalid.progress().tokenization.input_bytes, 3);
            assert!(invalid.begin_cooperative(&packet).is_err());
        }
    }
}

#[test]
fn unicode_native_held_answer_and_held_terminal_never_become_a_verdict() {
    let f = Fixture::new(true);
    for mode in [PreTokenization::Gpt2Unicode17, PreTokenization::Llama3Unicode17] {
        for alarm in [1, 2] {
            let mut worker = NativeEvaluator::new(text(&f, mode, alarm), policy(&f)).unwrap();
            assert_eq!(worker.evaluate(&input(b"a1")),
                Err(NativeEvaluationError::Incomplete(GenerationFinish::Held)));
            assert_eq!(worker.sampled_draws(), u64::from(alarm));
            let expected: &[u8] = if alarm == 1 { b"" } else { b"deny" };
            assert_eq!(worker.report().unwrap().bytes().unwrap(), expected);
            let position = worker.position();
            assert_eq!(worker.evaluate(&input(b" a")), Err(NativeEvaluationError::Contract(Error::WrongState)));
            assert_eq!(worker.position(), position);
            assert_eq!(worker.sampled_draws(), u64::from(alarm));
        }
    }
}

#[test]
fn unicode_invalid_full_prompt_is_refused_before_any_original_numeric_work() {
    let f = Fixture::new(true);
    for mode in [PreTokenization::Gpt2Unicode17, PreTokenization::Llama3Unicode17] {
        let mut decoder = text(&f, mode, 0);
        let policy = policy(&f);
        let prompt = b"<eos>a\xff1".to_vec();
        let result = decoder.generate(0, TextGenerationRequest { prompt: prompt.clone(), prefix_controls: vec![],
            max_new_tokens: 2, stop_tokens: policy.stop_tokens, tokenization: policy.tokenization,
            generation: policy.generation, max_output_bytes: policy.max_output_bytes }).unwrap_err();
        assert_eq!(result.error, Error::InvalidInput);
        assert_eq!(result.tokenization.input_bytes, prompt.len());
        assert_eq!(decoder.position(), 0);
        assert_eq!(decoder.sampled_draws(), 0);
        assert_eq!(decoder.monitoring_work().frame_reviews, 0);
    }
}

#[test]
fn unicode_reader_retains_original_eof_bounds_and_fresh_owner_admission() {
    struct Fragmented<'a> { bytes: &'a [u8], at: usize, interrupted: bool, eof_reads: usize }
    impl Read for Fragmented<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if !self.interrupted { self.interrupted = true; return Err(io::ErrorKind::Interrupted.into()); }
            if self.at == self.bytes.len() { self.eof_reads += 1; return Ok(0); }
            let count = buffer.len().min(7).min(self.bytes.len() - self.at);
            buffer[..count].copy_from_slice(&self.bytes[self.at..self.at + count]); self.at += count;
            Ok(count)
        }
    }
    struct NeverRead;
    impl Read for NeverRead {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> { panic!("no reader I/O after failed admission") }
    }
    let f = Fixture::new(true);
    for mode in [PreTokenization::Gpt2Unicode17, PreTokenization::Llama3Unicode17] {
        let json = f.json(mode);
        let mut reader = Fragmented { bytes: json.as_bytes(), at: 0, interrupted: false, eof_reads: 0 };
        let admitted = ByteBpe::read_huggingface_json_with_pre_tokenization(&f.profile,
            &mut reader, json.len(), mode).unwrap();
        assert_eq!(reader.at, json.len()); assert_eq!(reader.eof_reads, 1);
        assert_eq!(admitted.to_bytes().unwrap(), f.imported(mode).to_bytes().unwrap());
        assert_eq!(ByteBpe::read_huggingface_json_with_pre_tokenization(&f.profile,
            &mut NeverRead, 0, mode).unwrap_err().kind(), io::ErrorKind::InvalidInput);
        let mut numeric = numerical(&f, 0);
        numeric.advance_forced(0, byte_id(b'a'), DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap();
        assert_eq!(TextDecoder::from_huggingface_reader_with_pre_tokenization(numeric,
            &mut NeverRead, json.len(), mode).unwrap_err().kind(), io::ErrorKind::InvalidInput);
        assert!(ByteBpe::read_huggingface_json_with_pre_tokenization(&f.profile,
            &mut Cursor::new(json.as_bytes()), json.len() - 1, mode).is_err());
    }
}
