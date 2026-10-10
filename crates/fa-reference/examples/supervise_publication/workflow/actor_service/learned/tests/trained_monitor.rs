//! The trained GQA roster crosses the actual operator loader into the original
//! generation guard. These synthetic cases test this boundary, not model safety
//! or authentication of the model, codec, labels, or provisioning JSON.
#[path = "../../../../../../tests/support/decoder_kv_probe_campaign.rs"]
#[allow(dead_code)]
mod trained;

use super::super::monitor::MonitorInput;
use fa_reference::action::consequence::activation::{FrameIdentity, ProgressiveFrame, SourceFrame,
    monitor::{MonitorOutcome, learned::model::KvTap},
    probe::{ProbeOutcome, training::{CaseLabel, decoder::kv::{KvMonitorSettings, MAX_KV_MONITOR_JSON_BYTES}}},
    tensor::kv::{decoder::{DecoderModel, monitoring::LearnedStreamRetention,
        sampling::{SampleBudget, SamplingBudget, monitored::{GenerationBudget, GenerationPhase,
            GenerationStatus, GenerationStop, LearnedGeneration}}}, experiment::KvSide},
};
use fa_reference::strict_json::{self, Json, Limits};
use fa_reference::Error;

// Every numeric setting differs from its default, and each tap has distinct
// caps. A loader silently substituting defaults must fail the roundtrip.
fn settings(model: &DecoderModel) -> KvMonitorSettings {
    let mut settings = trained::settings(model);
    for (index, budget) in settings.taps.values_mut().enumerate() {
        let offset = index + 1;
        budget.encoded_bytes -= offset;
        budget.probe_coordinates -= offset + 4;
        budget.reconstruction_products -= (offset + 8) as u64;
        budget.materialized_values -= offset + 12;
        budget.refinements -= offset + 16;
    }
    settings.audit.rows = 7;
    settings.audit.monitoring.encoded_bytes -= 21;
    settings.audit.monitoring.probe_coordinates -= 22;
    settings.audit.monitoring.reconstruction_products -= 23;
    settings.audit.monitoring.materialized_values -= 24;
    settings.audit.monitoring.refinements -= 25;
    settings.preparation.compression.source_values -= 31;
    settings.preparation.compression.encoded_bytes -= 32;
    settings.preparation.compression.work_units -= 33;
    settings.preparation.source_check.source_values -= 41;
    settings.preparation.source_check.encoded_bytes -= 42;
    settings.preparation.source_check.reconstruction_products -= 43;
    settings.inference.scalar_products -= 51;
    settings
}

fn bits(value: &Json) -> u32 {
    let Json::Number(number) = value else { panic!("expected a finite coefficient"); };
    number.lexeme().parse::<f32>().unwrap().to_bits()
}

