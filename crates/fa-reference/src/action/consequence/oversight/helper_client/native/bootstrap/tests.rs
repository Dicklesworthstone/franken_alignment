//! Actual checkpoint parsing -> original numerical inference -> helper verdict.
//! Weights are explicitly synthetic, not a pretrained-model accuracy campaign.
use super::*;
use super::super::{NativeEvaluationError, NativeEvaluationStatus};
use super::super::tests::{expected, input};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationBudget, GenerationFinish, MAX_SAMPLING_ENTRIES,
};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{Merge, TokenBytes, TokenizationBudget};
use crate::action::consequence::activation::tensor::kv::decoder::{
    DecoderIdentity, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS,
};
use crate::action::consequence::activation::tensor::kv::decoder::safetensors::reader::MAX_WEIGHT_READ_CALLS;
use crate::round::Verdict;
use std::io::{self, Cursor};

type Tensor = (String, Vec<usize>, Vec<f32>);
pub(super) struct Fixture {
    pub(super) policy: NativeHelperPolicy,
    pub(super) configuration: Vec<u8>,
    pub(super) tokenizer: Vec<u8>,
    pub(super) monitoring: Vec<u8>,
    pub(super) sampling: Vec<u8>,
    pub(super) tensors: Vec<Tensor>,
}
impl Fixture {
    pub(super) fn new(alarm: bool) -> Self {
        let mut vocab: Vec<_> = (0..=255).map(|b| TokenBytes::Content(vec![b])).collect();
        let mut merges = Vec::new();
        let mut word = |bytes: &[u8]| {
            let mut left = u32::from(bytes[0]);
            for end in 2..=bytes.len() {
                let result = vocab.len() as u32;
                vocab.push(TokenBytes::Content(bytes[..end].to_vec()));
                merges.push(Merge { left, right: u32::from(bytes[end - 1]), result });
                left = result;
            }
            left
        };
        let allow = word(b"allow"); let deny = word(b"deny");
        let stop = vocab.len() as u32; vocab.push(TokenBytes::Control);
        let count = vocab.len();
        let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2,
            model_generation: 3, tokenizer_generation: 4, profile_generation: 5 },
            DecoderShape { vocabulary: count, hidden: 2, intermediate: 2, layers: 1,
                query_heads: 1, cache_heads: 1, context: 1024 }, 0.00001, 10000.0).unwrap();
        let tokenizer = ByteBpe::new(profile.clone(), vocab, merges).unwrap().to_bytes().unwrap();
        let configuration = format!(r#"{{"model_type":"llama","vocab_size":{count},"hidden_size":2,"intermediate_size":2,"num_hidden_layers":1,"num_attention_heads":1,"max_position_embeddings":1024,"rms_norm_eps":0.00001,"rope_theta":10000.0}}"#).into_bytes();
        let threshold = if alarm { 0.5 } else { 100.0 };
        let monitoring = format!(r#"{{"schema":"fa.decoder-monitor/1","generation":13,"identity":{{"tenant":1,"model":2,"model_generation":3,"tokenizer_generation":4,"profile_generation":5}},"budget":{{"encoded_bytes":100000,"probe_coordinates":100000}},"layers":[{{"layer":1,"levels":[23],"budget":{{"encoded_bytes":100000,"probe_coordinates":100000}},"probes":[{{"id":1,"generation":1,"weights":[-1.0,-1.0],"bias":0.0,"threshold":{threshold}}}]}}]}}"#).into_bytes();
        let sampling = format!(r#"{{"schema":"fa.decoder-sampling/1","id":1,"generation":1,"vocabulary":{count},"temperature":1.0,"top_k":1,"top_p":1.0,"stream":10,"seed":11}}"#).into_bytes();
        let policy = NativeHelperPolicy { input_profile: expected(), decoder_profile: profile,
            max_new_tokens: 2, stop_tokens: vec![stop], tokenization: TokenizationBudget::default(),
            generation: GenerationBudget { scalar_products: MAX_DECODER_PRODUCTS, sampling_entries: MAX_SAMPLING_ENTRIES },
            max_output_bytes: 128 };
        let mut embeddings = vec![0.0; count * 2];
        for id in 0..count { embeddings[id * 2] = 1.0; }
        embeddings[usize::from(b'!') * 2] = 0.0; embeddings[usize::from(b'!') * 2 + 1] = 1.0;
        for id in [allow, deny] { embeddings[id as usize * 2] = -1.0; embeddings[id as usize * 2 + 1] = -1.0; }
        let mut output = vec![0.0; count * 2];
        output[allow as usize * 2] = 10.0; output[deny as usize * 2 + 1] = 10.0;
        output[stop as usize * 2] = -10.0; output[stop as usize * 2 + 1] = -10.0;
        let mut tensors = vec![
            ("model.embed_tokens.weight".into(), vec![count, 2], embeddings),
            ("model.norm.weight".into(), vec![2], vec![1.0; 2]),
            ("lm_head.weight".into(), vec![count, 2], output),
        ];
        for (name, shape, values) in [
            ("input_layernorm.weight", vec![2], vec![1.0; 2]),
            ("post_attention_layernorm.weight", vec![2], vec![1.0; 2]),
            ("self_attn.q_proj.weight", vec![2, 2], vec![0.0; 4]),
            ("self_attn.k_proj.weight", vec![2, 2], vec![0.0; 4]),
            ("self_attn.v_proj.weight", vec![2, 2], vec![0.0; 4]),
            ("self_attn.o_proj.weight", vec![2, 2], vec![0.0; 4]),
            ("mlp.gate_proj.weight", vec![2, 2], vec![0.0; 4]),
            ("mlp.up_proj.weight", vec![2, 2], vec![0.0; 4]),
            ("mlp.down_proj.weight", vec![2, 2], vec![0.0; 4]),
        ] { tensors.push((format!("model.layers.0.{name}"), shape, values)); }
        Self { policy, configuration, tokenizer, monitoring, sampling, tensors }
    }
    pub(super) fn bootstrap(&self) -> NativeHelperBootstrap<'_> {
        NativeHelperBootstrap { policy: &self.policy, stream: 12, configuration: &self.configuration,
            tokenizer: &self.tokenizer, monitoring: &self.monitoring, sampling: &self.sampling }
    }
    pub(super) fn weights(&self) -> Vec<u8> { file(&self.tensors) }
    fn shards(&self) -> (Vec<u8>, BTreeMap<String, Cursor<Vec<u8>>>) {
        let partitions = [(&self.tensors[..3], "first.safetensors"), (&self.tensors[3..], "second.safetensors")];
        let mut names = Vec::new(); let mut sources = BTreeMap::new();
        for (tensors, label) in partitions {
            for (name, _, _) in tensors { names.push(format!("\"{name}\":\"{label}\"")); }
            sources.insert(label.to_owned(), Cursor::new(file(tensors)));
        }
        let index = format!("{{\"weight_map\":{{{}}}}}", names.join(",")).into_bytes();
        (index, sources)
    }
}
fn file(tensors: &[Tensor]) -> Vec<u8> {
    let mut data = Vec::new(); let mut entries = Vec::new();
    for (name, shape, values) in tensors {
        let start = data.len();
        for value in values { data.extend_from_slice(&value.to_le_bytes()); }
        let dimensions = shape.iter().map(usize::to_string).collect::<Vec<_>>().join(",");
        entries.push(format!("\"{name}\":{{\"dtype\":\"F32\",\"shape\":[{dimensions}],\"data_offsets\":[{start},{}]}}", data.len()));
    }
    let mut header = format!("{{{}}}", entries.join(",")).into_bytes();
    while !header.len().is_multiple_of(8) { header.push(b' '); }
    let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
    bytes.extend_from_slice(&header); bytes.extend_from_slice(&data); bytes
}
pub(super) fn budget() -> WeightReadBudget { WeightReadBudget::new(1_048_576, MAX_WEIGHT_READ_CALLS).unwrap() }

