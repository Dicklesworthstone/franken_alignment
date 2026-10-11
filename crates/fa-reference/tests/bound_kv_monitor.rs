//! Exact trained artifact provisioning through original model and policy APIs.
#[path = "support/bound_kv_monitor.rs"]
#[allow(dead_code, unused_imports)]
mod fixture;
use fixture::*;
use fa_reference::action::consequence::activation::{
    monitor::{MonitorOutcome, learned::{LearnedRefinementMonitor, model::LearnedModelMonitor}},
    probe::training::decoder::kv::{KvMonitorArtifact, MAX_KV_MONITOR_ARTIFACT_BYTES,
        MAX_KV_MONITOR_JSON_BYTES},
    tensor::kv::{decoder::{monitoring::LearnedDecoderPolicy,
        sampling::monitored::{GenerationBudget, GenerationStatus, GenerationStop}},
        model::learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy}},
};
use fa_reference::Error;
use std::collections::BTreeMap;

#[test]
fn original_trained_artifact_verifies_and_releases_only_the_audited_sample() {
    let report = campaign();
    assert!(report.accepted());
    let model = report.model();
    let tokenizer = tokenizer(model);
    let codec = codec(model);
    let settings = settings(model);
    let policy = report.policy(codec.clone(), &settings).unwrap();
    let bytes = report.monitor_artifact(&tokenizer, &codec, &settings,
        MAX_KV_MONITOR_ARTIFACT_BYTES).unwrap();
    let artifact = KvMonitorArtifact::decode(&bytes, bytes.len()).unwrap();
    artifact.verify(model, &tokenizer, &policy).unwrap();
    assert_eq!(artifact.monitor_json(), report.monitor_json(&codec, &settings,
        MAX_KV_MONITOR_JSON_BYTES).unwrap());
    for seed in [4, 5] {
        let mut run = model.monitored_generation(2000, 2001, spec(seed), policy.clone(),
            GenerationBudget::default()).unwrap();
        assert_eq!(run.position(), 0);
        assert_eq!(run.work().admitted_tokens, 0);
        assert!(run.advance(0).unwrap().audit().complete_quiet());
        let next = run.advance(1).unwrap();
        if seed == 4 {
            assert_eq!(next.sample().unwrap().token, 0);
            assert!(next.audit().complete_quiet());
            let stop = run.advance(2).unwrap();
            assert!(stop.audit().complete_quiet());
            assert_eq!(stop.sample().unwrap().token, 256);
            assert_eq!(run.status(), GenerationStatus::Finished(GenerationStop::StopToken(256)));
        } else {
            assert_eq!(next.status(), GenerationStatus::Held(MonitorOutcome::Alarm));
            assert!(next.sample().is_none());
            assert_eq!(run.accepted_tokens(), &[5]);
        }
    }
}

#[test]
fn same_labels_and_identical_case_logits_do_not_hide_an_unused_weight_or_signed_zero() {
    let report = campaign();
    let model = report.model();
    let tokenizer = tokenizer(model);
    let codec = codec(model);
    let settings = settings(model);
    let policy = report.policy(codec.clone(), &settings).unwrap();
    let bytes = report.monitor_artifact(&tokenizer, &codec, &settings,
        MAX_KV_MONITOR_ARTIFACT_BYTES).unwrap();
    let artifact = KvMonitorArtifact::decode(&bytes, bytes.len()).unwrap();
    for changed in [changed_unused_weight(), changed_signed_zero()] {
        assert_eq!(changed.profile(), model.profile());
        assert_eq!(changed.cache_profile(), model.cache_profile());
        for case in cases() {
            let expected = model.recompute(case.origin.task, &case.tokens, inference()).unwrap();
            let actual = changed.recompute(case.origin.task, &case.tokens, inference()).unwrap();
            assert_eq!(actual.logits().unwrap(), expected.logits().unwrap());
            assert_eq!(actual.cache_image().unwrap().encode().unwrap(),
                expected.cache_image().unwrap().encode().unwrap());
        }
        assert_eq!(artifact.verify(&changed, &tokenizer, &policy), Err(Error::Binding));
    }
    artifact.verify(model, &tokenizer, &policy).unwrap();
}

