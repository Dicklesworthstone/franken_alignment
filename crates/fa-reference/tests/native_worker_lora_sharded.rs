//! Registered base shards, one actual static adapter, and the original worker.
#![forbid(unsafe_code)]
#![cfg(unix)]
#[allow(dead_code)]
#[path = "support/native_worker_assets.rs"]
mod assets;
#[allow(dead_code)]
#[path = "support/native_worker_v2_assets.rs"]
mod v2_assets;
#[allow(dead_code)]
#[path = "native_worker_rotary/fixture.rs"]
mod rotary_assets;
#[path = "native_worker_lora/fixture.rs"]
mod lora_assets;
#[path = "native_worker_lora_sharded/fixture.rs"]
mod fixture;
#[path = "native_worker_lora/process.rs"]
mod process;
#[path = "native_worker_lora_sharded/process.rs"]
mod process_tests;

use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::{WeightError, TensorIssue};
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::lora::{LoraError, LoraMergeBudget};
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::reader::{WeightReadBudget, WeightReadError};
use fa_reference::action::consequence::oversight::helper_client::native::{NativeEvaluator, NativeEvaluationStatus};
use fa_reference::action::consequence::oversight::helper_client::native::bootstrap::NativeBootstrapError;
use fa_reference::action::consequence::oversight::helper_client::native::bootstrap::files::{NativeAsset, NativeAssetReadBudget, NativeFileBootstrapError};
use fa_reference::action::consequence::oversight::helper_client::native::bootstrap::files::lora::NativeLoraFileBootstrapError;
use fa_reference::action::consequence::oversight::helper_client::native::bootstrap::files::sharded::NativeShardFileBootstrapError;
use fa_reference::action::consequence::oversight::helper_workers::wire::decode_request;
use fa_reference::round::Verdict;
use fa_reference::Error;
use fixture::{Fixture, assets_budget, weights_budget, header_bytes};
use std::fs;

#[test]
fn sharded_adapter_receipts_and_actual_verdicts_retain_original_owners() {
    for rank in [1, 2] {
        for json in [false, true] {
            for zero in [false, true] {
                let fixture = Fixture::new(rank, zero, json, false);
                let mut assets = assets_budget(); let mut weights = weights_budget();
                let mut products = LoraMergeBudget::new(4 * rank as u64).unwrap();
                let (mut loaded, receipt) = NativeEvaluator::from_llama_lora_shard_files(
                    fixture.request(), &mut assets, &mut weights, &mut products, fixture.format).unwrap();
                assert_eq!(receipt.base.configuration.profile().identity(), fixture::base_identity());
                assert_eq!(receipt.adapter.adapted_profile, fixture.inner.policy.decoder_profile);
                assert_eq!(receipt.adapter.work.scalar_products, 4 * rank as u64);
                assert_eq!(receipt.base.weights.file_bytes, fixture.base_bytes());
                assert_eq!(receipt.base.weights.index_bytes, fs::metadata(&fixture.index).unwrap().len() as usize);
                assert_eq!(weights.usage().bytes_read, fixture.weight_bytes());
                assert_eq!(assets.usage().bytes_read, fixture.asset_bytes());
                assert_eq!(products.reserved_products(), 4 * rank as u64);
                assert_eq!(loaded.status(), NativeEvaluationStatus::AwaitingInput);
                assert_eq!(loaded.position(), 0); assert_eq!(loaded.sampled_draws(), 0);
                // Startup owns immutable numerical data: no source path is
                // consulted during the original review/inference afterwards.
                for path in fixture.all_asset_paths() { fs::remove_file(path).unwrap(); }
                let input = decode_request(&fixture::frame(b"?")).unwrap();
                assert_eq!(loaded.evaluate(&input), Ok(if zero { Verdict::Allow } else { Verdict::Deny }));
                assert_eq!(loaded.report().unwrap().prompt().source(), b"?");
                assert_eq!(loaded.report().unwrap().bytes().unwrap(), if zero { b"allow".as_slice() } else { b"deny" });
                assert!(loaded.work().decoder.scalar_products().unwrap() > 0);
                assert_eq!(loaded.sampled_draws(), 2);
            }
        }
    }
}