#[test]
fn checkpoint_loaded_helper_computes_input_dependent_verdicts_without_startup_inference() {
    let fixture = Fixture::new(false);
    for (prompt, expected) in [(b"?".as_slice(), Verdict::Allow), (b"!", Verdict::Deny)] {
        let weights = fixture.weights(); let mut usage = budget();
        let (mut worker, receipt) = NativeEvaluator::read_llama_checkpoint(fixture.bootstrap(),
            &mut Cursor::new(&weights), &mut usage).unwrap();
        assert_eq!(receipt.configuration.profile(), &fixture.policy.decoder_profile);
        assert_eq!(receipt.weights.file_bytes, weights.len());
        assert_eq!(usage.usage().bytes_read, weights.len());
        assert!(usage.usage().read_calls > 0);
        assert_eq!(worker.status(), NativeEvaluationStatus::AwaitingInput);
        assert_eq!(worker.position(), 0); assert_eq!(worker.sampled_draws(), 0);
        assert_eq!(worker.evaluate(&input(prompt)), Ok(expected));
        assert_eq!(worker.report().unwrap().prompt().source(), prompt);
        assert_eq!(worker.sampled_draws(), 2);
    }
}

#[test]
fn bootstrapping_matches_explicit_original_loader_monitor_and_sampler_composition() {
    let fixture = Fixture::new(false); let weights = fixture.weights();
    let (mut loaded, _) = NativeEvaluator::read_llama_checkpoint(fixture.bootstrap(), &mut Cursor::new(&weights), &mut budget()).unwrap();
    let (model, _) = DecoderModel::from_llama_safetensors(fixture.policy.decoder_profile.identity(),
        1024, &fixture.configuration, &weights).unwrap();
    let monitored = MonitoredSampledDecoder::from_json(model, 12, &fixture.monitoring, &fixture.sampling).unwrap();
    let tokenizer = ByteBpe::from_bytes(&fixture.policy.decoder_profile, &fixture.tokenizer).unwrap();
    let mut direct = NativeEvaluator::new(TextDecoder::new(monitored, tokenizer).unwrap(), fixture.policy.clone()).unwrap();
    let original = input(b"\x00raw\xff?");
    assert_eq!(loaded.evaluate(&original), direct.evaluate(&original));
    assert_eq!(loaded.report().unwrap().bytes(), direct.report().unwrap().bytes());
    assert_eq!(loaded.work(), direct.work());
}

