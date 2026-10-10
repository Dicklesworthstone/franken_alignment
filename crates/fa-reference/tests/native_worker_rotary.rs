//! The real original worker process must pin static RoPE independently of its
//! checkpoint, then retain ordinary monitoring, budgets and commit/reveal.
#![forbid(unsafe_code)]
#![cfg(unix)]

#[path = "support/native_worker_assets.rs"]
mod assets;
#[path = "support/native_worker_v2_assets.rs"]
mod v2_assets;
#[path = "native_worker_rotary/fixture.rs"]
mod fixture;
#[path = "native_worker_rotary/process.rs"]
mod process;

use fa_reference::round::Verdict;
use fixture::{LINEAR, LLAMA3, NONE, configure};
use process::{exchange, no_vote};

#[test]
fn worker_v3_static_profiles_preserve_input_dependent_commit_and_reveal() {
    for scaling in [LINEAR, LLAMA3] {
        for (sharded, json) in [(false, false), (true, true)] {
            let mut fixture = assets::Fixture::new(false);
            configure(&mut fixture, scaling, sharded, json);
            if sharded { std::fs::remove_file(fixture.root.join("weights.safetensors")).unwrap(); }
            for (prompt, verdict) in [(b"?".as_slice(), Verdict::Allow), (b"!", Verdict::Deny)] {
                exchange(&fixture, prompt, verdict);
            }
        }
    }
}

#[test]
fn worker_v3_requires_explicit_rotary_and_preserves_legacy_manifest_contracts() {
    let legacy = assets::Fixture::new(false);
    exchange(&legacy, b"?", Verdict::Allow);
    let mut version_two = assets::Fixture::new(false);
    v2_assets::configure(&mut version_two, false, true);
    exchange(&version_two, b"!", Verdict::Deny);
    let mut explicit_none = assets::Fixture::new(false);
    configure(&mut explicit_none, NONE, false, true);
    exchange(&explicit_none, b"?", Verdict::Allow);

    for replacement in [
        r#"{"kind":"linear"}"#,
        r#"{"kind":"linear","factor":0.5}"#,
        r#"{"kind":"linear","factor":"4"}"#,
        r#"{"kind":"linear","factor":1e999}"#,
        r#"{"kind":"dynamic","factor":4}"#,
        r#"{"kind":"llama3","factor":8}"#,
        r#"{"kind":"none","factor":1}"#,
        "null",
    ] {
        let mut fixture = assets::Fixture::new(false);
        configure(&mut fixture, LINEAR, false, true);
        fixture.manifest = fixture.manifest.replace(LINEAR, replacement);
        fixture.save();
        // A malformed operator contract must refuse before any startup asset,
        // not merely fail eventually because a weight happens to be missing.
        std::fs::remove_file(fixture.root.join("salt.bin")).unwrap();
        std::fs::remove_file(fixture.root.join("config.json")).unwrap();
        let stderr = no_vote(&fixture, None);
        assert!(stderr.contains("invalid manifest field") || stderr.contains("refused configuration"), "{stderr}");
        assert!(!stderr.contains("operator file I/O"), "{stderr}");
    }
    for old_version in [None, Some("fa.native-worker/1"), Some("fa.native-worker/2")] {
        let mut fixture = assets::Fixture::new(false);
        configure(&mut fixture, LINEAR, false, true);
        fixture.manifest = match old_version {
            None => fixture.manifest.replace(&format!(",\"rotary\":{LINEAR}"), ""),
            Some(schema) => fixture.manifest.replace("fa.native-worker/3", schema),
        };
        fixture.save();
        let stderr = no_vote(&fixture, None);
        assert!(stderr.contains("invalid manifest field"), "{stderr}");
    }
}

#[test]
fn worker_v3_profile_substitution_refuses_before_weight_or_index_access() {
    for sharded in [false, true] {
        for (declared, substituted) in [
            (LINEAR, r#"{"kind":"linear","factor":5}"#),
            (LLAMA3, r#"{"kind":"llama3","factor":8,"low_freq_factor":1,"high_freq_factor":4,"original_max_position_embeddings":256}"#),
        ] {
            let mut fixture = assets::Fixture::new(false);
            configure(&mut fixture, declared, sharded, true);
            fixture.manifest = fixture.manifest.replace(declared, substituted);
            fixture.save();
            remove_weights(&fixture, sharded);
            // This fixture's Q/K are zero: even numerically equal observations
            // cannot excuse a changed independently retained profile parameter.
            let stderr = no_vote(&fixture, None);
            assert!(stderr.contains("Contract(Binding)"), "{stderr}");
            assert!(!stderr.contains("Io {"), "{stderr}");
        }
        // The otherwise identical matching profile reaches the actual missing
        // source. This separates numerical custody from an I/O-only refusal.
        let mut fixture = assets::Fixture::new(false);
        configure(&mut fixture, LINEAR, sharded, true);
        remove_weights(&fixture, sharded);
        let stderr = no_vote(&fixture, None);
        assert!(stderr.contains("Io {") && stderr.contains("Weight"), "{stderr}");
        assert!(!stderr.contains("Binding"), "{stderr}");
    }

    let mut fixture = assets::Fixture::new(false);
    configure(&mut fixture, LINEAR, false, false);
    let unscaled = fixture.policy.decoder_profile.clone()
        .with_rotary_scaling(fa_reference::action::consequence::activation::tensor::kv::decoder::RotaryScaling::None).unwrap();
    std::fs::write(fixture.root.join("tokenizer.bin"), fixture::tokenizer(&unscaled)).unwrap();
    remove_weights(&fixture, false);
    let stderr = no_vote(&fixture, None);
    assert!(stderr.contains("Tokenizer(Binding)"), "{stderr}");
}

#[test]
fn worker_v3_held_or_incomplete_output_and_spent_steps_never_become_votes() {
    for scaling in [LINEAR, LLAMA3] {
        for alarm in [false, true] {
            let mut fixture = assets::Fixture::new(alarm);
            configure(&mut fixture, scaling, true, true);
            if !alarm {
                fixture.manifest = fixture.manifest.replace("\"max_new_tokens\":2", "\"max_new_tokens\":1");
                fixture.save();
            }
            let stderr = no_vote(&fixture, Some(b"?"));
            assert!(stderr.contains(if alarm { "Held" } else { "TokenLimit" }), "{stderr}");
        }
    }
    let mut fixture = assets::Fixture::new(false);
    configure(&mut fixture, LINEAR, true, true);
    fixture.manifest = fixture.manifest.replace("\"steps\":10000", "\"steps\":1");
    fixture.save();
    let stderr = no_vote(&fixture, Some(b"?"));
    assert!(stderr.contains("StepLimit"), "{stderr}");
}

fn remove_weights(fixture: &assets::Fixture, sharded: bool) {
    std::fs::remove_file(fixture.root.join("weights.safetensors")).unwrap();
    if sharded {
        for name in ["weights.index.json", "registered-0.bin", "registered-1.bin"] {
            std::fs::remove_file(fixture.root.join(name)).unwrap();
        }
    }
}