#[test]
fn sharded_adapter_identity_and_config_preflight_never_open_weight_sources() {
    for defect in 0..5 {
        let fixture = Fixture::new(1, false, false, false);
        let mut request = fixture.request();
        match defect {
            0 => request.base_identity.model_generation += 3,
            1 => fs::write(&fixture.adapter_config, b"{}").unwrap(),
            2 => {},
            3 => {
                let old = fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderProfile::new(
                    fixture::base_identity(), fixture.inner.policy.decoder_profile.shape(),
                    fixture.inner.policy.decoder_profile.epsilon(), fixture.inner.policy.decoder_profile.theta()).unwrap()
                    .with_rotary_scaling(fixture.inner.policy.decoder_profile.rotary_scaling()).unwrap();
                fs::write(&fixture.tokenizer, rotary_assets::tokenizer(&old)).unwrap();
            }
            _ => fs::write(&fixture.configuration, b"{}").unwrap(),
        }
        for path in fixture.shards.values().chain(std::iter::once(&fixture.adapter_weights)) { fs::remove_file(path).unwrap(); }
        fs::remove_file(&fixture.index).unwrap();
        let mut assets = assets_budget(); let mut weights = weights_budget();
        let mut products = LoraMergeBudget::new(if defect == 2 { 3 } else { 4 }).unwrap();
        let error = NativeEvaluator::from_llama_lora_shard_files(request,
            &mut assets, &mut weights, &mut products, fixture.format).unwrap_err();
        match defect {
            0 => { assert_eq!(error, NativeLoraFileBootstrapError::Adapter(LoraError::Identity)); assert_eq!(assets.usage().read_calls, 0); }
            1 => assert!(matches!(error, NativeLoraFileBootstrapError::Adapter(LoraError::Configuration { .. }))),
            2 => assert_eq!(error, NativeLoraFileBootstrapError::Adapter(LoraError::Limit)),
            3 => assert_eq!(error, NativeLoraFileBootstrapError::File(NativeFileBootstrapError::Bootstrap(NativeBootstrapError::Tokenizer(Error::Binding)))),
            _ => assert!(matches!(error, NativeLoraFileBootstrapError::File(NativeFileBootstrapError::Bootstrap(NativeBootstrapError::Checkpoint(_))))),
        }
        assert_eq!(weights.usage().read_calls, 0); assert_eq!(products.reserved_products(), 0);
    }
}

#[test]
fn sharded_adapter_complete_roster_and_all_regular_files_precede_scalar_ingestion() {
    for defect in 0..6 {
        let fixture = Fixture::new(1, false, true, false);
        let mut shards = fixture.shards.clone();
        match defect {
            0 => { shards.remove("base-1.safetensors"); },
            1 => { shards.insert("unregistered.safetensors".into(), fixture.inner.root.join("never-open")); },
            2 => { fs::remove_file(&shards["base-1.safetensors"]).unwrap(); },
            3 => { fs::remove_file(&fixture.adapter_weights).unwrap(); },
            4 => {
                let text = fs::read_to_string(&fixture.index).unwrap().replace("base-1.safetensors", "../base-1.safetensors");
                fs::write(&fixture.index, text).unwrap();
            }
            _ => { fs::remove_file(&shards["base-1.safetensors"]).unwrap(); fs::create_dir(&shards["base-1.safetensors"]).unwrap(); },
        }
        let mut request = fixture.request(); request.base.files.shards = &shards;
        let mut weights = weights_budget(); let mut products = LoraMergeBudget::new(4).unwrap();
        let error = NativeEvaluator::from_llama_lora_shard_files(request,
            &mut assets_budget(), &mut weights, &mut products, fixture.format).unwrap_err();
        match defect {
            0 | 1 | 4 => assert!(matches!(error, NativeLoraFileBootstrapError::Sharded(NativeShardFileBootstrapError::Weights(WeightReadError::Refused(_))))),
            3 => assert!(matches!(error, NativeLoraFileBootstrapError::File(NativeFileBootstrapError::Io { asset: NativeAsset::AdapterWeights, .. }))),
            _ => assert!(matches!(error, NativeLoraFileBootstrapError::Sharded(NativeShardFileBootstrapError::Shard { .. }))),
        }
        assert_eq!(weights.usage().read_calls, 0); assert_eq!(products.reserved_products(), 0);
    }
    let fixture = Fixture::new(1, false, true, false);
    assert!(NativeEvaluator::from_llama_lora_shard_files(fixture.request(), &mut assets_budget(),
        &mut weights_budget(), &mut LoraMergeBudget::new(4).unwrap(), fixture.format).is_ok());
}

