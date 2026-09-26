//! Original engine controls for live learned-K/V source eligibility.
#[path = "support/restart_model.rs"]
pub mod fixture;
use fa_reference::Error;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{SamplingPolicy, SamplingStart};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::monitored::{
    GenerationBudget, GenerationSpec, GenerationStatus, GenerationTelemetryBudget,
};
use fa_reference::action::consequence::oversight::learned_source::{
    LearnedAvailability, LearnedEvidenceLimits, LearnedSourceConfig, MAX_LEARNED_EVIDENCE_BYTES,
};
use std::collections::BTreeSet;

fn config(mode: u8) -> LearnedSourceConfig {
    let model = fixture::model();
    LearnedSourceConfig { stream: 21, evaluation_origin: 201, monitor_generation: 31,
        spec: GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
            policy: SamplingPolicy::new(1, 1, 3, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173,
        }).unwrap(), policy: fixture::policy(&model, mode, 1),
        budget: GenerationBudget::default(), telemetry: GenerationTelemetryBudget::default() }
}

#[test]
fn original_stochastic_execution_matches_and_each_new_prefix_stales_earlier_evidence() {
    let model = fixture::model();
    let mut config = config(0);
    config.spec = GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
        policy: SamplingPolicy::new(1, 1, 3, 0.8, 3, 1.0).unwrap(), stream: 71, seed: 173,
    }).unwrap();
    let mut control = model.monitored_generation_with_telemetry(config.stream, config.evaluation_origin,
        config.spec.clone(), config.policy.clone(), config.budget, config.telemetry).unwrap();
    let mut run = model.observed_learned_generation(config).unwrap();
    let source = run.observation();
    assert_eq!(source.availability(), LearnedAvailability::Empty);
    assert_eq!(source.capture(LearnedEvidenceLimits::default()).err(), Some(Error::Incomplete));
    let mut previous = None;
    for position in 0..4 {
        let expected = control.advance(position).unwrap();
        let actual = run.advance(position).unwrap();
        assert_eq!(actual.sample(), expected.sample());
        if let (Some(a), Some(b)) = (actual.sample(), expected.sample()) {
            assert_eq!(a.probability.to_bits(), b.probability.to_bits());
        }
        assert_eq!(fixture::logits(&actual.accepted().unwrap().logits),
            fixture::logits(&expected.accepted().unwrap().logits));
        assert_eq!(source.availability(), LearnedAvailability::Ready);
        if let Some(old) = previous { assert_eq!(source.validate(&old), Err(Error::Stale)); }
        let evidence = source.capture(LearnedEvidenceLimits::default()).unwrap();
        assert_eq!(source.validate(&evidence), Ok(()));
        assert_eq!(evidence.tokens(), control.accepted_tokens());
        assert_eq!(evidence.next_position(), position + 1);
        assert_eq!(evidence.evaluation_origin(), 201);
        assert_eq!(evidence.stream(), 21);
        assert_eq!(evidence.generation(), 31);
        assert!(evidence.audit().complete_quiet());
        assert_eq!(evidence.audit().planned_rows(), 4);
        assert_eq!(evidence.audit().first_position(), position);
        assert_eq!(evidence.audit().end_position(), position + 1);
        assert_eq!(run.work(), control.work());
        assert_eq!(run.telemetry_work(), control.telemetry_work());
        previous = Some(evidence);
    }
    // A declared completed horizon is a stable accepted source, not an error.
    let evidence = previous.unwrap();
    assert_eq!(run.advance(4).err(), Some(Error::WrongState));
    assert_eq!(source.validate(&evidence), Ok(()));
    drop(run);
    assert_eq!(source.availability(), LearnedAvailability::Closed);
    assert_eq!(source.validate(&evidence), Err(Error::Incomplete));
    assert_eq!(evidence.tokens().len(), 4); // Historical evidence is immutable.
}

#[test]
fn a_real_later_alarm_withholds_candidate_and_revokes_older_quiet_basis() {
    let model = fixture::model();
    let mut run = model.observed_learned_generation(config(2)).unwrap();
    let source = run.observation();
    run.advance(0).unwrap();
    let before = source.capture(LearnedEvidenceLimits::default()).unwrap();
    let held = run.advance(1).unwrap();
    assert!(matches!(held.status(), GenerationStatus::Held(_)));
    assert!(held.accepted().is_none() && held.sample().is_none());
    assert_eq!(source.availability(), LearnedAvailability::Held);
    assert_eq!(source.validate(&before), Err(Error::Incomplete));
    assert_eq!(source.capture(LearnedEvidenceLimits::default()).err(), Some(Error::Incomplete));
    assert_eq!(run.accepted_tokens(), &[0]);
    assert_eq!(before.tokens(), &[0]);
    assert_eq!(run.advance(1).err(), Some(Error::WrongState));
    let mut positive = model.observed_learned_generation(config(0)).unwrap();
    positive.advance(0).unwrap();
    assert!(positive.advance(1).unwrap().accepted().is_some());
}

