//! Actual monitored numerical restart and ORIGINAL effect-authority transitions.
#[path = "support/hosted_decoder.rs"]
#[allow(dead_code)]
mod support;
use support::*;
use fa_reference::action::consequence::activation::monitor::RefinementBudget;
use fa_reference::action::consequence::activation::monitor::decoder::{MonitoredStep, MonitoringStatus};
use fa_reference::action::consequence::activation::monitor::decoder::observation::DecoderAvailability;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledStep;
use fa_reference::action::consequence::gate::{ReviewBinding, TargetCeiling};
use fa_reference::action::consequence::delivery::StopRequest;
use fa_reference::action::consequence::oversight::decoder_host::{HostedCheckpointHandle, HostedResetRequest};
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use fa_reference::action::ActionState;
use fa_reference::Error;

fn force(f: &mut Fixture, token: u32) -> MonitoredStep {
    let host = f.broker.hosted_decoder().unwrap();
    f.broker.advance_hosted_forced(host.actor_revision, host.position, token, compute()).unwrap()
}
fn sample(f: &mut Fixture) -> (u32, Vec<u32>) {
    let host = f.broker.hosted_decoder().unwrap();
    match f.broker.advance_hosted_sampled(host.actor_revision, host.position, budget(6)).unwrap() {
        MonitoredSampledStep::Released(step) => (step.choice().token,
            step.reviewed().step().logits.iter().map(|v| v.to_bits()).collect()),
        other => panic!("quiet fixture held: {other:?}"),
    }
}
fn request(f: &Fixture, checkpoint: &HostedCheckpointHandle, round: u64) -> HostedResetRequest {
    let state = f.broker.inspect();
    HostedResetRequest { checkpoint: checkpoint.clone(), expected_control_sequence: state.sequence,
        expected_actor_revision: f.broker.actor_revision(), expected_authority_epoch: state.ledger.epoch,
        binding: ReviewBinding { round, reducer_generation: 1, evidence_root: [9; 32] },
        retained_targets: TargetCeiling::new(&[f.endpoint.target()]).unwrap(), replay_budget: compute() }
}
fn capture(f: &mut Fixture, id: u64) -> HostedCheckpointHandle {
    f.broker.capture_hosted_checkpoint(id, f.broker.actor_revision()).unwrap()
}

#[test]
fn paired_reset_reproduces_stochastic_choices_and_all_logits_without_rewinding_costs() {
    let mut f = Fixture::new(false);
    let run = quiet(); let old_source = run.observation();
    f.broker.own_sampled_decoder(run, DecoderBindingLimits::default()).unwrap();
    for token in [1, 2, 0] { assert!(matches!(force(&mut f, token), MonitoredStep::Released(_))); }
    sample(&mut f); sample(&mut f);
    let checkpoint = capture(&mut f, 1);
    let old = old_source.capture().unwrap();
    let expected: Vec<_> = (0..8).map(|_| sample(&mut f)).collect();
    let before = f.broker.hosted_decoder().unwrap();
    let reset = f.broker.reset_hosted_decoder(request(&f, &checkpoint, 100)).unwrap();
    assert!(reset.control.restored); assert_eq!(reset.position, 5); assert_eq!(reset.sampled_draws, 2);
    assert_eq!(reset.resumed_stream, Some(8));
    assert_eq!(reset.actor_revision, before.actor_revision + 2);
    assert_eq!(reset.control.actor_revision + 1, reset.actor_revision);
    assert_eq!(reset.replay_numerical.tokens, 5);
    let after = f.broker.hosted_decoder().unwrap();
    assert_eq!(after.numerical.tokens, before.numerical.tokens + 5);
    assert_eq!(after.numerical.scalar_products().unwrap(), before.numerical.scalar_products().unwrap()
        + reset.replay_numerical.scalar_products().unwrap());
    assert!(after.monitoring.encoded_bytes > before.monitoring.encoded_bytes);
    assert_eq!(old_source.availability(), DecoderAvailability::Closed);
    assert!(old_source.validate(&old).is_err());
    let actual: Vec<_> = (0..8).map(|_| sample(&mut f)).collect();
    assert_eq!(actual, expected);
    assert_eq!(f.broker.hosted_recovery_usage().unwrap().replay_attempts, 1);
}