#[test]
fn trained_monitor_export_loads_every_coefficient_tap_resource_limit_and_retention() {
    let report = trained::campaign(&trained::capture(&trained::model(), &trained::cases()));
    assert!(report.accepted());
    let model = report.model();
    let codec = trained::codec(model);
    let expected_probes = report.probes().unwrap();
    assert_eq!(expected_probes.len(), 4);
    let mut settings = settings(model);
    for (retention, retained_groups) in [(LearnedStreamRetention::All, 4), (LearnedStreamRetention::None, 0)] {
        settings.retention = retention;
        let bytes = report.monitor_json(&codec, &settings, MAX_KV_MONITOR_JSON_BYTES).unwrap();
        let loaded = MonitorInput::decode(&bytes).unwrap().bind(model, codec.clone()).unwrap();
        let expected = report.policy(codec.clone(), &settings).unwrap();
        assert_eq!(loaded.policy.codec().profile(), expected.codec().profile());
        assert_eq!(loaded.policy.monitor().profile(), model.cache_profile());
        assert_eq!(loaded.policy.monitor().budget(), settings.audit);
        assert_eq!(loaded.policy.preparation(), settings.preparation);
        assert_eq!(loaded.policy.inference(), settings.inference);
        assert_eq!(loaded.policy.allowance(), expected.allowance());
        assert!(loaded.probes.keys().eq(expected_probes.keys()));
        assert!(loaded.policy.monitor().taps().keys().eq(settings.taps.keys()));
        let json = strict_json::parse(&bytes, Limits::default()).unwrap();
        let emitted = json.get("taps").unwrap().as_array().unwrap();
        assert_eq!(emitted.len(), expected_probes.len());
        for (encoded, (tap, original)) in emitted.iter().zip(&expected_probes) {
            let monitor = &loaded.policy.monitor().taps()[tap];
            assert_eq!(monitor.budget(), settings.taps[tap]);
            assert_eq!(monitor.profile(), original.identity().profile);
            assert_eq!(monitor.dimensions(), 2); // Actual GQA K/V width, not hidden width 4.
            let probes = &loaded.probes[tap];
            assert_eq!(probes.len(), 1);
            let probe = &probes[0];
            assert_eq!(probe.identity(), original.identity());
            assert_eq!(encoded.get("layer").unwrap().as_u64(), Some(tap.layer));
            assert_eq!(encoded.get("side").unwrap().as_str(), Some(match tap.side {
                KvSide::Key => "key", KvSide::Value => "value",
            }));
            let encoded_probes = encoded.get("probes").unwrap().as_array().unwrap();
            assert_eq!(encoded_probes.len(), 1);
            let encoded_probe = &encoded_probes[0];
            let calibration = report.taps()[tap].calibration();
            let fitted = calibration.fitted();
            assert!(fitted.weights().iter().any(|value| *value != 0.0));
            assert_eq!(encoded_probe.get("id").unwrap().as_u64(), Some(original.identity().id));
            assert_eq!(encoded_probe.get("generation").unwrap().as_u64(), Some(original.identity().generation));
            assert_eq!(encoded_probe.get("weights").unwrap().as_array().unwrap().iter().map(bits).collect::<Vec<_>>(),
                fitted.weights().iter().map(|value| value.to_bits()).collect::<Vec<_>>());
            assert_eq!(bits(encoded_probe.get("bias").unwrap()), fitted.bias().to_bits());
            assert_eq!(bits(encoded_probe.get("threshold").unwrap()), calibration.selected_threshold().unwrap().to_bits());

            // The public probe API deliberately has no coefficient accessor.
            // Exact scores on zero and signed coordinate bases exercise every
            // loaded weight and its bias/threshold margin without exposing one.
            for values in [[0.0, 0.0], [1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0]] {
                let source = SourceFrame::capture(FrameIdentity { profile: probe.identity().profile,
                    stream: 7000, sequence: 1, position: 0 }, &values).unwrap();
                let block = source.verify_block(&source.encode_initial(23).unwrap()).unwrap();
                let frame = ProgressiveFrame::from_initial(&block).unwrap();
                let actual = probe.evaluate(&frame).unwrap();
                assert_eq!(actual.interval().lower, actual.interval().upper);
                assert_eq!(actual, original.evaluate(&frame).unwrap());
            }
        }

        // Also score fresh ORIGINAL decoder output, independently of the
        // captured training frames, over every supplied class and every tap.
        for case in report.cases().values() {
            let source = model.recompute(case.origin.task, &case.tokens, trained::inference()).unwrap()
                .cache_image().unwrap();
            for (tap, probe) in &expected_probes {
                let token = source.layer(tap.layer).unwrap().token(1).unwrap();
                let frame = match tap.side { KvSide::Key => token.key(), KvSide::Value => token.value() };
                let block = frame.verify_block(&frame.encode_initial(23).unwrap()).unwrap();
                let exact = ProgressiveFrame::from_initial(&block).unwrap();
                let actual = loaded.probes[tap][0].evaluate(&exact).unwrap();
                assert_eq!(actual, probe.evaluate(&exact).unwrap());
                assert_eq!(actual.outcome(), match case.label {
                    CaseLabel::Benign => ProbeOutcome::CertifiedQuiet,
                    CaseLabel::Violation => ProbeOutcome::CertifiedAlarm,
                });
            }
        }
        let mut run = model.monitored_generation(2000, 2001, trained::spec(4), loaded.policy,
            GenerationBudget::default()).unwrap();
        let event = run.advance(0).unwrap();
        assert_eq!(event.audit().source().report().groups, 4);
        assert_eq!(event.audit().source().report().retained_groups, retained_groups);
    }
}

