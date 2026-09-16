//! Actual decoder computation, independent residual oracle and causal mutations.
#[path = "support/decoder_identity.rs"] mod fixture;
use fixture::*;
use fa_reference::action::consequence::activation::identity::{IdentityAnchor, ModelPassport};
use fa_reference::action::consequence::activation::identity::decoder::{DecoderIdentityProbe, IdentityProbeProgress};
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, MAX_DECODER_PRODUCTS};
use fa_reference::Error;

fn budget() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }

#[test]
fn five_original_tokens_produce_two_analytically_registered_residuals() {
    let passport = passport();
    let mut probe = DecoderIdentityProbe::new(model(false, false), &passport, 51, budget()).unwrap();
    // Independently counted: 62 matrix terms/token and 8 attention terms/position;
    // separate prefixes of lengths 2 and 3 give 62*5 + 8*(3+6) = 382.
    assert_eq!(probe.work().planned_scalar_products, 382);
    assert_eq!(probe.work().planned_tokens, 5);
    assert_eq!(probe.work().entered_tokens, 0);
    let mut measured = Vec::new();
    for expected in 1..=5 {
        let event = probe.advance().unwrap();
        assert_eq!(probe.work().completed_tokens, expected);
        if let IdentityProbeProgress::Measured(measurement) = event {
            let anchor = &passport.anchors()[&measurement.anchor()];
            assert_eq!(anchor.compare(measurement.source()).unwrap().outside(), 0);
            assert_eq!(measurement.source().identity().sequence, 51);
            assert_eq!(measurement.capture().selection().sequence, anchor.stimulus().len() as u64);
            assert_eq!(measurement.capture().contract().profile(), anchor.profile());
            assert_eq!(measurement.source().identity().position, anchor.stimulus().len() as u64 - 1);
            measured.push(measurement.anchor());
        }
    }
    assert_eq!(measured, vec![10, 20]);
    assert!(probe.complete());
    assert_eq!(probe.work().completed_scalar_products, 382);
    assert_eq!(probe.work().measurement_bytes, 16);
    let before = probe.work();
    assert!(matches!(probe.advance().unwrap(), IdentityProbeProgress::Complete));
    assert_eq!(probe.work(), before);
}

#[test]
fn changing_actual_weights_with_identical_metadata_is_detected_by_the_original_comparator() {
    let passport = passport();
    for changed in [false, true] {
        let mut probe = DecoderIdentityProbe::new(model(changed, false), &passport, 1, budget()).unwrap();
        let mut outside = Vec::new();
        while !probe.complete() {
            if let IdentityProbeProgress::Measured(measurement) = probe.advance().unwrap() {
                outside.push(passport.anchors()[&measurement.anchor()].compare(measurement.source()).unwrap().outside());
            }
        }
        assert_eq!(outside, if changed { vec![0, 1] } else { vec![0, 0] });
    }
}

#[test]
fn whole_campaign_budget_and_late_invalid_tokens_refuse_before_starting_a_prefix() {
    let passport = passport();
    assert!(matches!(DecoderIdentityProbe::new(model(false, false), &passport, 1,
        DecoderBudget { scalar_products: 381 }), Err(Error::Limit)));
    assert!(DecoderIdentityProbe::new(model(false, false), &passport, 1,
        DecoderBudget { scalar_products: 382 }).is_ok());
    assert!(matches!(DecoderIdentityProbe::new(model(false, false), &passport, 0, budget()), Err(Error::InvalidInput)));
    assert!(matches!(DecoderIdentityProbe::new(model(false, false), &passport, 1,
        DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS + 1 }), Err(Error::Limit)));
    let original = &passport.anchors()[&20];
    let invalid = IdentityAnchor::new(20, original.profile(), original.stream(),
        vec![1, 0, 3], &[[-1.0, -1.0], [0.0, 0.0]]).unwrap();
    let invalid = ModelPassport::new(1, 1, manifest(), vec![passport.anchors()[&10].clone(), invalid]).unwrap();
    assert!(matches!(DecoderIdentityProbe::new(model(false, false), &invalid, 1, budget()), Err(Error::InvalidInput)));
}