#[test]
fn learned_hold_can_reset_but_old_reserved_permit_cannot_survive_the_rewind() {
    let mut f = Fixture::new(false);
    f.broker.own_sampled_decoder(alarm(), DecoderBindingLimits::default()).unwrap();
    force(&mut f, 0); let checkpoint = capture(&mut f, 1);
    let (action, inputs, permit) = approved(&mut f, 1);
    let old_evidence = f.broker.decoder_evidence(1).unwrap().unwrap().clone();
    assert!(matches!(force(&mut f, 1), MonitoredStep::Held(_)));
    assert_eq!(f.broker.hosted_decoder().unwrap().status, MonitoringStatus::Held);
    assert!(f.broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).is_err());
    let reset = f.broker.reset_hosted_decoder(request(&f, &checkpoint, 100)).unwrap();
    assert_eq!(reset.control.cancelled, vec![1]); assert_eq!(reset.control.refunded_units, 16);
    assert_eq!(reset.control.revocation_floor, 1); assert_eq!(f.broker.inspect().ledger.available, 100);
    assert_eq!(f.broker.inspect().ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(f.broker.decoder_evidence(1).unwrap().unwrap().stream(), old_evidence.stream());
    assert!(f.broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).is_err());
    let next = f.dispatch(2, None);
    let receipt = f.endpoint.deliver(&next).unwrap(); f.broker.accept_receipt(receipt).unwrap();
    assert_eq!(f.endpoint.execution_count(), 1); assert_eq!(f.broker.inspect().ledger.charged, 16);
    assert_eq!(f.broker.decoder_evidence(2).unwrap().unwrap().stream(), 8);
}

#[test]
fn unknown_publications_and_second_keys_are_not_refunded_or_reissued_by_reset() {
    for two_key in [false, true] {
        let mut f = Fixture::new(two_key);
        f.broker.own_sampled_decoder(quiet(), DecoderBindingLimits::default()).unwrap();
        force(&mut f, 0); let checkpoint = capture(&mut f, 1);
        let message = f.dispatch(1, two_key.then_some(50));
        let receipt = f.endpoint.deliver(&message).unwrap();
        f.broker.acknowledgment_lost(1).unwrap();
        force(&mut f, 1);
        let reset = f.broker.reset_hosted_decoder(request(&f, &checkpoint, 100)).unwrap();
        assert_eq!(reset.control.refunded_units, 0); assert!(reset.control.cancelled.is_empty());
        assert_eq!(f.broker.inspect().ledger.charged, 16);
        assert_eq!(f.broker.inspect().ledger.stages[&1], ActionState::Unknown);
        f.broker.inputs_unavailable(1, f.broker.input_revision(1).unwrap()).unwrap();
        assert!(f.broker.accept_receipt(receipt.clone()).unwrap());
        assert!(!f.broker.accept_receipt(receipt).unwrap());
        assert_eq!(f.endpoint.execution_count(), 1); assert_eq!(f.broker.inspect().ledger.charged, 16);
        let fresh = f.dispatch(2, two_key.then_some(50));
        let receipt = f.endpoint.deliver(&fresh).unwrap(); f.broker.accept_receipt(receipt).unwrap();
        assert_eq!(f.endpoint.execution_count(), 2); assert_eq!(f.broker.inspect().ledger.charged, 32);
    }
}

#[test]
fn invalid_handles_predecessors_and_numerical_budget_cannot_start_a_replay() {
    let mut f = Fixture::new(false); let mut foreign = Fixture::new(false);
    f.broker.own_sampled_decoder(quiet(), DecoderBindingLimits::default()).unwrap();
    foreign.broker.own_sampled_decoder(quiet(), DecoderBindingLimits::default()).unwrap();
    force(&mut f, 0); force(&mut foreign, 0);
    let checkpoint = capture(&mut f, 1); let other = capture(&mut foreign, 1);
    let original = request(&f, &checkpoint, 100);
    let before = f.broker.hosted_decoder().unwrap(); let usage = f.broker.hosted_recovery_usage().unwrap();
    let control = f.broker.inspect();
    for kind in 0..6 {
        let mut bad = original.clone();
        match kind {
            0 => bad.checkpoint = other.clone(),
            1 => bad.expected_actor_revision += 1,
            2 => bad.expected_control_sequence += 1,
            3 => bad.expected_authority_epoch += 1,
            4 => bad.replay_budget.scalar_products = 0,
            _ => bad.binding.evidence_root = [0; 32],
        }
        assert!(f.broker.reset_hosted_decoder(bad).is_err());
        assert_eq!(f.broker.hosted_decoder().unwrap(), before);
        assert_eq!(f.broker.hosted_recovery_usage().unwrap(), usage);
        assert_eq!(f.broker.inspect(), control);
    }
    assert!(f.broker.reset_hosted_decoder(original).unwrap().control.restored);
}

