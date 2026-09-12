//! Owned monitored inference feeds the original actor/control/effect chain.
#[path = "support/hosted_decoder.rs"]
#[allow(dead_code)]
mod support;
use support::*;
use fa_reference::action::consequence::activation::monitor::decoder::{MonitoredStep, MonitoringStatus};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledStep;
use fa_reference::action::consequence::activation::monitor::decoder::observation::DecoderAvailability;
use fa_reference::action::consequence::delivery::StopRequest;
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use fa_reference::action::consequence::gate::{ReviewBinding, TargetCeiling};
use fa_reference::action::consequence::gate::containment::ResetRequest;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::SamplingBudget;
use fa_reference::Error;

#[test]
fn automatic_actor_capture_matches_original_forced_and_sampled_inference_then_publishes() {
    for two_key in [false, true] {
        let mut f = Fixture::new(two_key);
        let model = model();
        let mut raw = model.recompute_sampled(7, &[1, 2], compute(), sampling(6)).unwrap();
        f.broker.own_sampled_decoder(quiet(), DecoderBindingLimits::default()).unwrap();
        let initial = f.broker.hosted_decoder().unwrap();
        assert_eq!(initial.position, 0);
        assert!(initial.cache_bytes > 0 && initial.sampler_bytes > 0);
        for (position, token) in [1, 2].into_iter().enumerate() {
            assert!(matches!(f.broker.advance_hosted_forced(f.broker.actor_revision(), position as u64, token, compute()).unwrap(), MonitoredStep::Released(_)));
        }
        let mut tokens = vec![1, 2];
        for position in 2..6 {
            let expected = raw.advance_sampled(position, budget(6)).unwrap();
            let actual = f.broker.advance_hosted_sampled(f.broker.actor_revision(), position, budget(6)).unwrap();
            match actual {
                MonitoredSampledStep::Released(actual) => {
                    assert_eq!(actual.choice(), &expected.choice);
                    assert_eq!(actual.reviewed().step().logits.as_ref(), expected.computation.logits.as_ref());
                    tokens.push(actual.choice().token);
                }
                other => panic!("quiet arithmetic held: {other:?}"),
            }
        }
        let message = f.dispatch(1, two_key.then_some(50));
        assert_eq!(f.broker.decoder_evidence(1).unwrap().unwrap().tokens(), tokens);
        let receipt = f.endpoint.deliver(&message).unwrap(); f.broker.accept_receipt(receipt).unwrap();
        assert_eq!(f.endpoint.execution_count(), 1); assert_eq!(f.endpoint.payload(), b"publish");
        let end = f.broker.hosted_decoder().unwrap();
        assert_eq!(end.sampled_draws, 4); assert_eq!(end.actor_revision, initial.actor_revision + 6);
        assert!(end.cache_bytes > initial.cache_bytes);
        assert_eq!(f.broker.inspect().ledger.charged, 16);
    }
}

#[test]
fn stale_actor_position_and_sample_budget_refuse_without_changing_evidence_or_rng() {
    let mut f = Fixture::new(false); let run = quiet(); let source = run.observation();
    f.broker.own_sampled_decoder(run, DecoderBindingLimits::default()).unwrap();
    f.broker.advance_hosted_forced(f.broker.actor_revision(), 0, 1, compute()).unwrap();
    let old = source.capture().unwrap(); let before = f.broker.hosted_decoder().unwrap();
    assert!(matches!(f.broker.advance_hosted_sampled(before.actor_revision - 1, 1, budget(6)), Err(Error::Stale)));
    assert!(matches!(f.broker.advance_hosted_sampled(before.actor_revision, 0, budget(6)), Err(Error::Stale)));
    let mut insufficient = budget(6); insufficient.sampling = SamplingBudget { vocabulary: 5 };
    assert!(matches!(f.broker.advance_hosted_sampled(before.actor_revision, 1, insufficient), Err(Error::Limit)));
    assert_eq!(f.broker.hosted_decoder().unwrap(), before); source.validate(&old).unwrap();
    assert!(matches!(f.broker.advance_hosted_sampled(before.actor_revision, 1, budget(6)).unwrap(), MonitoredSampledStep::Released(_)));
}

