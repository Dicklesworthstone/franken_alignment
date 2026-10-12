//! Actual artifact admission and exact original predictor configuration.
//! Runtime publication/forecast ordering is exercised in the predictive tests.
use super::*;
use super::super::{fields::{Assets, Fields, MAX_DATA_BYTES}, forecast::ForecastInput};
use fa_reference::action::consequence::activation::{
    CaptureProfile,
    consistency::{BinaryForecast, ErrorBudget, ForecastRegistration},
    monitor::learned::LearnedMonitorBudget,
    tensor::kv::experiment::KvSide,
};
use fa_reference::action::consequence::delivery::{stream::StreamProfile,
    persistent::observed::consistency::{FileConsistencyConfig, FileConsistencyParameters,
        learned::FileLearnedConsistencyConfig}};
use std::path::{Path, PathBuf};

/// Install independent synthetic forecast data into an existing /1 or /4
/// fixture. Publication limits are read from that recipe, so runtime tests may
/// provision a multi-message stream before this helper is called.
pub(super) fn install(root: &Root, recipe: &Path, owned: bool, max_prediction_age_ticks: u64) -> PathBuf {
    let original = fs::read_to_string(recipe).unwrap();
    let mut fields = Fields::parse(original.as_bytes(), 64 * 1024).unwrap();
    let mut publication = fields.object("publication_stream").unwrap();
    let id = publication.number("id").unwrap();
    let generation = publication.number("generation").unwrap();
    let messages = publication.size("max_messages").unwrap();
    let message_bytes = publication.size("max_message_bytes").unwrap();
    let total_bytes = publication.size("max_total_bytes").unwrap();
    publication.end().unwrap();
    let path = root.0.join("forecast.json");
    let source = if owned { r#""kind":"owned_learned_kv","layer":1,"side":"value""# }
        else { r#""kind":"raw_residual","layer":1"# };
    let tap = if owned { 3 } else { 4 };
    let learned = if owned { r#", "learned_budget":{"per_job":{"encoded_bytes":1048576,"probe_coordinates":10000,"reconstruction_products":1000000,"materialized_values":256,"refinements":64},"lifetime":{"encoded_bytes":8388608,"probe_coordinates":80000,"reconstruction_products":8000000,"materialized_values":2048,"refinements":512},"max_retained_source_bytes":8388608}"# }
        else { "" };
    fs::write(&path, format!(r#"{{"schema":"fa.learned-forecast/1","model_identity":{{"tenant":1,"model":9,"model_generation":1,"tokenizer_generation":1,"profile_generation":1}},"source":{{{source},"profile":{{"tenant":1,"model":9,"model_generation":1,"tap":{tap},"layout_generation":1}}}},"probe":{{"id":41,"generation":1,"weights_bits":[1065353216,2147483648],"bias_bits":0,"threshold_bits":1120403456}},"registration":{{"domain":77,"generation":1,"policy_generation":1,"event_prefix_hex":"41","negative":{{"null":32768,"alternative":32768}},"at_threshold":{{"null":32768,"alternative":32768}},"positive":{{"null":32768,"alternative":32768}}}},"alpha":{{"numerator":1,"denominator":100}},"stream":5,"max_predictions":8,"max_prediction_age_ticks":{max_prediction_age_ticks},"publication_stream":{{"id":{id},"generation":{generation},"max_messages":{messages},"max_message_bytes":{message_bytes},"max_total_bytes":{total_bytes}}}{learned}}}"#)).unwrap();
    let updated = if original.contains("fa.learned-publication/1") {
        original.replacen("fa.learned-publication/1", "fa.learned-publication/5", 1)
            .replacen("\"sidecar\":{", "\"sidecar\":{\"priority\":[],", 1)
            .replacen("\"native_review\":{", "\"native_review\":{\"round_ids\":[1],", 1)
    } else {
        assert!(original.contains("fa.learned-publication/4"));
        original.replacen("fa.learned-publication/4", "fa.learned-publication/5", 1)
    };
    fs::write(recipe, updated.replacen("\"files\":{",
        &format!("\"files\":{{\"forecast\":\"{}\",", path.display()), 1)).unwrap();
    path
}

fn expected(stream: StreamProfile, owned: bool) -> FileConsistencyConfig {
    let probability = BinaryForecast::new(32768, 32768).unwrap();
    FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 41, probe_generation: 1,
        profile: CaptureProfile { tenant: 1, model: 9, model_generation: 1,
            tap: if owned { 3 } else { 4 }, layout_generation: 1 },
        weights: vec![1.0, -0.0], bias: 0.0, threshold: 100.0,
        forecast: ForecastRegistration { domain: 77, generation: 1, policy_generation: 1,
            event_prefix: b"A".to_vec(), negative: probability, at_threshold: probability, positive: probability },
        alpha: ErrorBudget::new(1, 100).unwrap(), stream: 5,
        max_predictions: 8, max_prediction_age_ticks: 60_000,
    }).unwrap().with_stream_messages(stream).unwrap()
}

#[test]
fn forecast_files_pin_the_exact_original_raw_and_owned_predictors() {
    for owned in [false, true] {
        let root = Root::new(); let config = configured(&root);
        let path = fixture::write(&root, b"allow");
        install(&root, &path, owned, 60_000);
        let loaded = recipe::load(&path, &config, false).unwrap();
        assert!(loaded.generation.requires_forecast_text_stream_continuation());
        assert!(loaded.generation.allows_text_stream_continuation());
        let original = expected(loaded.stream, owned);
        if owned {
            let per_job = LearnedMonitorBudget { encoded_bytes: 1_048_576, probe_coordinates: 10_000,
                reconstruction_products: 1_000_000, materialized_values: 256, refinements: 64 };
            let lifetime = LearnedMonitorBudget { encoded_bytes: 8_388_608, probe_coordinates: 80_000,
                reconstruction_products: 8_000_000, materialized_values: 2048, refinements: 512 };
            let original = FileLearnedConsistencyConfig::new(original, 1, KvSide::Value,
                per_job, lifetime, 8_388_608).unwrap().with_owned_generation().unwrap()
                .with_pre_output_forecast().unwrap();
            assert_eq!(loaded.generation.required_owned_pre_output_forecast(), Some(&original));
            assert!(loaded.generation.required_pre_output_forecast().is_none());
        } else {
            let original = original.with_hosted_residual(1).unwrap().with_pre_output_forecast().unwrap();
            assert_eq!(loaded.generation.required_pre_output_forecast(), Some(&original));
            assert!(loaded.generation.required_owned_pre_output_forecast().is_none());
        }
        assert!(!config.store.exists());
    }
}

#[test]
fn forecast_coefficients_keep_signed_zero_and_accept_only_exact_finite_words() {
    let root = Root::new(); let config = configured(&root);
    let path = fixture::write(&root, b"allow"); let file = install(&root, &path, false, 60_000);
    let original = fs::read_to_string(&file).unwrap();
    let loaded = recipe::load(&path, &config, false).unwrap();
    let exact = loaded.generation.required_pre_output_forecast().unwrap();
    fs::write(&file, format!("  {original}\n")).unwrap();
    assert_eq!(recipe::load(&path, &config, false).unwrap().generation.required_pre_output_forecast(), Some(exact));
    fs::write(&file, original.replace("1065353216,2147483648", "1065353216,0")).unwrap();
    assert_ne!(recipe::load(&path, &config, false).unwrap().generation.required_pre_output_forecast(), Some(exact));
    for invalid in ["2139095040", "4286578688", "2143289344", "4294967296", "-1", "1.0"] {
        fs::write(&file, original.replace("\"bias_bits\":0", &format!("\"bias_bits\":{invalid}"))).unwrap();
        assert!(recipe::load(&path, &config, false).is_err(), "accepted {invalid}");
        assert!(!config.store.exists());
    }
}

#[test]
fn forecast_schema_probabilities_and_mode_fields_close_before_weight_intake() {
    for owned in [false, true] {
        let root = Root::new(); let config = configured(&root);
        let path = fixture::write(&root, b"allow"); let file = install(&root, &path, owned, 60_000);
        let original = fs::read_to_string(&file).unwrap();
        recipe::load(&path, &config, false).unwrap();
        // A missing later asset makes an accidental parser fall-through visible.
        fs::remove_file(root.0.join("weights.safetensors")).unwrap();
        for (old, replacement) in [
            ("fa.learned-forecast/1", "fa.learned-forecast/2"),
            ("\"domain\":77", "\"domain\":77,\"unknown\":true"),
            ("\"domain\":77", "\"domain\":77,\"domain\":77"),
            ("\"null\":32768", "\"null\":0"),
            ("\"alternative\":32768", "\"alternative\":65536"),
            ("\"numerator\":1", "\"numerator\":0"),
            ("\"denominator\":100", "\"denominator\":1"),
            ("\"event_prefix_hex\":\"41\"", "\"event_prefix_hex\":\"\""),
            ("\"event_prefix_hex\":\"41\"", "\"event_prefix_hex\":\"FF\""),
            ("\"max_predictions\":8", "\"max_predictions\":0"),
            ("\"max_predictions\":8", "\"max_predictions\":513"),
            ("\"max_prediction_age_ticks\":60000", "\"max_prediction_age_ticks\":0"),
            ("\"layer\":1", "\"layer\":0"),
        ] {
            assert!(original.contains(old));
            fs::write(&file, original.replacen(old, replacement, 1)).unwrap();
            let failure = recipe::load(&path, &config, false).err().unwrap();
            assert!(!failure.contains("No such file"), "reached missing weights: {failure}");
            assert!(!config.store.exists());
        }
        let invalid = if owned { original.replace("\"side\":\"value\"", "\"side\":\"residual\"") }
            else { original.replace("\"layer\":1", "\"layer\":1,\"side\":\"key\"") };
        fs::write(&file, invalid).unwrap();
        let failure = recipe::load(&path, &config, false).err().unwrap();
        assert!(!failure.contains("No such file")); assert!(!config.store.exists());
    }
}

#[test]
fn forecast_binding_refuses_changed_model_source_width_and_stream_before_store_creation() {
    for owned in [false, true] {
        let root = Root::new(); let config = configured(&root);
        let path = fixture::write(&root, b"allow"); let file = install(&root, &path, owned, 60_000);
        let original = fs::read_to_string(&file).unwrap();
        recipe::load(&path, &config, false).unwrap();
        for (old, replacement) in [
            ("\"tenant\":1", "\"tenant\":2"),
            ("\"model\":9", "\"model\":10"),
            ("\"model_generation\":1", "\"model_generation\":2"),
            ("\"tokenizer_generation\":1", "\"tokenizer_generation\":2"),
            ("\"profile_generation\":1", "\"profile_generation\":2"),
            ("\"layout_generation\":1", "\"layout_generation\":2"),
            ("\"layer\":1", "\"layer\":2"),
            ("[1065353216,2147483648]", "[1065353216]"),
            ("\"stream\":5", "\"stream\":6"),
            ("\"id\":9", "\"id\":10"),
            ("\"max_messages\":1", "\"max_messages\":2"),
            ("\"max_message_bytes\":64", "\"max_message_bytes\":63"),
            ("\"max_total_bytes\":64", "\"max_total_bytes\":65"),
        ] {
            assert!(original.contains(old));
            fs::write(&file, original.replacen(old, replacement, 1)).unwrap();
            assert!(recipe::load(&path, &config, false).is_err(), "accepted {replacement}");
            assert!(!config.store.exists());
        }
        // Both taps exist with identical width; only original source selection differs.
        let old = if owned { "\"tap\":3" } else { "\"tap\":4" };
        fs::write(&file, original.replace(old, "\"tap\":2")).unwrap();
        assert!(recipe::load(&path, &config, false).is_err()); assert!(!config.store.exists());
        if owned {
            fs::write(&file, original.replace("\"side\":\"value\"", "\"side\":\"key\"")
                .replace("\"tap\":3", "\"tap\":2")).unwrap();
            assert!(recipe::load(&path, &config, false).unwrap().generation.required_owned_pre_output_forecast().is_some());
        }
    }
}

#[test]
fn forecast_asset_bytes_share_the_original_aggregate_ledger_exactly() {
    let root = Root::new(); let config = configured(&root);
    let path = fixture::write(&root, b"allow"); let file = install(&root, &path, true, 60_000);
    let length = fs::metadata(&file).unwrap().len() as usize;
    let mut assets = Assets::new(length).unwrap();
    ForecastInput::load(&file, &mut assets).unwrap(); assert_eq!(assets.remaining(), 0);
    let mut short = Assets::new(length - 1).unwrap();
    assert!(ForecastInput::load(&file, &mut short).is_err());
    let names = ["fit-binding.json", "learned-monitor.json", "forecast.json", "model.json",
        "tokenizer.json", "sampling.json", "prompt.txt", "weights.safetensors", "fit.fakv"];
    let total: u64 = names.into_iter().map(|name| fs::metadata(root.0.join(name)).unwrap().len()).sum();
    let original = fs::read_to_string(&path).unwrap();
    fs::write(&path, original.replace("\"asset_bytes\":4194304", &format!("\"asset_bytes\":{total}"))).unwrap();
    recipe::load(&path, &config, false).unwrap(); assert!(!config.store.exists());
    fs::write(&path, original.replace("\"asset_bytes\":4194304", &format!("\"asset_bytes\":{}", total - 1))).unwrap();
    assert!(recipe::load(&path, &config, false).is_err()); assert!(!config.store.exists());
}

#[test]
fn forecast_file_limit_and_owned_work_limits_use_original_bounded_admission() {
    let root = Root::new(); let config = configured(&root);
    let path = fixture::write(&root, b"allow"); let file = install(&root, &path, true, 60_000);
    let original = fs::read_to_string(&file).unwrap();
    let mut exact = original.as_bytes().to_vec(); exact.resize(MAX_DATA_BYTES, b' ');
    fs::write(&file, &exact).unwrap();
    let mut assets = Assets::new(MAX_DATA_BYTES).unwrap();
    ForecastInput::load(&file, &mut assets).unwrap(); assert_eq!(assets.remaining(), 0);
    exact.push(b' '); fs::write(&file, exact).unwrap();
    // Extra aggregate allowance does not enlarge the independent artifact cap.
    assert!(ForecastInput::load(&file, &mut Assets::new(MAX_DATA_BYTES + 1).unwrap()).is_err());
    fs::write(&file, original.replacen("\"probe_coordinates\":10000",
        "\"probe_coordinates\":16777217", 1)).unwrap();
    assert!(recipe::load(&path, &config, false).is_err()); assert!(!config.store.exists());
    // A zero work allowance is the original explicit refusal policy, not an
    // implicit default or a new supply of forecast work at the file boundary.
    fs::write(&file, original.replacen("\"probe_coordinates\":10000", "\"probe_coordinates\":0", 1)).unwrap();
    assert!(recipe::load(&path, &config, false).unwrap().generation.required_owned_pre_output_forecast().is_some());
    assert!(!config.store.exists());
}

#[test]
fn forecast_source_binding_uses_original_gqa_cache_width_instead_of_hidden_width() {
    use fa_reference::action::consequence::activation::tensor::kv::decoder::{
        DecoderLayerWeights, DecoderModel, DecoderProfile, DecoderShape,
    };
    let profile = DecoderProfile::new(models::identity(9), DecoderShape { vocabulary: 4, hidden: 4,
        intermediate: 4, layers: 1, query_heads: 2, cache_heads: 1, context: 16 }, 0.00001, 10000.0).unwrap();
    let layer = DecoderLayerWeights { attention_norm: vec![1.0; 4], queries: vec![0.0; 16],
        keys: vec![0.0; 8], values: vec![0.0; 8], attention_output: vec![0.0; 16],
        feed_forward_norm: vec![1.0; 4], gate: vec![0.0; 16], up: vec![0.0; 16], down: vec![0.0; 16] };
    let model = DecoderModel::new(profile, vec![1.0; 16], vec![layer], vec![1.0; 4], vec![0.0; 16]).unwrap();
    assert_eq!(model.residual_contract(1).unwrap().dimensions(), 4);
    assert_eq!(model.cache_profile().layers()[&1].values().dimensions(), 2);
    for owned in [true, false] {
        let root = Root::new(); let path = fixture::write(&root, b"allow");
        let file = install(&root, &path, owned, 60_000);
        let original = fs::read_to_string(&file).unwrap();
        let stream = StreamProfile::new(9, 1, 1, 64, 64).unwrap();
        let load = || ForecastInput::load(&file, &mut Assets::new(MAX_DATA_BYTES).unwrap()).unwrap();
        // The same two coordinates bind the actual GQA K/V contract, while the
        // original residual has four coordinates and requires a different probe.
        assert_eq!(load().bind(&model, 5, stream).is_ok(), owned);
        fs::write(&file, original.replace("[1065353216,2147483648]", "[1065353216,2147483648,0,0]")).unwrap();
        assert_eq!(load().bind(&model, 5, stream).is_ok(), !owned);
    }
}

#[test]
fn forecast_recipe_version_is_explicit_and_legacy_versions_retain_their_contracts() {
    let root = Root::new(); let config = configured(&root);
    let path = fixture::write(&root, b"allow"); let original = fs::read_to_string(&path).unwrap();
    for version in 1..=4 {
        let recipe = original.replace("fa.learned-publication/1", &format!("fa.learned-publication/{version}"));
        let recipe = if version >= 2 { recipe.replacen("\"sidecar\":{", "\"sidecar\":{\"priority\":[],", 1)
            .replacen("\"native_review\":{", "\"native_review\":{\"round_ids\":[1],", 1) } else { recipe };
        fs::write(&path, &recipe).unwrap();
        let loaded = recipe::load(&path, &config, false).unwrap();
        assert_eq!(loaded.generation.allows_text_stream_continuation(), version == 4);
        assert!(!loaded.generation.requires_forecast_text_stream_continuation());
        assert!(loaded.generation.required_pre_output_forecast().is_none());
        assert!(loaded.generation.required_owned_pre_output_forecast().is_none());
        fs::write(&path, recipe.replacen("\"files\":{", "\"files\":{\"forecast\":\"/missing.json\",", 1)).unwrap();
        assert!(recipe::load(&path, &config, false).err().unwrap().contains("unsupported"));
        assert!(!config.store.exists());
    }
    fs::write(&path, original).unwrap(); install(&root, &path, false, 60_000);
    let v5 = fs::read_to_string(&path).unwrap();
    recipe::load(&path, &config, false).unwrap();
    fs::write(&path, v5.replace(&format!("\"forecast\":\"{}\",", root.0.join("forecast.json").display()), "")).unwrap();
    assert!(recipe::load(&path, &config, false).err().unwrap().contains("missing learned recipe field forecast"));
    assert!(!config.store.exists());
}