#[test]
fn changed_model_shape_or_numerical_profile_cannot_touch_weight_reader() {
    for field in ["hidden_size", "rms_norm_eps"] {
        let mut fixture = Fixture::new(false);
        let old = if field == "hidden_size" { "\"hidden_size\":2" } else { "\"rms_norm_eps\":0.00001" };
        let new = if field == "hidden_size" { "\"hidden_size\":4" } else { "\"rms_norm_eps\":0.00002" };
        fixture.configuration = String::from_utf8(fixture.configuration).unwrap().replace(old, new).into_bytes();
        let mut reader = Cursor::new(fixture.weights()); let mut usage = budget();
        assert!(matches!(NativeEvaluator::read_llama_checkpoint(fixture.bootstrap(), &mut reader, &mut usage),
            Err(NativeBootstrapError::Contract(Error::Binding))));
        assert_eq!(reader.position(), 0); assert_eq!(usage.usage().read_calls, 0);
    }
}

#[test]
fn malformed_tokenizer_sampler_or_helper_policy_refuses_before_weight_io() {
    for mode in 0..5 {
        let mut fixture = Fixture::new(false);
        match mode {
            0 => fixture.tokenizer[8] ^= 1,
            1 => fixture.sampling = b"{}".to_vec(),
            2 => fixture.policy.stop_tokens = vec![u32::from(b'a')],
            3 => fixture.policy.max_output_bytes = 1,
            _ => fixture.monitoring = vec![0; MAX_MONITOR_CONFIG_BYTES + 1],
        }
        let mut reader = Cursor::new(fixture.weights()); let mut usage = budget();
        assert!(NativeEvaluator::read_llama_checkpoint(fixture.bootstrap(), &mut reader, &mut usage).is_err());
        assert_eq!(reader.position(), 0); assert_eq!(usage.usage().read_calls, 0);
    }
}

#[test]
fn invalid_or_unbound_monitor_returns_no_evaluator_after_accounted_loading() {
    for invalid in [false, true] {
        let mut fixture = Fixture::new(false);
        fixture.monitoring = if invalid { b"{}".to_vec() } else {
            String::from_utf8(fixture.monitoring).unwrap().replace("\"model_generation\":3", "\"model_generation\":4").into_bytes()
        };
        let weights = fixture.weights(); let mut usage = budget();
        assert!(matches!(NativeEvaluator::read_llama_checkpoint(fixture.bootstrap(), &mut Cursor::new(&weights), &mut usage),
            Err(NativeBootstrapError::Sampling(_))));
        assert_eq!(usage.usage().bytes_read, weights.len());
    }
}

#[test]
fn loaded_alarm_is_not_replaced_with_quiet_probes_or_an_allow_fallback() {
    let fixture = Fixture::new(true);
    let (mut worker, _) = NativeEvaluator::read_llama_checkpoint(fixture.bootstrap(), &mut Cursor::new(fixture.weights()), &mut budget()).unwrap();
    assert_eq!(worker.evaluate(&input(b"?")), Err(NativeEvaluationError::Incomplete(GenerationFinish::Held)));
    assert_eq!(worker.report().unwrap().bytes().unwrap(), b"");
    assert_eq!(worker.sampled_draws(), 1);
    assert!(worker.evaluate(&input(b"!" )).is_err());
    assert_eq!(worker.sampled_draws(), 1);
}