#[test]
fn exact_tokenizer_vocabulary_merge_order_and_named_controls_are_bound() {
    let report = campaign();
    let model = report.model();
    let tokenizer = tokenizer(model);
    let codec = codec(model);
    let settings = settings(model);
    let policy = report.policy(codec.clone(), &settings).unwrap();
    let bytes = report.monitor_artifact(&tokenizer, &codec, &settings,
        MAX_KV_MONITOR_ARTIFACT_BYTES).unwrap();
    let artifact = KvMonitorArtifact::decode(&bytes, bytes.len()).unwrap();
    for variant in 1..=3 {
        let changed = changed_tokenizer(model, variant);
        assert_eq!(changed.profile(), tokenizer.profile());
        assert_ne!(changed.to_bytes().unwrap(), tokenizer.to_bytes().unwrap());
        assert_eq!(artifact.verify(model, &changed, &policy), Err(Error::Binding));
    }
    let changed = changed_tokenizer(model, 2);
    use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::TokenizationBudget;
    assert_eq!(tokenizer.encode(b"abc", TokenizationBudget::default()).unwrap().tokens(), &[257, 99]);
    assert_eq!(changed.encode(b"abc", TokenizationBudget::default()).unwrap().tokens(), &[97, 258]);
    artifact.verify(model, &tokenizer, &policy).unwrap();
}

#[test]
fn same_codec_labels_do_not_cover_changed_basis_or_declared_fit_origin() {
    let report = campaign();
    let model = report.model();
    let tokenizer = tokenizer(model);
    let codec = codec(model);
    let settings = settings(model);
    let bytes = report.monitor_artifact(&tokenizer, &codec, &settings,
        MAX_KV_MONITOR_ARTIFACT_BYTES).unwrap();
    let artifact = KvMonitorArtifact::decode(&bytes, bytes.len()).unwrap();
    let image = model.recompute(1000, &[0, 0], inference()).unwrap().cache_image().unwrap();
    let changed_basis = LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(1010, image)]), FitBudget::default()).unwrap();
    let changed_origin = codec_from(model, 1000, 1011);
    for changed in [changed_basis, changed_origin] {
        assert_eq!(changed.profile(), codec.profile());
        assert_eq!(changed.policy().id(), codec.policy().id());
        assert_eq!(changed.policy().generation(), codec.policy().generation());
        let policy = report.policy(changed, &settings).unwrap();
        assert_eq!(artifact.verify(model, &tokenizer, &policy), Err(Error::Binding));
    }
    artifact.verify(model, &tokenizer, &report.policy(codec, &settings).unwrap()).unwrap();
}

#[test]
fn public_verification_checks_json_against_exact_independent_probes_and_caps() {
    let report = campaign();
    let model = report.model();
    let tokenizer = tokenizer(model);
    let codec = codec(model);
    let settings = settings(model);
    let policy = report.policy(codec.clone(), &settings).unwrap();
    let bytes = report.monitor_artifact(&tokenizer, &codec, &settings,
        MAX_KV_MONITOR_ARTIFACT_BYTES).unwrap();
    let artifact = KvMonitorArtifact::decode(&bytes, bytes.len()).unwrap();
    let mut caps = settings.clone();
    caps.inference.scalar_products -= 1;
    assert_eq!(artifact.verify(model, &tokenizer, &report.policy(codec.clone(), &caps).unwrap()),
        Err(Error::Binding));
    let taps = report.taps().iter().map(|(tap, result)| {
        let probe = result.calibration().fitted().probe(1.0).unwrap();
        (*tap, LearnedRefinementMonitor::new(vec![probe], settings.taps[tap]).unwrap())
    }).collect();
    let monitor = LearnedModelMonitor::new(model.cache_profile().clone(), taps, settings.audit).unwrap();
    let changed = LearnedDecoderPolicy::new(codec, monitor, settings.retention,
        settings.preparation, settings.inference).unwrap();
    assert_eq!(artifact.verify(model, &tokenizer, &changed), Err(Error::Binding));

    // JSON-only mutation leaves exact binary model/policy binding untouched.
    // Public verification itself must refuse, not rely on the command's parser.
    let needle = b"\"threshold\":0";
    let offset = artifact.monitor_json().windows(needle.len()).position(|part| part == needle).unwrap();
    let mut changed = bytes.clone();
    changed[32 + offset + needle.len() - 1] = b'1';
    let changed = KvMonitorArtifact::decode(&changed, changed.len()).unwrap();
    assert_eq!(changed.verify(model, &tokenizer, &policy), Err(Error::Binding));
    artifact.verify(model, &tokenizer, &policy).unwrap();
}