#[test]
fn computed_sample_hold_keeps_the_draw_and_reservation_but_never_exposes_a_new_effect() {
    let mut f = Fixture::new(false); let run = alarm(); let source = run.observation();
    f.broker.own_sampled_decoder(run, DecoderBindingLimits::default()).unwrap();
    f.broker.advance_hosted_forced(f.broker.actor_revision(), 0, 0, compute()).unwrap();
    let (action, inputs, permit) = approved(&mut f, 1);
    let before = f.broker.hosted_decoder().unwrap();
    assert!(matches!(f.broker.advance_hosted_sampled(before.actor_revision, 1, budget(2)).unwrap(), MonitoredSampledStep::Held(_)));
    let held = f.broker.hosted_decoder().unwrap();
    assert_eq!(held.status, MonitoringStatus::Held); assert_eq!(held.position, 2);
    assert_eq!(held.sampled_draws, 1); assert_eq!(held.actor_revision, before.actor_revision + 1);
    assert_eq!(source.availability(), DecoderAvailability::Held);
    assert_eq!(f.broker.inspect().ledger.reserved, 16);
    assert!(f.broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).is_err());
    let next = spec(&f); assert!(f.broker.propose(2, next, &snapshot()).is_err());
    assert!(matches!(f.broker.advance_hosted_sampled(held.actor_revision, 2, budget(2)), Err(Error::WrongState)));
    assert_eq!(f.broker.hosted_decoder().unwrap(), held);
    f.broker.cancel(1).unwrap(); assert_eq!(f.broker.inspect().ledger.available, 100);
    assert_eq!(f.endpoint.execution_count(), 0);
}

#[test]
fn external_actor_replacement_and_unpaired_checkpoint_reset_are_not_backdoors() {
    let mut f = Fixture::new(false);
    let checkpoint = f.broker.capture_checkpoint(1, 0).unwrap();
    f.broker.own_sampled_decoder(quiet(), DecoderBindingLimits::default()).unwrap();
    let state = f.broker.inspect(); let revision = f.broker.actor_revision();
    assert_eq!(f.broker.replace_actor_state(revision, control::actor(&[1])), Err(Error::WrongState));
    assert!(matches!(f.broker.capture_checkpoint(2, revision), Err(Error::WrongState)));
    assert_eq!(f.broker.reset(ResetRequest { checkpoint, expected_control_sequence: state.sequence,
        expected_actor_revision: revision, binding: ReviewBinding { round: 99, reducer_generation: 1, evidence_root: [1; 32] },
        retained_targets: TargetCeiling::new(&[f.endpoint.target()]).unwrap() }), Err(Error::WrongState));
    assert_eq!(f.broker.actor_revision(), revision); assert_eq!(f.broker.inspect(), state);
    assert!(matches!(f.broker.advance_hosted_forced(revision, 0, 1, compute()).unwrap(), MonitoredStep::Released(_)));
}

#[test]
fn stopping_the_original_controller_also_stops_owned_inference_before_a_draw() {
    let mut f = Fixture::new(false);
    f.broker.own_sampled_decoder(quiet(), DecoderBindingLimits::default()).unwrap();
    f.broker.advance_hosted_forced(f.broker.actor_revision(), 0, 1, compute()).unwrap();
    let before = f.broker.hosted_decoder().unwrap(); let state = f.broker.inspect();
    f.broker.request_stop(StopRequest { operation: 1, expected_control_sequence: state.sequence,
        expected_authority_epoch: state.ledger.epoch }).unwrap();
    assert!(matches!(f.broker.advance_hosted_sampled(before.actor_revision, before.position, budget(6)), Err(Error::WrongState)));
    assert_eq!(f.broker.hosted_decoder().unwrap(), before);
    assert!(f.broker.progress_stop(&mut f.endpoint).unwrap().progress.drained());
}

#[test]
fn owning_a_decoder_cannot_displace_an_already_pinned_external_source() {
    let mut f = Fixture::new(false); let original = control::run();
    f.broker.enable_decoder_monitoring(original.observation(), DecoderBindingLimits::default()).unwrap();
    let before = f.broker.inspect(); let revision = f.broker.actor_revision();
    assert_eq!(f.broker.own_sampled_decoder(quiet(), DecoderBindingLimits::default()), Err(Error::Duplicate));
    assert!(matches!(f.broker.hosted_decoder(), Err(Error::Incomplete)));
    assert_eq!(f.broker.inspect(), before); assert_eq!(f.broker.actor_revision(), revision);
}

#[test]
fn dropping_the_controller_closes_the_same_numeric_source_but_preserves_historical_evidence() {
    let mut f = Fixture::new(false); let run = quiet(); let source = run.observation();
    f.broker.own_sampled_decoder(run, DecoderBindingLimits::default()).unwrap();
    f.broker.advance_hosted_forced(f.broker.actor_revision(), 0, 1, compute()).unwrap();
    let evidence = source.capture().unwrap(); drop(f);
    assert_eq!(source.availability(), DecoderAvailability::Closed);
    assert_eq!(evidence.tokens(), &[1]); assert_eq!(source.validate(&evidence), Err(Error::Incomplete));
}