fn same_generation(actual: &LearnedGeneration, expected: &LearnedGeneration) {
    assert_eq!(actual.status(), expected.status());
    assert_eq!(actual.position(), expected.position());
    assert_eq!(actual.accepted_tokens(), expected.accepted_tokens());
    assert_eq!(actual.generated_tokens(), expected.generated_tokens());
    assert_eq!(actual.accepted_logits(), expected.accepted_logits());
    assert_eq!(actual.accepted_cache_image().unwrap().encode().unwrap(),
        expected.accepted_cache_image().unwrap().encode().unwrap());
    assert_eq!(actual.samples(), expected.samples());
    assert_eq!(actual.sampler_state(), expected.sampler_state());
    assert_eq!(actual.work(), expected.work());
    assert_eq!(actual.telemetry_work(), expected.telemetry_work());
    let actual = actual.last_event().unwrap();
    let expected = expected.last_event().unwrap();
    assert_eq!(actual.status(), expected.status());
    assert_eq!(actual.sample(), expected.sample());
    assert_eq!(actual.audit().planned_rows(), expected.audit().planned_rows());
    assert_eq!(actual.audit().quiet_rows(), expected.audit().quiet_rows());
    assert_eq!(actual.audit().blocked_row(), expected.audit().blocked_row());
    assert_eq!(actual.audit().work(), expected.audit().work());
    assert_eq!(actual.audit().source().report(), expected.audit().source().report());
}

