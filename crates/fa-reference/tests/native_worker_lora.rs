//! Original binary, private socketpair, loaded adapter, inference and commit/reveal.
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
mod fixture;
#[path = "native_worker_lora/process.rs"]
mod process;
use fa_reference::round::Verdict;
use fixture::configure;
use process::{exchange, exchange_frame, no_vote, no_vote_frame};

#[test]
fn worker_v4_adapter_changes_actual_independent_commit_reveal_for_rank_one_and_two() {
    for rank in [1, 2] {
        for json in [false, true] {
            let mut zero = assets::Fixture::new(false); configure(&mut zero, rank, true, json);
            exchange(&zero, b"?", Verdict::Allow); exchange(&zero, b"!", Verdict::Deny);
            let mut changed = assets::Fixture::new(false); configure(&mut changed, rank, false, json);
            // The same original base and packets reverse their actual verdicts
            // only when the nonzero imported down-projection adapter executes.
            exchange(&changed, b"?", Verdict::Deny); exchange(&changed, b"!", Verdict::Allow);
        }
    }
    let original = assets::Fixture::new(false);
    exchange_frame(&original, &assets::frame(b"?"), Verdict::Allow);
    let mut legacy = assets::Fixture::new(false);
    rotary_assets::configure(&mut legacy, rotary_assets::LINEAR, false, true);
    exchange_frame(&legacy, &assets::frame(b"!"), Verdict::Deny);
}

