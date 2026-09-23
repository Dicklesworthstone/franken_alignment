use super::*;
use fa_reference::action::consequence::oversight::helper_client::native::NativeEvaluationStatus;
use fa_reference::action::consequence::oversight::helper_client::native::bootstrap::files::sharded::MAX_SHARDED_ASSET_READ_BYTES;
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::shards::MAX_WEIGHT_SHARDS;
use fa_reference::round::Verdict;

fn weight_budget() -> WeightReadBudget { WeightReadBudget::new(1_048_576, 4096).unwrap() }

#[test]
fn worker_v2_every_declared_layout_and_format_loads_the_original_evaluator() {
    for sharded in [false, true] {
        for json in [false, true] {
            let mut fixture = assets::Fixture::new(false); v2_assets::configure(&mut fixture, sharded, json);
            let manifest = Manifest::parse(fixture.manifest.as_bytes()).unwrap();
            assert_eq!(manifest.policy, fixture.policy); assert_eq!(manifest.stream, 12);
            assert_eq!(manifest.tokenizer_format, if json { NativeTokenizerFormat::HuggingFaceRawByteLevel } else { NativeTokenizerFormat::NativeArchive });
            assert_eq!(matches!(&manifest.checkpoint, CheckpointFiles::Sharded { .. }), sharded);
            let mut auxiliary = manifest.asset_budget().unwrap(); let mut weights = weight_budget();
            let mut worker = manifest.load(&mut auxiliary, &mut weights).unwrap();
            assert_eq!(worker.status(), NativeEvaluationStatus::AwaitingInput);
            assert_eq!(worker.position(), 0); assert_eq!(worker.sampled_draws(), 0);
            assert_eq!(worker.evaluate(&assets::input(b"!")), Ok(Verdict::Deny));
            assert_eq!(worker.sampled_draws(), 2); assert!(weights.usage().bytes_read > 0);
        }
    }
}

#[test]
fn worker_v2_legacy_manifest_stays_native_single_without_optional_defaults() {
    let fixture = assets::Fixture::new(false);
    let manifest = Manifest::parse(fixture.manifest.as_bytes()).unwrap();
    assert_eq!(manifest.tokenizer_format, NativeTokenizerFormat::NativeArchive);
    assert!(matches!(&manifest.checkpoint, CheckpointFiles::Single(_)));
    let mut worker = manifest.load(&mut manifest.asset_budget().unwrap(), &mut weight_budget()).unwrap();
    assert_eq!(worker.evaluate(&assets::input(b"?")), Ok(Verdict::Allow));
    assert!(Manifest::parse(fixture.manifest.replace("fa.native-worker/1", "fa.native-worker/2").as_bytes()).is_err());
    assert!(Manifest::parse(fixture.manifest.replace("\"stream\":12", "\"stream\":12,\"tokenizer_format\":\"native-archive\"").as_bytes()).is_err());
}

#[test]
fn worker_v2_unknown_conflicting_and_missing_format_or_source_fields_refuse() {
    let mut fixture = assets::Fixture::new(false); v2_assets::configure(&mut fixture, true, false);
    for (from, to) in [
        ("\"tokenizer_format\":\"native-archive\"", "\"tokenizer_format\":null"),
        ("\"tokenizer_format\":\"native-archive\"", "\"tokenizer_format\":\"auto\""),
        (",\"tokenizer_format\":\"native-archive\"", ""),
        ("\"kind\":\"sharded\"", "\"kind\":\"single\""),
        ("\"kind\":\"sharded\"", "\"kind\":\"sharded\",\"path\":\"/unused\""),
        ("\"kind\":\"sharded\"", "\"kind\":\"sharded\",\"kind\":\"sharded\""),
        ("\"kind\":\"sharded\"", "\"kind\":\"remote\""),
        ("\"shards\":", "\"unknown_shards\":"),
        ("fa.native-worker/2", "fa.native-worker/3"),
    ] {
        assert!(fixture.manifest.contains(from));
        assert!(Manifest::parse(fixture.manifest.replacen(from, to, 1).as_bytes()).is_err(), "{from}");
    }
}

#[test]
fn worker_v2_shard_map_bounds_paths_and_duplicates_are_checked_before_loading() {
    let mut fixture = assets::Fixture::new(false); v2_assets::configure(&mut fixture, true, false);
    for key in ["registered-0.bin", "weights.index.json"] {
        let name = v2_assets::quote(fixture.root.join(key).to_str().unwrap());
        assert!(fixture.manifest.contains(&name));
        for bad in ["\"relative\"", "\"/bad\\u0000path\"", "null"] {
            assert!(Manifest::parse(fixture.manifest.replacen(&name, bad, 1).as_bytes()).is_err());
        }
    }
    for count in [0, MAX_WEIGHT_SHARDS, MAX_WEIGHT_SHARDS + 1] {
        let entries = (0..count).map(|i| format!("\"part-{i}.safetensors\":\"/registered\"")).collect::<Vec<_>>().join(",");
        let source = format!("{{\"kind\":\"sharded\",\"index\":\"/index\",\"shards\":{{{entries}}}}}");
        let value = strict_json::parse(source.as_bytes(), Limits::default()).unwrap();
        assert_eq!(CheckpointFiles::parse(&value).is_ok(), count == MAX_WEIGHT_SHARDS);
    }
    let source = b"{\"kind\":\"sharded\",\"index\":\"/index\",\"shards\":{\"\":\"/a\"}}";
    assert!(CheckpointFiles::parse(&strict_json::parse(source, Limits::default()).unwrap()).is_err());
}

#[test]
fn worker_v2_tokenizer_preflight_precedes_missing_weights_and_retains_spent_budgets() {
    for sharded in [false, true] {
        let mut fixture = assets::Fixture::new(false); v2_assets::configure(&mut fixture, sharded, true);
        std::fs::write(fixture.root.join("tokenizer.bin"), b"{}").unwrap();
        std::fs::remove_file(fixture.root.join("weights.safetensors")).unwrap();
        if sharded { std::fs::remove_file(fixture.root.join("weights.index.json")).unwrap(); }
        let manifest = Manifest::parse(fixture.manifest.as_bytes()).unwrap();
        let mut auxiliary = manifest.asset_budget().unwrap(); let mut weights = weight_budget();
        let error = manifest.load(&mut auxiliary, &mut weights).unwrap_err();
        assert!(error.to_string().contains("Tokenizer"), "{error}");
        assert_eq!(weights.usage().read_calls, 0); assert!(auxiliary.usage().read_calls > 0);
        let spent = auxiliary.usage();
        assert!(manifest.load(&mut auxiliary, &mut weights).is_err());
        assert!(auxiliary.usage().read_calls > spent.read_calls);
        assert_eq!(weights.usage().read_calls, 0);
    }
}

#[test]
fn worker_v2_only_sharded_manifests_receive_the_index_aware_asset_ceiling() {
    let mut fixture = assets::Fixture::new(false); v2_assets::configure(&mut fixture, true, false);
    let mut manifest = Manifest::parse(fixture.manifest.as_bytes()).unwrap();
    manifest.asset_bytes = MAX_SHARDED_ASSET_READ_BYTES;
    assert!(manifest.asset_budget().is_ok());
    manifest.checkpoint = CheckpointFiles::Single(fixture.root.join("weights.safetensors"));
    assert!(manifest.asset_budget().is_err());
}
