//! Actual monitored inference driving the ORIGINAL durable stop and drain.
#![cfg(unix)]
#[path = "support/file_decoder.rs"] mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::activation::{HEADER_BYTES, monitor::MonitorOutcome};
use fa_reference::action::consequence::activation::monitor::decoder::{MonitoredStep, MonitoringStatus};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledStep;
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::observed::containment::FileResetRequest;
use fa_reference::action::consequence::delivery::persistent::observed::decoder::FileDecoderConfig;
use fa_reference::action::consequence::gate::ReviewBinding;
use fa_reference::action::consequence::oversight::decoder_host::{HostedStopCause, HostedStopPolicy};
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use fa_reference::Error;

fn stop_policy() -> HostedStopPolicy { HostedStopPolicy::new(91, 1, 9001).unwrap() }
fn boot(root: &Directory, config: &FileDecoderConfig) -> (FileOversight, FileHumanReviewer) {
    let (mut host, human) = FileOversight::create(root.store(), profile()).unwrap();
    host.enable_decoder(host.revision(), config.clone()).unwrap();
    host.enable_decoder_stop(host.revision(), stop_policy()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, human)
}

#[test]
fn quiet_inference_still_reaches_original_congress_two_key_publication_and_receipt() {
    let root = Directory::new(); let (mut host, human) = boot(&root, &configuration(3.0));
    assert_eq!(host.decoder_stop_policy().unwrap(), Some(stop_policy()));
    assert!(matches!(forced(&mut host, 0), MonitoredStep::Released(_)));
    assert!(matches!(sampled(&mut host), MonitoredSampledStep::Released(_)));
    assert_eq!(host.decoder_stop_incident().unwrap(), None);
    let keys = ready(&mut host, &human, 1, b"quiet result"); dispatch(&mut host, &keys);
    let outcome = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap().outcome;
    assert_eq!(outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(outcome));
    assert_eq!(host.inspect().payload, b"quiet result");
    assert_eq!(host.inspect().executions, 1);
    assert!(host.inspect().stop.is_none());
}

#[test]
fn alarm_threshold_and_observation_budget_loss_each_stop_without_disclosing_or_refunding_a_draw() {
    for (threshold, exhausted, expected) in [
        (1.5, false, MonitorOutcome::Alarm),
        (2.0, false, MonitorOutcome::AtThreshold),
        (3.0, true, MonitorOutcome::BudgetExhausted),
    ] {
        let root = Directory::new();
        let monitor = if exhausted {
            String::from_utf8(data::monitor(threshold)).unwrap().replace("\"encoded_bytes\":10000",
                &format!("\"encoded_bytes\":{}", HEADER_BYTES + 8)).into_bytes()
        } else { data::monitor(threshold) };
        let config = FileDecoderConfig::new(numerical_profile(), data::weights(), monitor,
            data::sampling(), 5, DecoderBindingLimits::default()).unwrap();
        let (mut host, human) = boot(&root, &config);
        assert!(matches!(forced(&mut host, 0), MonitoredStep::Released(_)));
        let n = host.decoder_inspection().unwrap().numerical;
        let cp = host.capture_decoder_checkpoint(host.revision(), 7, n.actor_revision, host.inspect().control.ledger.epoch).unwrap();
        let keys = ready(&mut host, &human, 1, b"not dispatched");
        assert!(matches!(sampled(&mut host), MonitoredSampledStep::Held(_)));
        let incident = host.decoder_stop_incident().unwrap().unwrap();
        assert_eq!(incident.cause(), HostedStopCause::Monitoring(expected));
        assert_eq!(incident.sampled_draws(), 1); assert_eq!(incident.position(), 2);
        assert_eq!(incident.last_stop_error(), None);
        let receipt = incident.stop_receipt().unwrap();
        assert_eq!(receipt.request().operation, stop_policy().operation());
        assert_eq!(receipt.cancelled(), &[1]); assert_eq!(receipt.refunded_units(), 16);
        assert_eq!(host.inspect().control.ledger.reserved, 0);
        assert_eq!(host.inspect().executions, 0);
        assert!(host.decoder_inspection().unwrap().paused);
        assert!(host.dispatch(host.revision(), &keys.automatic, &keys.human,
            &keys.action, &keys.inputs, snapshot()).is_err());
        let c = host.inspect().control;
        let request = FileResetRequest { operation: 100, expected_control_sequence: c.sequence,
            expected_actor_revision: host.decoder_inspection().unwrap().numerical.actor_revision,
            expected_authority_epoch: c.ledger.epoch, binding: ReviewBinding { round: 700,
                evidence_root: [17; 32], reducer_generation: 1 }, retained_targets: vec![host.inspect().target] };
        assert_eq!(host.reset_decoder_checkpoint(host.revision(), &cp, request, budget()).unwrap(), Err(Error::WrongState));
        assert_eq!(host.decoder_stop_incident().unwrap(), Some(incident.clone()));
        let n = host.decoder_inspection().unwrap().numerical;
        assert_eq!(n.sampled_draws, 1); assert_eq!(n.status, MonitoringStatus::Held);
        assert!(host.resume_decoder(host.revision(), n.actor_revision, n.position).is_err());
        drop(host);
        let (mut host, _) = FileOversight::open_with_decoder(root.store(), profile(), &config).unwrap();
        assert_eq!(host.decoder_stop_incident().unwrap(), Some(incident));
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        assert!(host.resume_decoder(host.revision(), n.actor_revision, n.position).is_err());
        assert!(host.progress_stop(host.revision(), ElapsedTick(2)).unwrap().progress.drained());
    }
}

