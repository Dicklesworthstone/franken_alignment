//! Original codec/probe/e-process with independently chosen causal controls.
use super::*;
use crate::action::consequence::activation::{CaptureProfile, FrameIdentity};
use crate::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget,
    ForecastRegistration, LikelihoodEvidence};
use crate::action::consequence::activation::probe::LinearProbe;

fn profile() -> CaptureProfile {
    CaptureProfile { tenant: 1, model: 2, model_generation: 3, tap: 4, layout_generation: 5 }
}
fn frame(values: &[f32]) -> SourceFrame {
    SourceFrame::capture(FrameIdentity { profile: profile(), stream: 21, sequence: 1, position: 0 }, values).unwrap()
}
fn model(weights: &[f32], threshold: f32) -> ForecastModel {
    ForecastModel::new(LinearProbe::new(91, 1, profile(), weights, 0.0, threshold).unwrap(), ForecastRegistration {
        domain: 71, generation: 1, policy_generation: 1, event_prefix: b"publish".to_vec(),
        negative: BinaryForecast::new(49_152, 16_384).unwrap(),
        at_threshold: BinaryForecast::new(32_768, 32_768).unwrap(),
        positive: BinaryForecast::new(16_384, 49_152).unwrap(),
    }).unwrap()
}
fn policy(initial: u8, stride: u8, maximum: u8) -> ProgressiveForecastPolicy {
    ProgressiveForecastPolicy::new(initial, stride, maximum, MAX_PROGRESSIVE_FORECAST_BYTES).unwrap()
}
fn assert_same_band(exact: &Prediction, partial: &Prediction) {
    assert_eq!(partial.forecast(), exact.forecast());
    assert_eq!(partial.domain(), exact.domain());
    assert_eq!(partial.generation(), exact.generation());
    assert_eq!(partial.policy_generation(), exact.policy_generation());
    assert_eq!(partial.observation().frame(), exact.observation().frame());
    assert_eq!(partial.observation().probe(), exact.observation().probe());
    assert_eq!(partial.observation().outcome(), exact.observation().outcome());
    assert!(partial.observation().interval().lower <= exact.observation().interval().lower);
    assert!(partial.observation().interval().upper >= exact.observation().interval().upper);
}

#[test]
fn certified_coarse_bands_use_fewer_bytes_without_changing_the_registered_forecast() {
    for sign in [-1.0, 1.0] {
        let source = frame(&vec![sign * 1.25; 256]);
        let exact = model(&vec![1.0; 256], 0.0).predict(&source).unwrap();
        let m = model(&vec![1.0; 256], 0.0).with_progressive(policy(0, 4, 23)).unwrap();
        let actual = m.predict(&source).unwrap();
        assert_same_band(&exact, &actual);
        assert_eq!(actual.observation().mantissa_bits(), 0);
        assert_ne!(actual.observation().interval().lower, actual.observation().interval().upper);
        assert_eq!(actual.encoded_bytes(), HEADER_BYTES + (9_usize * 256).div_ceil(8));
        assert!(actual.encoded_bytes() < exact.encoded_bytes());
        assert!(m.event(b"publish test")); assert!(!m.event(b"Publish test"));
    }
}

#[test]
fn rare_subnormal_refines_to_the_last_bit_instead_of_becoming_a_quiet_zero() {
    for sign in [-1.0, 1.0] {
        let source = frame(&[sign * f32::from_bits(1)]);
        let exact = model(&[1.0], 0.0).predict(&source).unwrap();
        let capped = model(&[1.0], 0.0).with_progressive(policy(0, 1, 22)).unwrap();
        assert_eq!(capped.predict(&source), Err(Error::Incomplete));
        let actual = model(&[1.0], 0.0).with_progressive(policy(0, 1, 23)).unwrap().predict(&source).unwrap();
        assert_same_band(&exact, &actual);
        assert_eq!(actual.observation().mantissa_bits(), 23);
        assert_eq!(actual.encoded_bytes(), HEADER_BYTES + 2 + 23 * (HEADER_BYTES + 1));
        assert!(actual.encoded_bytes() > exact.encoded_bytes(), "refinement headers are real cost");
    }
}

#[test]
fn equality_requires_a_degenerate_interval_and_never_uses_its_midpoint() {
    for value in [0.0, -0.0, 1.0, -2.0] {
        let source = frame(&[value]);
        let exact = model(&[1.0], value).predict(&source).unwrap();
        assert_eq!(exact.observation().outcome(), ProbeOutcome::AtThreshold);
        let capped = model(&[1.0], value).with_progressive(policy(0, 4, 22)).unwrap();
        assert_eq!(capped.predict(&source), Err(Error::Incomplete));
        let actual = model(&[1.0], value).with_progressive(policy(0, 4, 23)).unwrap().predict(&source).unwrap();
        assert_same_band(&exact, &actual); assert_eq!(actual.observation().mantissa_bits(), 23);
    }
}