#[test]
fn worker_v4_identity_schema_and_single_base_contract_refuse_before_assets() {
    for defect in 0..5 {
        let mut fixture = assets::Fixture::new(false); configure(&mut fixture, 1, false, false);
        fixture.manifest = match defect {
            0 => fixture.manifest.replace(fixture::base_identity(),
                r#"{"tenant":1,"model":2,"model_generation":6,"tokenizer_generation":4,"profile_generation":5}"#),
            1 => format!("{}}}", fixture.manifest.rsplit_once(",\"adapter\":").unwrap().0),
            2 => fixture.manifest.replace("fa.native-worker/4", "fa.native-worker/3"),
            3 => {
                let path = v2_assets::quote(fixture.root.join("weights.safetensors").to_str().unwrap());
                let original = format!("\"weights\":{{\"kind\":\"single\",\"path\":{path}}}");
                let replaced = format!("\"weights\":{{\"kind\":\"sharded\",\"index\":{path},\"shards\":{{\"part.safetensors\":{path}}}}}");
                assert!(fixture.manifest.contains(&original)); fixture.manifest.replace(&original, &replaced)
            }
            _ => fixture.manifest.replace("\"merge_products\":4", "\"merge_products\":1073741825"),
        };
        fixture.save();
        std::fs::remove_file(fixture.root.join("salt.bin")).unwrap();
        std::fs::remove_file(fixture.root.join("config.json")).unwrap();
        let stderr = no_vote(&fixture, None);
        assert!(stderr.contains("invalid manifest field") || stderr.contains("refused configuration")
            || stderr.contains("startup input or work limit"), "{stderr}");
        assert!(!stderr.contains("operator file I/O"), "{stderr}");
    }
}

#[test]
fn worker_v4_adapter_and_old_tokenizer_refuse_before_both_weight_files_open() {
    for defect in 0..4 {
        let mut fixture = assets::Fixture::new(false); configure(&mut fixture, 1, false, false);
        match defect {
            0 | 1 => {
                let path = fixture.root.join("adapter_config.json");
                let value = std::fs::read_to_string(&path).unwrap();
                let value = if defect == 0 { value.replace("\"r\":1", "\"r\":0") }
                    else { value.replace("\"init_lora_weights\":true", "\"init_lora_weights\":\"pissa\"") };
                std::fs::write(path, value).unwrap();
            }
            2 => {
                fixture.manifest = fixture.manifest.replace("\"merge_products\":4", "\"merge_products\":3");
                fixture.save();
            }
            _ => {
                let old = fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderProfile::new(
                    fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderIdentity {
                        model_generation: 3, profile_generation: 5, ..fixture.policy.decoder_profile.identity()
                    }, fixture.policy.decoder_profile.shape(), fixture.policy.decoder_profile.epsilon(),
                    fixture.policy.decoder_profile.theta()).unwrap()
                    .with_rotary_scaling(fixture.policy.decoder_profile.rotary_scaling()).unwrap();
                std::fs::write(fixture.root.join("tokenizer.bin"), rotary_assets::tokenizer(&old)).unwrap();
            }
        }
        for name in ["weights.safetensors", "adapter.safetensors"] {
            std::fs::remove_file(fixture.root.join(name)).unwrap();
        }
        let stderr = no_vote(&fixture, None);
        assert!(stderr.contains(if defect == 3 { "Tokenizer(Binding)" } else if defect == 2 {
            "Adapter(Limit)" } else { "Adapter(Configuration" }), "{stderr}");
        assert!(!stderr.contains("Io {"), "{stderr}");
    }
    let mut matching = assets::Fixture::new(false); configure(&mut matching, 1, false, false);
    std::fs::remove_file(matching.root.join("weights.safetensors")).unwrap();
    let stderr = no_vote(&matching, None);
    assert!(stderr.contains("Io { asset: Weights") && !stderr.contains("Binding"), "{stderr}");
}

#[test]
fn worker_v4_requires_the_adapted_monitor_and_the_exact_registered_input_profile() {
    let mut fixture = assets::Fixture::new(false); configure(&mut fixture, 1, false, true);
    let path = fixture.root.join("monitor.json");
    let old = std::fs::read_to_string(&path).unwrap()
        .replace("\"model_generation\":6", "\"model_generation\":3")
        .replace("\"profile_generation\":7", "\"profile_generation\":5");
    std::fs::write(path, old).unwrap();
    let stderr = no_vote(&fixture, None);
    assert!(stderr.contains("Binding"), "{stderr}");
    let mut matching = assets::Fixture::new(false); configure(&mut matching, 1, false, true);
    let stderr = no_vote_frame(&matching, Some(&assets::frame(b"?")));
    assert!(stderr.contains("Contract(Binding)"), "{stderr}");
    exchange(&matching, b"?", Verdict::Deny);
}

#[test]
fn worker_v4_base_and_adapter_share_the_original_weight_byte_allowance() {
    for enough in [false, true] {
        let mut fixture = assets::Fixture::new(false); configure(&mut fixture, 1, false, true);
        let total = ["weights.safetensors", "adapter.safetensors"].into_iter()
            .map(|name| std::fs::metadata(fixture.root.join(name)).unwrap().len()).sum::<u64>();
        fixture.manifest = fixture.manifest.replace("\"weight_bytes\":1048576",
            &format!("\"weight_bytes\":{}", total + u64::from(enough)));
        fixture.save();
        if enough { exchange(&fixture, b"?", Verdict::Deny); }
        else {
            let stderr = no_vote(&fixture, None);
            assert!(stderr.contains("Adapter(Weights(Refused(Limit)))"), "{stderr}");
        }
    }
}

#[test]
fn worker_v4_held_incomplete_or_spent_step_output_never_becomes_a_vote() {
    for defect in 0..3 {
        let mut fixture = assets::Fixture::new(defect == 0); configure(&mut fixture, 2, false, true);
        if defect == 1 { fixture.manifest = fixture.manifest.replace("\"max_new_tokens\":2", "\"max_new_tokens\":1"); }
        if defect == 2 { fixture.manifest = fixture.manifest.replace("\"steps\":10000", "\"steps\":1"); }
        fixture.save();
        let stderr = no_vote(&fixture, Some(b"?"));
        assert!(stderr.contains(match defect { 0 => "Held", 1 => "TokenLimit", _ => "StepLimit" }), "{stderr}");
    }
}