#[test]
fn complete_framing_and_exact_size_are_required_without_partial_parameter_import() {
    let report = campaign();
    let model = report.model();
    let tokenizer = tokenizer(model);
    let codec = codec(model);
    let settings = settings(model);
    let bytes = report.monitor_artifact(&tokenizer, &codec, &settings,
        MAX_KV_MONITOR_ARTIFACT_BYTES).unwrap();
    assert_eq!(report.monitor_artifact(&tokenizer, &codec, &settings, bytes.len()).unwrap(), bytes);
    assert_eq!(report.monitor_artifact(&tokenizer, &codec, &settings, bytes.len() - 1), Err(Error::Limit));
    assert_eq!(report.monitor_artifact(&tokenizer, &codec, &settings,
        MAX_KV_MONITOR_ARTIFACT_BYTES + 1), Err(Error::Limit));
    assert_eq!(KvMonitorArtifact::decode(&bytes, bytes.len() - 1).unwrap_err(), Error::Limit);
    for cut in [0, 7, 31, 32, bytes.len() / 2, bytes.len() - 1] {
        assert!(KvMonitorArtifact::decode(&bytes[..cut], bytes.len()).is_err());
    }
    for corruption in 0..5 {
        let mut changed = bytes.clone();
        match corruption {
            0 => changed[7] = b'3',
            1 => changed[8..16].copy_from_slice(&0_u64.to_be_bytes()),
            2 => changed[16..24].copy_from_slice(&u64::MAX.to_be_bytes()),
            3 => changed[24..32].copy_from_slice(&1_u64.to_be_bytes()),
            _ => changed.push(0),
        }
        assert!(KvMonitorArtifact::decode(&changed, MAX_KV_MONITOR_ARTIFACT_BYTES).is_err());
    }
}

#[test]
fn failed_training_and_cross_profile_tokenizers_cannot_export_a_bound_artifact() {
    let failed_model = failed_model();
    let failed = trained::campaign(&trained::capture(&failed_model, &cases()));
    assert!(!failed.accepted());
    assert_eq!(failed.monitor_artifact(&tokenizer(&failed_model), &codec(&failed_model),
        &settings(&failed_model), MAX_KV_MONITOR_ARTIFACT_BYTES), Err(Error::WrongState));
    let report = campaign();
    let model = report.model();
    let mut identity = model.profile().identity();
    identity.tokenizer_generation += 1;
    let foreign = model_with_identity(identity);
    assert_eq!(foreign.cache_profile(), model.cache_profile());
    assert_eq!(report.monitor_artifact(&tokenizer(&foreign), &codec(model),
        &settings(model), MAX_KV_MONITOR_ARTIFACT_BYTES), Err(Error::Binding));
    assert!(report.monitor_artifact(&tokenizer(model), &codec(model), &settings(model),
        MAX_KV_MONITOR_ARTIFACT_BYTES).is_ok());
}

#[test]
fn static_rotary_model_uses_original_scaled_binding_with_a_working_control() {
    let model = scaled_model();
    let report = trained::campaign(&trained::capture(&model, &cases()));
    assert!(report.accepted());
    let tokenizer = tokenizer(&model);
    let codec = codec(&model);
    let settings = settings(&model);
    let policy = report.policy(codec.clone(), &settings).unwrap();
    let bytes = report.monitor_artifact(&tokenizer, &codec, &settings,
        MAX_KV_MONITOR_ARTIFACT_BYTES).unwrap();
    let artifact = KvMonitorArtifact::decode(&bytes, bytes.len()).unwrap();
    artifact.verify(&model, &tokenizer, &policy).unwrap();
    let unscaled = fixture::model();
    assert_eq!(unscaled.profile().identity(), model.profile().identity());
    assert_eq!(artifact.verify(&unscaled, &fixture::tokenizer(&unscaled), &policy), Err(Error::Binding));
}