#[test]
fn automatic_stop_preserves_executed_and_unknown_charges_until_original_endpoint_drain() {
    let root = Directory::new(); let (mut host, human) = boot(&root, &configuration(1.5));
    forced(&mut host, 0);
    let first = ready(&mut host, &human, 1, b"published"); dispatch(&mut host, &first);
    let second = ready(&mut host, &human, 2, b"not published"); dispatch(&mut host, &second);
    let third = ready(&mut host, &human, 3, b"not dispatched");
    assert_eq!(host.publish_checked(host.revision(), 1, Some(&first.inputs), snapshot(), ElapsedTick(1)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.inspect().control.ledger.charged, 32);
    assert!(matches!(sampled(&mut host), MonitoredSampledStep::Held(_)));
    let stopped = host.inspect();
    assert_eq!(stopped.control.ledger.charged, 32);
    assert_eq!(stopped.control.ledger.reserved, 0);
    assert_eq!(stopped.stop.as_ref().unwrap().cancelled(), &[3]);
    assert_eq!(stopped.payload, b"published"); assert_eq!(stopped.executions, 1);
    assert!(!host.stop_progress().unwrap().drained());
    assert!(host.dispatch(host.revision(), &third.automatic, &third.human,
        &third.action, &third.inputs, snapshot()).is_err());
    let drained = host.progress_stop(host.revision(), ElapsedTick(2)).unwrap();
    assert!(drained.progress.drained());
    assert_eq!(drained.outcomes[&1], Ok(Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 })));
    assert_eq!(drained.outcomes[&2], Ok(Reconciliation::Resolved(EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed })));
    let state = host.inspect();
    assert_eq!(state.control.ledger.charged, 16);
    assert_eq!(state.control.ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(state.control.ledger.stages[&2], ActionState::ConfirmedNotExecuted);
    assert_eq!(state.control.ledger.stages[&3], ActionState::Cancelled);
    assert_eq!(state.payload, b"published"); assert_eq!(state.executions, 1);
}

#[test]
fn entered_arithmetic_failure_stops_but_precomputation_refusals_do_not() {
    let root = Directory::new(); let (mut control, _) = boot(&root, &configuration(3.0));
    let n = control.decoder_inspection().unwrap().numerical;
    let zero = fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderBudget { scalar_products: 0 };
    assert!(matches!(control.advance_decoder_forced(control.revision(), n.actor_revision, 0, 0, zero).unwrap(), Err(Error::Limit)));
    assert!(matches!(control.advance_decoder_forced(control.revision(), n.actor_revision, 0, u32::MAX, budget()).unwrap(), Err(Error::InvalidInput)));
    assert_eq!(control.decoder_stop_incident().unwrap(), None);
    assert!(matches!(forced(&mut control, 0), MonitoredStep::Released(_)));
    let other = Directory::new(); let mut weights = data::weights();
    let header = u64::from_le_bytes(weights[..8].try_into().unwrap()) as usize;
    assert!(std::str::from_utf8(&weights[8..8 + header]).unwrap().starts_with("{\"lm_head.weight\":"));
    // Finite admitted weight, but MAX * normalized([1,0])[0] overflows f32.
    weights[8 + header..12 + header].copy_from_slice(&f32::MAX.to_le_bytes());
    let config = FileDecoderConfig::new(numerical_profile(), weights, data::monitor(3.0),
        data::sampling(), 5, DecoderBindingLimits::default()).unwrap();
    let (mut host, _) = boot(&other, &config); let n = host.decoder_inspection().unwrap().numerical;
    assert!(matches!(host.advance_decoder_forced(host.revision(), n.actor_revision, 0, 0, budget()).unwrap(), Err(Error::Overflow)));
    let incident = host.decoder_stop_incident().unwrap().unwrap();
    assert_eq!(incident.cause(), HostedStopCause::Numerical(Error::Overflow));
    assert!(incident.stop_receipt().is_some());
    assert_eq!(incident.position(), 0); assert_eq!(incident.sampled_draws(), 0);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn policy_is_frozen_before_work_and_legacy_manual_reset_mode_is_not_relabeled() {
    let root = Directory::new(); let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
    assert_eq!(host.enable_decoder_stop(host.revision(), stop_policy()), Err(JournalError::Contract(Error::Incomplete)));
    host.enable_decoder(host.revision(), configuration(1.5)).unwrap();
    host.enable_decoder_stop(host.revision(), stop_policy()).unwrap();
    assert_eq!(host.enable_decoder_stop(host.revision(), stop_policy()), Err(JournalError::Contract(Error::Duplicate)));
    assert_eq!(host.decoder_stop_policy().unwrap(), Some(stop_policy()));
    let other = Directory::new(); let (mut legacy, _, _) = create_decoder(&other, 1.5);
    forced(&mut legacy, 0);
    assert_eq!(legacy.enable_decoder_stop(legacy.revision(), stop_policy()), Err(JournalError::Contract(Error::WrongState)));
    assert!(matches!(sampled(&mut legacy), MonitoredSampledStep::Held(_)));
    assert_eq!(legacy.decoder_stop_policy().unwrap(), None);
    assert!(legacy.inspect().stop.is_none());
}

#[test]
fn comparison_witness_cannot_substitute_a_stop_result_during_replay() {
    let root = Directory::new(); let (mut host, _) = boot(&root, &configuration(1.5));
    forced(&mut host, 0); sampled(&mut host);
    let path = root.store().join("delivery.bin"); let original = std::fs::read(&path).unwrap();
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
    let mut changed = original.clone(); *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, changed).unwrap();
    assert!(matches!(FileOversight::read_publication(root.store(), &profile()), Err(JournalError::Contract(Error::Binding))));
    std::fs::write(&path, original).unwrap();
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
}