#[test]
fn cumulative_byte_limit_is_checked_at_each_real_packet_boundary() {
    let source = frame(&[f32::from_bits(1)]);
    let first = source.encoded_len(None, 0).unwrap();
    let all = first + source.encoded_len(Some(0), 23).unwrap();
    for budget in [first - 1, first, all - 1] {
        let limited = ProgressiveForecastPolicy::new(0, 23, 23, budget).unwrap();
        assert_eq!(model(&[1.0], 0.0).with_progressive(limited).unwrap().predict(&source), Err(Error::Limit));
    }
    let exact_budget = ProgressiveForecastPolicy::new(0, 23, 23, all).unwrap();
    let actual = model(&[1.0], 0.0).with_progressive(exact_budget).unwrap().predict(&source).unwrap();
    assert_eq!(actual.encoded_bytes(), all);
    assert_eq!(actual.observation().outcome(), ProbeOutcome::CertifiedAlarm);
}

#[test]
fn original_exact_accumulator_retains_tiny_signal_under_huge_cancellation() {
    let source = frame(&[f32::MAX, f32::from_bits(1), f32::MAX]);
    let m = model(&[f32::MAX, f32::from_bits(1), -f32::MAX], 0.0);
    let exact = m.predict(&source).unwrap();
    let actual = m.with_progressive(policy(0, 3, 23)).unwrap().predict(&source).unwrap();
    assert_same_band(&exact, &actual);
    assert_eq!(actual.observation().interval().lower.magnitude_words()[0], 1);
    assert_eq!(actual.observation().mantissa_bits(), 23);
}

#[test]
fn mixed_sign_dimensions_and_all_precision_ladders_match_the_original_exact_oracle() {
    let source = frame(&[1.2345, -4.5678, f32::MIN_POSITIVE, -0.0]);
    for threshold in [-100.0, -20.0, 0.0, 100.0] {
        let original = model(&[-5.0, 3.0, -7.25, 1.0], threshold);
        let exact = original.predict(&source).unwrap();
        for initial in 0..=23 {
            for stride in 1..=23 {
                let actual = original.clone().with_progressive(policy(initial, stride, 23)).unwrap().predict(&source).unwrap();
                assert_same_band(&exact, &actual);
                assert!(actual.encoded_bytes() <= MAX_PROGRESSIVE_FORECAST_BYTES);
            }
        }
    }
}

#[test]
fn original_sequential_evidence_sees_exactly_the_same_factors_and_crossing() {
    let mut exact = LikelihoodEvidence::new(ErrorBudget::new(1, 4).unwrap());
    let mut adaptive = exact.clone();
    for (value, event) in [(1.25, true), (1.25, true), (-1.25, true), (0.0, false)] {
        let source = frame(&[value]);
        let baseline = model(&[1.0], 0.0).predict(&source).unwrap();
        let partial = model(&[1.0], 0.0).with_progressive(policy(0, 8, 23)).unwrap().predict(&source).unwrap();
        assert_eq!(exact.observe(baseline.forecast(), event), adaptive.observe(partial.forecast(), event));
        assert_eq!(adaptive, exact);
    }
    assert_eq!(adaptive.first_crossing(), Some(2));
}

#[test]
fn invalid_limits_and_source_bindings_refuse_without_weakening_the_exact_default() {
    for args in [(24, 1, 24, 100), (2, 1, 1, 100), (0, 0, 23, 100), (0, 24, 23, 100), (0, 1, 23, 0)] {
        assert!(ProgressiveForecastPolicy::new(args.0, args.1, args.2, args.3).is_err());
    }
    assert_eq!(ProgressiveForecastPolicy::new(0, 1, 23, MAX_PROGRESSIVE_FORECAST_BYTES + 1), Err(Error::Limit));
    let original = model(&[1.0], 0.0);
    assert_eq!(original.progressive_policy(), None);
    assert_eq!(original.predict(&frame(&[1.25])).unwrap().observation().mantissa_bits(), 23);
    let selected = original.with_progressive(policy(0, 1, 23)).unwrap();
    assert!(matches!(selected.clone().with_progressive(policy(0, 1, 23)), Err(Error::Duplicate)));
    assert_eq!(selected.predict(&frame(&[1.0, 2.0])), Err(Error::Binding));
    let mut other = frame(&[1.0]).identity(); other.profile.model_generation += 1;
    assert_eq!(selected.predict(&SourceFrame::capture(other, &[1.0]).unwrap()), Err(Error::Binding));
}