#[test]
fn sharded_adapter_late_base_header_preserves_prior_headers_and_no_merge_charge() {
    let fixture = Fixture::new(1, false, false, false);
    let path = &fixture.shards["base-1.safetensors"];
    let original = fs::read(path).unwrap(); let end = header_bytes(&original);
    let mut corrupted = original.clone();
    let header = String::from_utf8(original[8..end].to_vec()).unwrap().replacen("\"F32\"", "\"U32\"", 1);
    corrupted[8..end].copy_from_slice(header.as_bytes()); fs::write(path, corrupted).unwrap();
    let mut weights = weights_budget(); let mut assets = assets_budget(); let mut products = LoraMergeBudget::new(4).unwrap();
    assert!(matches!(NativeEvaluator::from_llama_lora_shard_files(fixture.request(), &mut assets,
        &mut weights, &mut products, fixture.format), Err(NativeLoraFileBootstrapError::Sharded(
            NativeShardFileBootstrapError::Weights(WeightReadError::Refused(WeightError::Tensor { issue: TensorIssue::Encoding, .. }))))));
    let headers = fixture.shards.values().map(|path| header_bytes(&fs::read(path).unwrap())).sum::<usize>();
    assert_eq!(weights.usage().bytes_read, headers); assert_eq!(products.reserved_products(), 0);
    let spent_assets = assets.usage(); fs::write(path, original).unwrap();
    let (_, receipt) = NativeEvaluator::from_llama_lora_shard_files(fixture.request(), &mut assets,
        &mut weights, &mut products, fixture.format).unwrap();
    assert_eq!(weights.usage().bytes_read, headers + fixture.weight_bytes());
    assert_eq!(assets.usage().bytes_read, spent_assets.bytes_read + fixture.asset_bytes());
    assert_eq!(products.reserved_products(), receipt.adapter.work.scalar_products);
}