#[test]
fn matching_dimensions_cannot_substitute_a_query_tap_or_another_numeric_layout() {
    let original = passport();
    for change in 0..3 {
        let mut capture = original.anchors()[&10].profile();
        let mut registered_manifest = manifest();
        match change {
            0 => capture.tap = 1, // same width, but a query rather than residual
            1 => capture.layout_generation += 1,
            _ => registered_manifest.tokenizer_generation += 1,
        }
        let anchor = IdentityAnchor::new(10, capture, 21, vec![0, 1], &[[0.0, 0.0], [1.0, 1.0]]).unwrap();
        let foreign = ModelPassport::new(1, 1, registered_manifest, vec![anchor]).unwrap();
        assert!(matches!(DecoderIdentityProbe::new(model(false, false), &foreign, 1, budget()), Err(Error::Binding)));
    }
}

#[test]
fn arithmetic_failure_is_terminal_and_retains_entered_not_completed_work() {
    let mut probe = DecoderIdentityProbe::new(model(false, true), &passport(), 1, budget()).unwrap();
    assert!(matches!(probe.advance(), Err(Error::Overflow)));
    assert!(!probe.complete());
    assert_eq!(probe.failure(), Some(Error::Overflow));
    let work = probe.work();
    assert_eq!(work.entered_tokens, 1);
    assert_eq!(work.entered_scalar_product_bound, 70);
    assert_eq!(work.completed_tokens, 0);
    assert_eq!(work.completed_scalar_products, 0);
    assert_eq!(work.measured_anchors, 0);
    for _ in 0..3 { assert!(matches!(probe.advance(), Err(Error::Overflow))); }
    assert_eq!(probe.work(), work);
}

#[test]
fn pause_and_probe_do_not_change_an_existing_continuation_cache_or_logits() {
    let model = model(false, false);
    let mut live = model.recompute(99, &[2], budget()).unwrap();
    let image = live.cache_image().unwrap().encode().unwrap();
    let logits: Vec<_> = live.logits().unwrap().iter().map(|v| v.to_bits()).collect();
    let mut probe = DecoderIdentityProbe::new(model.clone(), &passport(), 7, budget()).unwrap();
    assert!(matches!(probe.advance().unwrap(), IdentityProbeProgress::Advanced));
    let paused = probe.work();
    assert_eq!(live.cache_image().unwrap().encode().unwrap(), image);
    assert_eq!(live.tokens(), &[2]);
    assert_eq!(probe.work(), paused);
    while !probe.complete() { probe.advance().unwrap(); }
    assert_eq!(live.cache_image().unwrap().encode().unwrap(), image);
    assert_eq!(live.logits().unwrap().iter().map(|v| v.to_bits()).collect::<Vec<_>>(), logits);
    live.advance(1, 0, budget()).unwrap();
    let independent = model.recompute(99, &[2, 0], budget()).unwrap();
    assert_eq!(live.cache_image().unwrap().encode().unwrap(), independent.cache_image().unwrap().encode().unwrap());
}

#[test]
fn measurement_preserves_the_original_decoder_capture_bits_not_registered_bounds() {
    let model = model(true, false);
    let passport = passport();
    let mut probe = DecoderIdentityProbe::new(model.clone(), &passport, 17, budget()).unwrap();
    while !probe.complete() {
        if let IdentityProbeProgress::Measured(measurement) = probe.advance().unwrap() {
            let anchor = &passport.anchors()[&measurement.anchor()];
            let mut reference = model.session(anchor.stream()).unwrap();
            let mut expected = None;
            for (position, token) in anchor.stimulus().iter().copied().enumerate() {
                let step = reference.advance(position as u64, token, budget()).unwrap();
                expected = step.layers.into_iter().find(|row| row.residual.source().identity().profile == anchor.profile());
            }
            let original = expected.unwrap().residual;
            let mut actual = measurement.source().encode_initial(23).unwrap();
            // Only the explicit measurement sequence differs from the token sequence.
            actual[56..64].copy_from_slice(&original.source().identity().sequence.to_be_bytes());
            assert_eq!(actual, original.source().encode_initial(23).unwrap());
            assert_eq!(measurement.capture(), original.receipt());
        }
    }
}
