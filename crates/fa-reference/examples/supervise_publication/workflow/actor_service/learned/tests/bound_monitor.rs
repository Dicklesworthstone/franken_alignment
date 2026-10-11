//! The real operator loader compares complete trained assets before permitting
//! original generation. Synthetic signal tests are not detector qualification.
#[path = "../../../../../../tests/support/bound_kv_monitor.rs"]
#[allow(dead_code)]
mod trained;

use super::super::monitor::MonitorInput;
use fa_reference::action::consequence::activation::{
    monitor::MonitorOutcome,
    probe::training::decoder::kv::{KvMonitorArtifact, MAX_KV_MONITOR_ARTIFACT_BYTES,
        MAX_KV_MONITOR_JSON_BYTES},
    tensor::kv::decoder::sampling::{SampleBudget, SamplingBudget,
        monitored::{GenerationBudget, GenerationStatus, GenerationStop}},
};

#[test]
fn bound_monitor_enters_the_actual_loader_and_guards_real_quiet_and_violating_samples() {
    let campaign = trained::campaign();
    assert!(campaign.accepted());
    let model = campaign.model();
    let tokenizer = trained::tokenizer(model);
    let codec = trained::codec(model);
    let mut settings = trained::settings(model);
    // Distinct explicit limits must survive the artifact and actual parser.
    settings.audit.rows = 7;
    settings.inference.scalar_products -= 51;
    for (index, budget) in settings.taps.values_mut().enumerate() {
        budget.probe_coordinates -= index + 1;
    }
    let bytes = campaign.monitor_artifact(&tokenizer, &codec, &settings,
        MAX_KV_MONITOR_ARTIFACT_BYTES).unwrap();
    let loaded = MonitorInput::decode(&bytes).unwrap()
        .bind_with_tokenizer(model, &tokenizer, codec.clone()).unwrap();
    let expected_policy = campaign.policy(codec, &settings).unwrap();
    assert_eq!(loaded.policy.inference(), settings.inference);
    assert_eq!(loaded.policy.monitor().budget(), settings.audit);
    assert_eq!(loaded.policy.allowance(), expected_policy.allowance());
    assert_eq!(loaded.probes.len(), 4);
    for (tap, expected) in campaign.probes().unwrap() {
        assert_eq!(loaded.probes[&tap].len(), 1);
        assert_eq!(loaded.probes[&tap][0].identity(), expected.identity());
        assert_eq!(loaded.policy.monitor().taps()[&tap].budget(), settings.taps[&tap]);
        assert_eq!(expected.identity().dimensions, 2); // GQA cache width, not hidden width.
    }
    for (seed, chosen) in [(4, 0), (5, 1)] {
        let spec = trained::spec(seed);
        let mut oracle = model.recompute_sampled(2000, spec.prompt(), trained::inference(),
            spec.sampling().clone()).unwrap();
        let sample = oracle.advance_sampled(1, SampleBudget { decoder: trained::inference(),
            sampling: SamplingBudget { vocabulary: 260 } }).unwrap();
        assert_eq!(sample.choice.token, chosen);
        let mut run = model.monitored_generation(2000, 2001, spec, loaded.policy.clone(),
            GenerationBudget::default()).unwrap();
        let prompt = run.advance(0).unwrap();
        assert!(prompt.audit().complete_quiet());
        assert_eq!(prompt.audit().planned_rows(), 4);
        let accepted = run.accepted_cache_image().unwrap().encode().unwrap();
        let logits = run.accepted_logits().unwrap().to_vec();
        let sampler = run.sampler_state();
        let event = run.advance(1).unwrap();
        assert_eq!(event.audit().planned_rows(), 4);
        assert_eq!(event.audit().source().report().source_values, 8);
        if chosen == 0 {
            assert!(event.audit().complete_quiet());
            assert_eq!(event.sample(), Some(&sample.choice));
            assert_eq!(event.accepted().unwrap().logits, sample.computation.logits);
            let stop = run.advance(2).unwrap();
            assert!(stop.audit().complete_quiet());
            assert_eq!(stop.sample().unwrap().token, 256);
            assert_eq!(run.status(), GenerationStatus::Finished(GenerationStop::StopToken(256)));
            assert_eq!(run.generated_tokens(), &[0, 256]);
        } else {
            assert_eq!(event.status(), GenerationStatus::Held(MonitorOutcome::Alarm));
            assert!(event.accepted().is_none());
            assert!(event.sample().is_none());
            assert!(run.generated_tokens().is_empty());
            assert_eq!(run.accepted_cache_image().unwrap().encode().unwrap(), accepted);
            assert_eq!(run.accepted_logits().unwrap(), logits);
            assert_eq!(run.sampler_state(), sampler);
            assert_eq!(run.work().sampling_attempts, 1);
            assert_eq!(run.work().admitted_tokens, 2);
        }
    }
}