#[test]
fn truncated_trailing_and_nonfinite_weights_never_return_partial_native_owners() {
    for mode in 0..3 {
        let mut fixture = Fixture::new(false);
        if mode == 2 { fixture.tensors[0].2[0] = f32::INFINITY; }
        let mut weights = fixture.weights();
        if mode == 0 { weights.pop(); } else if mode == 1 { weights.push(0); }
        let mut usage = budget();
        assert!(matches!(NativeEvaluator::read_llama_checkpoint(fixture.bootstrap(), &mut Cursor::new(weights), &mut usage),
            Err(NativeBootstrapError::Checkpoint(_))));
        assert!(usage.usage().bytes_read > 0);
    }
}

#[test]
fn shard_loading_produces_the_same_real_verdict_and_original_receipt() {
    let fixture = Fixture::new(false); let (index, mut sources) = fixture.shards();
    let mut usage = budget();
    let (mut worker, receipt) = NativeEvaluator::read_llama_checkpoint_shards(fixture.bootstrap(), &index, &mut sources, &mut usage).unwrap();
    assert_eq!(receipt.weights.shards.len(), 2);
    assert_eq!(receipt.weights.file_bytes, usage.usage().bytes_read);
    assert_eq!(worker.evaluate(&input(b"!")), Ok(Verdict::Deny));
    assert_eq!(worker.sampled_draws(), 2);
}

#[test]
fn shard_inventory_errors_do_not_open_labels_or_consume_unselected_readers() {
    let fixture = Fixture::new(false); let (index, mut sources) = fixture.shards();
    sources.remove("second.safetensors"); let mut usage = budget();
    assert!(NativeEvaluator::read_llama_checkpoint_shards(fixture.bootstrap(), &index, &mut sources, &mut usage).is_err());
    assert_eq!(usage.usage().read_calls, 0);
    assert!(sources.values().all(|source| source.position() == 0));
}

#[test]
fn failed_startup_never_resets_shared_weight_io_allowance() {
    struct Interrupted;
    impl Read for Interrupted {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> { Err(io::ErrorKind::Interrupted.into()) }
    }
    let fixture = Fixture::new(false);
    let mut usage = WeightReadBudget::new(1_048_576, 3).unwrap();
    assert!(NativeEvaluator::read_llama_checkpoint(fixture.bootstrap(), &mut Interrupted, &mut usage).is_err());
    assert_eq!(usage.usage().read_calls, 3); assert_eq!(usage.remaining_calls(), 0);
    let mut source = Cursor::new(fixture.weights());
    assert!(NativeEvaluator::read_llama_checkpoint(fixture.bootstrap(), &mut source, &mut usage).is_err());
    assert_eq!(usage.usage().read_calls, 3); assert_eq!(source.position(), 0);
    // A separately admitted fresh budget is the positive control, not a hidden reset.
    assert!(NativeEvaluator::read_llama_checkpoint(fixture.bootstrap(), &mut source, &mut budget()).is_ok());
}

#[test]
fn complete_weights_need_the_original_eof_probe_budget() {
    let fixture = Fixture::new(false); let weights = fixture.weights();
    for spare in [0, 1] {
        let mut usage = WeightReadBudget::new(weights.len() + spare, MAX_WEIGHT_READ_CALLS).unwrap();
        let result = NativeEvaluator::read_llama_checkpoint(fixture.bootstrap(), &mut Cursor::new(&weights), &mut usage);
        if spare == 0 { assert!(matches!(result, Err(NativeBootstrapError::Checkpoint(CheckpointError::Limit)))); }
        else { assert!(result.is_ok()); }
    }
}

#[test]
fn zero_decoder_stream_is_not_inferred_from_sampler_or_wall_clock() {
    let fixture = Fixture::new(false); let mut bootstrap = fixture.bootstrap(); bootstrap.stream = 0;
    let mut usage = budget(); let mut source = Cursor::new(fixture.weights());
    assert!(matches!(NativeEvaluator::read_llama_checkpoint(bootstrap, &mut source, &mut usage),
        Err(NativeBootstrapError::Contract(Error::InvalidInput))));
    assert_eq!(source.position(), 0); assert_eq!(usage.usage().read_calls, 0);
    assert!(NativeEvaluator::read_llama_checkpoint(fixture.bootstrap(), &mut source, &mut usage).is_ok());
}