#[test]
fn missing_source_check_is_failed_not_a_reusable_quiet_prefix() {
    let model = fixture::model();
    let mut control = model.observed_learned_generation(config(0)).unwrap();
    control.advance(0).unwrap();
    let one = control.telemetry_work().source_check_values;
    assert!(one > 0);
    control.advance(1).unwrap();
    let mut limited = config(0);
    limited.telemetry.source_check_values = one;
    let mut run = model.observed_learned_generation(limited).unwrap();
    run.advance(0).unwrap();
    let source = run.observation();
    let before = source.capture(LearnedEvidenceLimits::default()).unwrap();
    assert_eq!(run.advance(1).err(), Some(Error::Limit));
    assert_eq!(source.availability(), LearnedAvailability::Failed);
    assert_eq!(source.validate(&before), Err(Error::Incomplete));
    assert_eq!(run.work().admitted_tokens, 2);
    assert_eq!(run.work().accepted_decoder.tokens, 1);
    assert_eq!(run.advance(1).err(), Some(Error::WrongState));
}

#[test]
fn stale_calls_leave_eligibility_intact_and_equal_ids_cannot_substitute_an_owner() {
    let model = fixture::model();
    let mut run = model.observed_learned_generation(config(0)).unwrap();
    let source = run.observation();
    assert_eq!(run.advance(1).err(), Some(Error::Stale));
    assert_eq!(source.availability(), LearnedAvailability::Empty);
    run.advance(0).unwrap();
    let evidence = source.capture(LearnedEvidenceLimits::default()).unwrap();
    let work = run.work();
    for position in [0, 2] { assert_eq!(run.advance(position).err(), Some(Error::Stale)); }
    assert_eq!(run.work(), work);
    assert_eq!(source.validate(&evidence), Ok(()));
    let mut other = model.observed_learned_generation(config(0)).unwrap();
    other.advance(0).unwrap();
    let other_source = other.observation();
    let alternate = other_source.capture(LearnedEvidenceLimits::default()).unwrap();
    assert_eq!(alternate.tokens(), evidence.tokens());
    assert_eq!(other_source.validate(&evidence), Err(Error::Binding));
    assert_eq!(source.validate(&alternate), Err(Error::Binding));
}

#[test]
fn every_capture_cost_has_an_exact_and_one_less_boundary_without_source_mutation() {
    let model = fixture::model();
    let mut run = model.observed_learned_generation(config(0)).unwrap();
    run.advance(0).unwrap();
    let source = run.observation();
    let evidence = source.capture(LearnedEvidenceLimits::default()).unwrap();
    let cost = evidence.cost();
    assert!(cost.token_ids > 0 && cost.score_words > 0 && cost.encoded_bytes > 0);
    let exact = LearnedEvidenceLimits { token_ids: cost.token_ids,
        score_words: cost.score_words, encoded_bytes: cost.encoded_bytes };
    assert_eq!(source.capture(exact).unwrap().cost(), cost);
    for limits in [LearnedEvidenceLimits { token_ids: cost.token_ids - 1, ..exact },
        LearnedEvidenceLimits { score_words: cost.score_words - 1, ..exact },
        LearnedEvidenceLimits { encoded_bytes: cost.encoded_bytes - 1, ..exact },
        LearnedEvidenceLimits { encoded_bytes: MAX_LEARNED_EVIDENCE_BYTES + 1, ..exact }] {
        assert_eq!(source.capture(limits).err(), Some(Error::Limit));
        assert_eq!(source.validate(&evidence), Ok(()));
    }
}

#[test]
fn construction_preserves_training_split_and_requires_nonzero_monitor_generation() {
    let model = fixture::model();
    assert!(model.observed_learned_generation(config(0)).is_ok());
    let mut duplicate = config(0);
    duplicate.stream = 11;
    assert_eq!(model.observed_learned_generation(duplicate).err(), Some(Error::Duplicate));
    let mut duplicate = config(0);
    duplicate.evaluation_origin = 101;
    assert_eq!(model.observed_learned_generation(duplicate).err(), Some(Error::Duplicate));
    let mut missing = config(0);
    missing.monitor_generation = 0;
    assert_eq!(model.observed_learned_generation(missing).err(), Some(Error::InvalidInput));
}
