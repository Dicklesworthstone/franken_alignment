use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, DecoderModel, MAX_DECODER_PRODUCTS};
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::pretrained::{
    CheckpointError, CheckpointFileLimits, CheckpointInput, ConfigIssue, LlamaConfig, MAX_CONFIG_BYTES,
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::WeightError;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
#[path = "support/pretrained_fixture.rs"]
mod fixture;

const BASE: &str = r#"{"model_type":"llama","vocab_size":6,"hidden_size":4,"intermediate_size":6,"num_hidden_layers":2,"num_attention_heads":2,"num_key_value_heads":1,"max_position_embeddings":64,"rms_norm_eps":1e-5"#;
fn configuration(extra: &str) -> String { format!("{BASE}{extra}}}") }
fn decode(bytes: &[u8], context: usize) -> Result<LlamaConfig, CheckpointError> {
    LlamaConfig::decode(fixture::profile(16).identity(), context, bytes)
}
fn check_model(model: &DecoderModel) {
    let budget = DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS };
    let a = model.recompute(1, &[1, 2, 0], budget).unwrap();
    let b = fixture::model(16).recompute(1, &[1, 2, 0], budget).unwrap();
    assert!(a.logits().unwrap().iter().zip(b.logits().unwrap()).all(|(a,b)| a.to_bits() == b.to_bits()));
    assert_eq!(a.cache_image().unwrap().encode().unwrap(), b.cache_image().unwrap().encode().unwrap());
}