#[test]
fn replay_monitor_exhaustion_preserves_charges_and_cannot_restore_old_quiet_eligibility() {
    let mut f = Fixture::new(false);
    let allowance = RefinementBudget { encoded_bytes: 1_000_000, probe_coordinates: 8 };
    let run = monitored(model(), 1_000_000.0, allowance); let source = run.observation();
    f.broker.own_sampled_decoder(run, DecoderBindingLimits::default()).unwrap();
    force(&mut f, 0); let checkpoint = capture(&mut f, 1);
    let (_, _, _reserved) = approved(&mut f, 1);
    let before = f.broker.inspect(); let revision = f.broker.actor_revision();
    let original_work = f.broker.hosted_decoder().unwrap().monitoring;
    assert_eq!(original_work.probe_coordinates, 8);
    for round in [100, 101] {
        assert_eq!(f.broker.reset_hosted_decoder(request(&f, &checkpoint, round)).unwrap_err(), Error::Incomplete);
        assert_eq!(f.broker.inspect(), before); assert_eq!(f.broker.actor_revision(), revision);
        assert_eq!(f.broker.hosted_decoder().unwrap().position, 1);
        assert_eq!(f.broker.hosted_decoder().unwrap().monitoring.probe_coordinates, 8);
        assert!(source.capture().is_err());
    }
    assert_eq!(f.broker.hosted_recovery_usage().unwrap().replay_attempts, 2);
    assert_eq!(f.broker.hosted_decoder().unwrap().numerical.tokens, 3);
    assert_eq!(f.broker.inspect().ledger.reserved, 16);
    assert!(f.broker.propose(2, spec(&f), &snapshot()).is_err());
    f.broker.cancel(1).unwrap(); assert_eq!(f.broker.inspect().ledger.available, 100);
}

#[test]
fn original_incident_escalation_suspends_instead_of_installing_another_quiet_owner() {
    let mut f = Fixture::new(false);
    f.broker.own_sampled_decoder(quiet(), DecoderBindingLimits::default()).unwrap();
    force(&mut f, 0); let checkpoint = capture(&mut f, 1);
    for incident in 1..=3 {
        let reset = f.broker.reset_hosted_decoder(request(&f, &checkpoint, 100 + incident)).unwrap();
        assert_eq!(reset.control.incident_count, incident);
        assert_eq!(reset.control.restored, incident < 3);
        assert_eq!(reset.resumed_stream.is_some(), incident < 3);
    }
    assert!(f.broker.inspect().suspended);
    let host = f.broker.hosted_decoder().unwrap();
    assert!(f.broker.advance_hosted_sampled(host.actor_revision, host.position, budget(6)).is_err());
    assert!(f.broker.reset_hosted_decoder(request(&f, &checkpoint, 200)).is_err());
    assert!(f.broker.capture_hosted_checkpoint(2, f.broker.actor_revision()).is_err());
    assert_eq!(f.endpoint.execution_count(), 0);
}

#[test]
fn quiet_nonempty_capture_limits_and_terminal_stop_have_no_unpaired_escape() {
    let mut f = Fixture::new(false);
    f.broker.own_sampled_decoder(quiet(), DecoderBindingLimits::default()).unwrap();
    assert!(f.broker.capture_hosted_checkpoint(1, f.broker.actor_revision()).is_err());
    force(&mut f, 0);
    let first = capture(&mut f, 1);
    assert!(f.broker.capture_checkpoint(99, f.broker.actor_revision()).is_err());
    assert!(f.broker.capture_hosted_checkpoint(1, f.broker.actor_revision()).is_err());
    for id in 2..=32 { capture(&mut f, id); }
    assert_eq!(f.broker.hosted_recovery_usage().unwrap().checkpoints, 32);
    assert_eq!(f.broker.capture_hosted_checkpoint(33, f.broker.actor_revision()).unwrap_err(), Error::Limit);
    let state = f.broker.inspect();
    f.broker.request_stop(StopRequest { operation: 1, expected_control_sequence: state.sequence,
        expected_authority_epoch: state.ledger.epoch }).unwrap();
    let usage = f.broker.hosted_recovery_usage().unwrap();
    assert!(f.broker.reset_hosted_decoder(request(&f, &first, 100)).is_err());
    assert_eq!(f.broker.hosted_recovery_usage().unwrap(), usage);
    assert!(f.broker.progress_stop(&mut f.endpoint).unwrap().progress.drained());
}