#[test]
fn loaded_trained_monitor_releases_benign_samples_but_withholds_a_real_violation() {
    let report = trained::campaign(&trained::capture(&trained::model(), &trained::cases()));
    let model = report.model();
    let codec = trained::codec(model);
    let settings = settings(model);
    let bytes = report.monitor_json(&codec, &settings, MAX_KV_MONITOR_JSON_BYTES).unwrap();
    let loaded = MonitorInput::decode(&bytes).unwrap().bind(model, codec.clone()).unwrap();
    let original = report.policy(codec, &settings).unwrap();
    for (seed, expected_token) in [(4, 0), (5, 1)] {
        let spec = trained::spec(seed);
        let mut oracle = model.recompute_sampled(2000, spec.prompt(), trained::inference(),
            spec.sampling().clone()).unwrap();
        let sampled = oracle.advance_sampled(1, SampleBudget { decoder: trained::inference(),
            sampling: SamplingBudget { vocabulary: 10 } }).unwrap();
        assert_eq!(sampled.choice.token, expected_token);
        let mut actual = model.monitored_generation(2000, 2001, spec.clone(), loaded.policy.clone(),
            GenerationBudget::default()).unwrap();
        let mut expected = model.monitored_generation(2000, 2001, spec, original.clone(),
            GenerationBudget::default()).unwrap();
        let prompt = actual.advance(0).unwrap();
        expected.advance(0).unwrap();
        assert!(prompt.audit().complete_quiet());
        assert_eq!(prompt.audit().planned_rows(), 4);
        same_generation(&actual, &expected);
        let accepted_cache = actual.accepted_cache_image().unwrap().encode().unwrap();
        let accepted_logits = actual.accepted_logits().unwrap().to_vec();
        let sampler = actual.sampler_state();
        let next = actual.advance(1).unwrap();
        expected.advance(1).unwrap();
        same_generation(&actual, &expected);
        assert_eq!(next.phase(), GenerationPhase::Continuation);
        assert_eq!(next.audit().planned_rows(), 4);
        assert_eq!(next.audit().source().report().source_values, 8);
        if expected_token == 0 {
            assert!(next.audit().complete_quiet());
            assert_eq!(next.sample(), Some(&sampled.choice));
            assert_eq!(next.accepted().unwrap().logits, sampled.computation.logits);
            assert_eq!(actual.accepted_tokens(), &[4, 0]);
            let stop = actual.advance(2).unwrap();
            expected.advance(2).unwrap();
            same_generation(&actual, &expected);
            assert!(stop.audit().complete_quiet());
            assert_eq!(stop.sample().unwrap().token, 2);
            assert_eq!(actual.status(), GenerationStatus::Finished(GenerationStop::StopToken(2)));
            assert_eq!(actual.accepted_tokens(), &[4, 0, 2]);
            assert_eq!(actual.generated_tokens(), &[0, 2]);
        } else {
            assert_eq!(next.status(), GenerationStatus::Held(MonitorOutcome::Alarm));
            assert!(next.sample().is_none());
            assert!(next.accepted().is_none());
            assert_eq!(actual.accepted_tokens(), &[5]);
            assert!(actual.generated_tokens().is_empty());
            assert_eq!(actual.accepted_cache_image().unwrap().encode().unwrap(), accepted_cache);
            assert_eq!(actual.accepted_logits().unwrap(), accepted_logits);
            assert_eq!(actual.sampler_state(), sampler);
            assert_eq!(actual.work().sampling_attempts, 1);
            assert_eq!(actual.work().admitted_tokens, 2);
            assert_eq!(actual.advance(1).unwrap_err(), Error::WrongState);
        }
    }
}

#[test]
fn loaded_last_tap_budget_cannot_turn_three_quiet_rows_into_a_released_token() {
    let report = trained::campaign(&trained::capture(&trained::model(), &trained::cases()));
    let model = report.model();
    let codec = trained::codec(model);
    let mut settings = settings(model);
    let tap = KvTap { layer: 2, side: KvSide::Value };
    settings.taps.get_mut(&tap).unwrap().probe_coordinates = 0;
    let bytes = report.monitor_json(&codec, &settings, MAX_KV_MONITOR_JSON_BYTES).unwrap();
    let loaded = MonitorInput::decode(&bytes).unwrap().bind(model, codec).unwrap();
    assert_eq!(loaded.policy.monitor().taps()[&tap].budget().probe_coordinates, 0);
    let mut run = model.monitored_generation(2000, 2001, trained::spec(4), loaded.policy,
        GenerationBudget::default()).unwrap();
    let sampler = run.sampler_state();
    let event = run.advance(0).unwrap();
    assert_eq!(run.status(), GenerationStatus::Held(MonitorOutcome::BudgetExhausted));
    assert_eq!(event.audit().planned_rows(), 4);
    assert_eq!(event.audit().quiet_rows(), 3);
    assert_eq!(event.audit().examined_rows(), 3);
    let stopped = event.audit().blocked_row().unwrap();
    assert_eq!((stopped.layer, stopped.side, stopped.position), (tap.layer, tap.side, 0));
    assert!(!event.audit().complete_quiet());
    assert!(event.accepted().is_none());
    assert!(event.sample().is_none());
    assert!(run.accepted_tokens().is_empty());
    assert_eq!(run.position(), 0);
    assert_eq!(run.sampler_state(), sampler);
    assert_eq!(run.work().admitted_tokens, 1);
    assert_eq!(run.work().sampling_attempts, 0);
    assert!(event.audit().work().probe_coordinates > 0);
    assert_eq!(run.advance(0).unwrap_err(), Error::WrongState);
}