#[test]
fn legacy_configuration_records_defaults_without_changing_the_existing_engine() {
    let config = configuration(r#", "rope_theta":10000, "hidden_act":"silu", "architectures":["LlamaForCausalLM"]"#);
    let decoded = decode(config.as_bytes(), 16).unwrap();
    assert_eq!(decoded.profile(), &fixture::profile(16));
    assert_eq!(decoded.trained_context(), 64);
    assert!(decoded.defaulted_fields().contains("attention_bias"));
    assert!(!decoded.defaulted_fields().contains("rope_theta"));
    let bytes = fixture::encode(&fixture::tensors(false));
    let (model, receipt) = DecoderModel::from_llama_safetensors(fixture::profile(16).identity(), 16, config.as_bytes(), &bytes).unwrap();
    assert_eq!(receipt.configuration, decoded); check_model(&model);
}

#[test]
fn default_rope_parameter_object_matches_legacy_but_conflicting_sources_refuse() {
    let config = configuration(r#", "rope_parameters":{"rope_type":"default","rope_theta":10000}"#);
    assert_eq!(decode(config.as_bytes(), 16).unwrap().profile(), &fixture::profile(16));
    for extra in [
        r#", "rope_theta":20000,"rope_parameters":{"rope_type":"default","rope_theta":10000}"#,
        r#", "rope_parameters":{"rope_type":"linear","rope_theta":10000}"#,
        r#", "rope_parameters":{"rope_type":"default","rope_theta":10000,"factor":2}"#,
        r#", "rope_scaling":{"rope_type":"llama3","factor":8}"#,
    ] {
        assert!(matches!(decode(configuration(extra).as_bytes(), 16), Err(CheckpointError::Configuration { issue: ConfigIssue::Unsupported, .. })));
    }
    assert_eq!(decode(configuration(r#", "rope_scaling":null"#).as_bytes(), 16).unwrap().profile().theta(), 10000.0);
}

#[test]
fn missing_cache_head_count_uses_declared_mha_default_not_checkpoint_shape_guessing() {
    let config = configuration("").replace("\"num_key_value_heads\":1,", "");
    let decoded = decode(config.as_bytes(), 16).unwrap();
    assert_eq!(decoded.profile().shape().cache_heads, 2);
    assert!(decoded.defaulted_fields().contains("num_key_value_heads"));
    let bytes = fixture::encode(&fixture::tensors(true));
    assert!(matches!(DecoderModel::from_llama_safetensors(fixture::profile(16).identity(), 16, config.as_bytes(), &bytes),
        Err(CheckpointError::Weights(WeightError::Tensor { .. }))));
}

#[test]
fn unsupported_architectures_bias_tying_and_remote_code_never_fall_back() {
    for extra in [
        r#", "attention_bias":true"#, r#", "mlp_bias":true"#,
        r#", "tie_word_embeddings":true"#, r#", "pretraining_tp":2"#, r#", "pretraining_tp":1.0"#,
        r#", "hidden_act":"relu"#, r#", "head_dim":4"#,
        r#", "attention_dropout":0.1"#, r#", "partial_rotary_factor":0.5"#,
        r#", "architectures":["LlamaForSequenceClassification"]"#,
        r#", "auto_map":{"AutoModel":"untrusted.module"}"#,
        r#", "quantization_config":{"bits":4}"#, r#", "sliding_window":8"#,
        r#", "is_encoder_decoder":true"#, r#", "add_cross_attention":true"#,
    ] {
        assert!(matches!(decode(configuration(extra).as_bytes(), 16), Err(CheckpointError::Configuration { issue: ConfigIssue::Unsupported, .. })), "{extra}");
    }
}

#[test]
fn accepted_metadata_is_listed_and_never_changes_tokens_or_parameter_arithmetic() {
    let config = configuration(r#", "_name_or_path":"not-an-executable", "torch_dtype":"float16", "transformers_version":"4.50.0", "eos_token_id":[2,3], "bos_token_id":1, "pad_token_id":null, "use_cache":false"#);
    let decoded = decode(config.as_bytes(), 16).unwrap();
    for name in ["_name_or_path", "torch_dtype", "eos_token_id", "use_cache"] {
        assert!(decoded.ignored_metadata_fields().contains(name));
    }
    assert_eq!(decoded.profile(), &fixture::profile(16));
    let (model, _) = DecoderModel::from_llama_safetensors(fixture::profile(16).identity(), 16, config.as_bytes(),
        include_bytes!("fixtures/decoder_mixed.safetensors")).unwrap();
    let session = model.recompute(1, &[2,3,2], DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap();
    assert_eq!(session.tokens(), &[2,3,2]);
    assert!(decode(configuration(r#", "eos_token_id":["2"]"#).as_bytes(),16).is_err());
}

#[test]
fn explicit_execution_window_can_shrink_but_cannot_extend_trained_context() {
    let config = configuration("");
    assert_eq!(decode(config.as_bytes(), 8).unwrap().profile().shape().context, 8);
    for context in [0, 65, usize::MAX] { assert!(decode(config.as_bytes(), context).is_err()); }
    let enormous = config.replace("\"hidden_size\":4", "\"hidden_size\":18446744073709551615");
    assert!(decode(enormous.as_bytes(),16).is_err());
}

#[test]
fn configuration_duplicates_truncation_missing_fields_and_nonfinite_numbers_refuse() {
    let config = configuration("");
    for length in 0..config.len() { assert!(decode(&config.as_bytes()[..length], 16).is_err()); }
    let missing = config.replace("\"hidden_size\":4,", "");
    assert!(matches!(decode(missing.as_bytes(),16), Err(CheckpointError::Configuration { issue: ConfigIssue::Missing, .. })));
    assert!(matches!(decode(configuration(r#", "hidden_size":4"#).as_bytes(),16),
        Err(CheckpointError::Configuration { issue: ConfigIssue::Syntax, .. })));
    for extra in [r#", "rope_theta":1e999"#, r#", "rope_theta":"10000"#, r#", "attention_bias":null"#] {
        assert!(decode(configuration(extra).as_bytes(),16).is_err());
    }
    assert!(matches!(decode(&vec![b' '; MAX_CONFIG_BYTES + 1],16), Err(CheckpointError::Limit)));
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        for _ in 0..64 {
            let path = std::env::temp_dir().join(format!("fa-decoder-weights-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {},
                Err(error) => panic!("temporary directory: {error}"),
            }
        }
        panic!("temporary directory namespace exhausted");
    }
    fn files(&self) -> (PathBuf, PathBuf) {
        let config = self.0.join("config.json"); let weights = self.0.join("model.safetensors");
        fs::write(&config, configuration("")).unwrap();
        fs::write(&weights, include_bytes!("fixtures/decoder_mixed.safetensors")).unwrap();
        (config, weights)
    }
}
impl Drop for Temp { fn drop(&mut self) { fs::remove_dir_all(&self.0).unwrap(); } }

#[test]
fn regular_files_load_and_remain_independent_of_source_lifetime() {
    let temp = Temp::new(); let (config, weights) = temp.files();
    let (model, receipt) = DecoderModel::from_llama_files(fixture::profile(16).identity(), 16,
        &config, &weights, CheckpointFileLimits::default()).unwrap();
    assert_eq!(receipt.configuration.config_bytes(), configuration("").len());
    drop(temp); check_model(&model);
}

#[test]
fn invalid_config_refuses_before_a_missing_weight_file_is_opened() {
    let temp = Temp::new(); let (config, weights) = temp.files(); fs::remove_file(&weights).unwrap();
    fs::write(&config, configuration(r#", "tie_word_embeddings":true"#)).unwrap();
    assert!(matches!(DecoderModel::from_llama_files(fixture::profile(16).identity(),16,&config,&weights,CheckpointFileLimits::default()),
        Err(CheckpointError::Configuration { issue: ConfigIssue::Unsupported, .. })));
    fs::write(&config, configuration("")).unwrap();
    assert!(matches!(DecoderModel::from_llama_files(fixture::profile(16).identity(),16,&config,&weights,CheckpointFileLimits::default()),
        Err(CheckpointError::Io { input: CheckpointInput::Weights, .. })));
}

#[test]
fn exact_file_limits_pass_and_one_byte_less_refuses() {
    let temp = Temp::new(); let (config, weights) = temp.files();
    let limits = CheckpointFileLimits { config_bytes: fs::metadata(&config).unwrap().len() as usize,
        weight_bytes: fs::metadata(&weights).unwrap().len() as usize };
    let (model, _) = DecoderModel::from_llama_files(fixture::profile(16).identity(),16,&config,&weights,limits).unwrap();
    check_model(&model);
    for changed in [CheckpointFileLimits { config_bytes: limits.config_bytes-1, ..limits },
        CheckpointFileLimits { weight_bytes: limits.weight_bytes-1, ..limits },
        CheckpointFileLimits { weight_bytes: 0, ..limits }]
    {
        assert!(matches!(DecoderModel::from_llama_files(fixture::profile(16).identity(),16,&config,&weights,changed),Err(CheckpointError::Limit)));
    }
}

#[test]
fn directories_and_truncated_weight_files_do_not_become_empty_models() {
    let temp = Temp::new(); let (config, weights) = temp.files();
    assert!(matches!(DecoderModel::from_llama_files(fixture::profile(16).identity(),16,&config,&temp.0,CheckpointFileLimits::default()),
        Err(CheckpointError::NotRegular(CheckpointInput::Weights))));
    fs::write(&weights, [0;7]).unwrap();
    assert!(matches!(DecoderModel::from_llama_files(fixture::profile(16).identity(),16,&config,&weights,CheckpointFileLimits::default()),
        Err(CheckpointError::Weights(WeightError::Header))));
}

#[cfg(unix)]
#[test]
fn selected_symlinks_refuse_without_following_checkpoint_paths() {
    let temp = Temp::new(); let (config, weights) = temp.files();
    let link = temp.0.join("linked.safetensors"); std::os::unix::fs::symlink(&weights, &link).unwrap();
    assert!(matches!(DecoderModel::from_llama_files(fixture::profile(16).identity(),16,&config,&link,CheckpointFileLimits::default()),
        Err(CheckpointError::NotRegular(CheckpointInput::Weights))));
    let (model, _) = DecoderModel::from_llama_files(fixture::profile(16).identity(),16,&config,&weights,CheckpointFileLimits::default()).unwrap();
    check_model(&model);
}