#[test]
fn sharded_adapter_original_asset_byte_read_call_and_aggregate_limits_have_positive_twins() {
    let fixture = Fixture::new(1, false, false, false);
    let mut complete_weights = weights_budget();
    NativeEvaluator::from_llama_lora_shard_files(fixture.request(), &mut assets_budget(),
        &mut complete_weights, &mut LoraMergeBudget::new(4).unwrap(), fixture.format).unwrap();
    let reads = complete_weights.usage().read_calls;
    for dimension in 0..5 {
        for shortage in [0, 1] {
            let mut request = fixture.request();
            if dimension == 0 { request.base.limits.weight_bytes = fixture.base_bytes() - shortage; }
            if dimension == 1 { request.base.index_bytes = fs::metadata(&fixture.index).unwrap().len() as usize - shortage; }
            let mut assets = NativeAssetReadBudget::for_shards(if dimension == 2 {
                fixture.asset_bytes() + 1 - shortage } else { 1_048_576 }, 4096).unwrap();
            let mut weights = WeightReadBudget::new(if dimension == 3 {
                fixture.weight_bytes() + 1 - shortage } else { 1_048_576 },
                if dimension == 4 { reads - shortage } else { 4096 }).unwrap();
            let mut products = LoraMergeBudget::new(4).unwrap();
            let result = NativeEvaluator::from_llama_lora_shard_files(request, &mut assets,
                &mut weights, &mut products, fixture.format);
            assert_eq!(result.is_ok(), shortage == 0, "dimension {dimension}: {result:?}");
            if shortage == 1 {
                assert_eq!(products.reserved_products(), 0);
                if dimension <= 2 { assert_eq!(weights.usage().read_calls, 0); }
                if dimension == 4 {
                    assert_eq!(weights.usage().read_calls, reads - 1);
                    let spent = weights.usage();
                    assert!(NativeEvaluator::from_llama_lora_shard_files(fixture.request(), &mut assets_budget(),
                        &mut weights, &mut products, fixture.format).is_err());
                    assert_eq!(weights.usage(), spent);
                }
            }
        }
    }
}

#[test]
fn sharded_adapter_late_adapter_and_monitor_failures_keep_spent_work_without_refilling() {
    for defect in 0..4 {
        let fixture = Fixture::new(1, false, false, false);
        let mut adapter = fs::read(&fixture.adapter_weights).unwrap(); let end = header_bytes(&adapter);
        match defect {
            0 => {
                let header = String::from_utf8(adapter[8..end].to_vec()).unwrap().replacen("\"F32\"", "\"U32\"", 1);
                adapter[8..end].copy_from_slice(header.as_bytes()); fs::write(&fixture.adapter_weights, adapter).unwrap();
            }
            1 => { adapter.push(0); fs::write(&fixture.adapter_weights, adapter).unwrap(); },
            2 => {
                // A and B remain finite; the original scaled merge overflows.
                let last = adapter.len() - 4; adapter[last..].copy_from_slice(&f32::MAX.to_le_bytes());
                fs::write(&fixture.adapter_weights, adapter).unwrap();
            }
            _ => fs::write(&fixture.monitoring, b"{}").unwrap(),
        }
        let mut assets = assets_budget(); let mut weights = weights_budget(); let mut products = LoraMergeBudget::new(4).unwrap();
        let error = NativeEvaluator::from_llama_lora_shard_files(fixture.request(), &mut assets,
            &mut weights, &mut products, fixture.format).unwrap_err();
        match defect {
            0 => {
                assert!(matches!(error, NativeLoraFileBootstrapError::Adapter(LoraError::Weights(WeightReadError::Refused(WeightError::Tensor { issue: TensorIssue::Encoding, .. })))));
                assert_eq!(weights.usage().bytes_read, fixture.base_bytes() + end);
            }
            1 => assert!(matches!(error, NativeLoraFileBootstrapError::Adapter(LoraError::Weights(_)))),
            2 => assert!(matches!(error, NativeLoraFileBootstrapError::Adapter(LoraError::Model(Error::Overflow)))),
            _ => assert!(matches!(error, NativeLoraFileBootstrapError::File(NativeFileBootstrapError::Bootstrap(NativeBootstrapError::Sampling(_))))),
        }
        assert_eq!(products.reserved_products(), if defect <= 1 { 0 } else { 4 });
        assert!(weights.usage().bytes_read >= fixture.base_bytes());
        if defect >= 2 {
            let spent = weights.usage();
            assert_eq!(NativeEvaluator::from_llama_lora_shard_files(fixture.request(), &mut assets,
                &mut weights, &mut products, fixture.format).unwrap_err(), NativeLoraFileBootstrapError::Adapter(LoraError::Limit));
            assert_eq!(weights.usage(), spent); assert_eq!(products.reserved_products(), 4);
        }
    }
}