#[test]
fn bound_operator_refuses_same_label_unused_weights_tokenizer_semantics_and_codec_origins() {
    let campaign = trained::campaign();
    let model = campaign.model();
    let tokenizer = trained::tokenizer(model);
    let codec = trained::codec(model);
    let settings = trained::settings(model);
    let bytes = campaign.monitor_artifact(&tokenizer, &codec, &settings,
        MAX_KV_MONITOR_ARTIFACT_BYTES).unwrap();
    assert!(MonitorInput::decode(&bytes).unwrap().bind(model, codec.clone()).is_err());
    MonitorInput::decode(&bytes).unwrap()
        .bind_with_tokenizer(model, &tokenizer, codec.clone()).unwrap();

    let substituted = trained::changed_unused_weight();
    assert_eq!(model.profile(), substituted.profile());
    // Every supplied campaign prefix is unchanged by this unused embedding.
    // Agreement on observed logits therefore cannot establish model equality.
    for case in trained::cases() {
        let original = model.recompute(case.origin.task, &case.tokens, trained::inference()).unwrap();
        let changed = substituted.recompute(case.origin.task, &case.tokens, trained::inference()).unwrap();
        assert_eq!(original.logits(), changed.logits());
        assert_eq!(original.cache_image().unwrap().encode().unwrap(),
            changed.cache_image().unwrap().encode().unwrap());
    }
    assert!(MonitorInput::decode(&bytes).unwrap()
        .bind_with_tokenizer(&substituted, &tokenizer, codec.clone()).is_err());
    for variant in 1..=3 {
        let changed = trained::changed_tokenizer(model, variant);
        assert!(changed.binds(model.profile()));
        assert!(MonitorInput::decode(&bytes).unwrap()
            .bind_with_tokenizer(model, &changed, codec.clone()).is_err());
    }
    let changed_codec = trained::codec_from(model, 1002, 1012);
    assert_eq!(changed_codec.profile(), codec.profile());
    assert_eq!(changed_codec.policy(), codec.policy());
    assert!(MonitorInput::decode(&bytes).unwrap()
        .bind_with_tokenizer(model, &tokenizer, changed_codec).is_err());

    // Legacy JSON remains explicitly unbound provisioning. The paired new
    // refusal is powered by the actual artifact, not a newly incompatible v1.
    let legacy = campaign.monitor_json(&codec, &settings, MAX_KV_MONITOR_JSON_BYTES).unwrap();
    MonitorInput::decode(&legacy).unwrap().bind(&substituted, codec.clone()).unwrap();
    MonitorInput::decode(&legacy).unwrap()
        .bind_with_tokenizer(model, &tokenizer, codec).unwrap();
}

#[test]
fn bound_operator_checks_the_embedded_policy_and_never_falls_back_on_corrupt_framing() {
    let campaign = trained::campaign();
    let model = campaign.model();
    let tokenizer = trained::tokenizer(model);
    let codec = trained::codec(model);
    let mut settings = trained::settings(model);
    settings.inference.scalar_products = 900_000;
    let bytes = campaign.monitor_artifact(&tokenizer, &codec, &settings,
        MAX_KV_MONITOR_ARTIFACT_BYTES).unwrap();
    let expected = campaign.policy(codec.clone(), &settings).unwrap();
    let artifact = KvMonitorArtifact::decode(&bytes, MAX_KV_MONITOR_ARTIFACT_BYTES).unwrap();
    artifact.verify(model, &tokenizer, &expected).unwrap();
    let json = std::str::from_utf8(artifact.monitor_json()).unwrap();
    let field = "\"inference_products\":";
    let position = 32 + json.find(field).unwrap() + field.len();
    let mut changed = bytes.clone();
    assert!(changed[position] > b'1' && changed[position] <= b'9');
    changed[position] -= 1; // A still valid, smaller runtime budget, not invalid JSON.
    let altered = KvMonitorArtifact::decode(&changed, MAX_KV_MONITOR_ARTIFACT_BYTES).unwrap();
    assert!(altered.verify(model, &tokenizer, &expected).is_err());
    assert!(MonitorInput::decode(&changed).unwrap()
        .bind_with_tokenizer(model, &tokenizer, codec.clone()).is_err());

    let mut wrong_version = bytes.clone();
    wrong_version[7] = b'3';
    let mut suffix = bytes.clone(); suffix.push(0);
    let mut bad_length = bytes.clone(); bad_length[8..16].copy_from_slice(&u64::MAX.to_be_bytes());
    for malformed in [wrong_version, suffix, bad_length, bytes[..31].to_vec(),
        bytes[..bytes.len() - 1].to_vec()] {
        assert!(MonitorInput::decode(&malformed).is_err());
    }
    MonitorInput::decode(&bytes).unwrap()
        .bind_with_tokenizer(model, &tokenizer, codec).unwrap();
}
