//! Probe parameters retained by the SAME original monitored actor/controller.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod fixture;
#[path = "support/decoder_identity.rs"] mod numerical;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::activation::identity::{ModelPassport, decoder::IdentityProbeProgress};
use fa_reference::action::consequence::activation::monitor::{RefinementBudget, RefinementMonitor};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledDecoder;
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, MAX_DECODER_PRODUCTS};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{SamplingPolicy, SamplingStart};
use fa_reference::action::consequence::delivery::PublicationEndpoint;
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{OversightBroker, decoder_monitoring::DecoderBindingLimits};
use fa_reference::action::consequence::oversight::identity::{IdentityOutcome, IdentityPolicy, IdentityStatus};
use fa_reference::Error;
use std::collections::BTreeMap;

fn budget() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
fn broker() -> OversightBroker {
    let p = fixture::profile(); let d = p.delivery;
    let mut endpoint = PublicationEndpoint::new(d.target, d.initial_payload.clone(), d.retention_ticks, d.max_deliveries).unwrap();
    let mut broker = OversightBroker::new(ControllerConfig { scope: d.scope, total: d.total,
        max_attempts: d.max_attempts, actor: d.actor, suspend_at_incident: d.suspend_at_incident,
        policy: d.policy, congress: d.congress, narrowed_targets: TargetCeiling::new(&d.narrowed_targets).unwrap(),
    }, &mut endpoint, p.committee).unwrap();
    let acknowledgment = endpoint.install_fence(broker.fence_request()).unwrap();
    broker.confirm_fence(acknowledgment).unwrap();
    broker.observe_time(ElapsedTick(1)).unwrap(); broker
}
fn run(changed: bool) -> MonitoredSampledDecoder {
    let model = numerical::model(changed, false);
    let allowance = RefinementBudget { encoded_bytes: 10000, probe_coordinates: 10000 };
    let monitors: BTreeMap<_, _> = (1..=2).map(|layer| {
        let probe = LinearProbe::new(layer, 1, model.residual_contract(layer).unwrap().profile(), &[1.0, 0.0], 0.0, 100.0).unwrap();
        (layer, RefinementMonitor::new(vec![probe], vec![23], allowance).unwrap())
    }).collect();
    MonitoredSampledDecoder::new(model, 7, 11, monitors, allowance,
        SamplingStart { policy: SamplingPolicy::new(1, 1, 3, 1.0, 0, 1.0).unwrap(), stream: 77, seed: 9 }).unwrap()
}

#[test]
fn actual_owned_parameters_feed_native_identity_without_rewinding_or_sampling_the_actor() {
    for changed in [false, true] {
        let mut broker = broker();
        let passport = numerical::passport();
        let observer = broker.enable_identity_checks(passport.clone(), IdentityPolicy {
            observer_id: 91, timeout_ticks: 10, validity_ticks: 50, max_checks: 8,
        }).unwrap();
        broker.own_sampled_decoder(run(changed), DecoderBindingLimits::default()).unwrap();
        broker.advance_hosted_forced(broker.actor_revision(), 0, 1, budget()).unwrap();
        let before = broker.hosted_decoder().unwrap();
        let check = broker.begin_identity_check(1, broker.inspect().sequence, broker.actor_revision()).unwrap();
        let mut probe = broker.hosted_identity_probe(broker.actor_revision(), check.passport(), 1, budget()).unwrap();
        observer.observe_manifest(&check, passport.manifest().clone(), ElapsedTick(1)).unwrap();
        while !probe.complete() {
            if let IdentityProbeProgress::Measured(measurement) = probe.advance().unwrap() {
                observer.observe_anchor(&check, measurement.anchor(), measurement.source(), ElapsedTick(1)).unwrap();
            }
        }
        assert_eq!(broker.hosted_decoder().unwrap(), before);
        assert_eq!(before.position, 1); assert_eq!(before.sampled_draws, 0);
        assert_eq!(probe.work().completed_tokens, 5); assert_eq!(probe.work().completed_scalar_products, 382);
        let control = broker.inspect();
        let result = broker.apply_identity_check(&check, control.sequence, control.ledger.epoch).unwrap();
        if changed {
            assert!(matches!(result.report.outcome, IdentityOutcome::Mismatch(_)));
            assert!(broker.inspect().suspended);
            assert_eq!(broker.identity_status().unwrap(), IdentityStatus::Mismatch { check: 1 });
        } else {
            assert_eq!(result.report.outcome, IdentityOutcome::Matched);
            assert!(matches!(broker.identity_status().unwrap(), IdentityStatus::Matching { check: 1, .. }));
            assert!(!broker.inspect().suspended);
        }
    }
}

#[test]
fn missing_owner_stale_revision_and_foreign_tokenizer_cannot_select_a_standin_model() {
    let mut broker = broker(); let passport = numerical::passport();
    assert!(matches!(broker.hosted_identity_probe(0, &passport, 1, budget()), Err(Error::Incomplete)));
    broker.own_sampled_decoder(run(false), DecoderBindingLimits::default()).unwrap();
    assert!(matches!(broker.hosted_identity_probe(0, &passport, 1, budget()), Err(Error::Stale)));
    let mut manifest = passport.manifest().clone(); manifest.tokenizer_generation += 1;
    let foreign = ModelPassport::new(1, 1, manifest, passport.anchors().values().cloned().collect()).unwrap();
    let before = broker.hosted_decoder().unwrap();
    assert!(matches!(broker.hosted_identity_probe(broker.actor_revision(), &foreign, 1, budget()), Err(Error::Binding)));
    let probe = broker.hosted_identity_probe(broker.actor_revision(), &passport, 1, budget()).unwrap();
    assert_eq!(probe.work().entered_tokens, 0); assert_eq!(broker.hosted_decoder().unwrap(), before);
    assert_eq!(broker.inspect().ledger.available, 100);
    assert_eq!(broker.inspect().ledger.reserved, 0); assert_eq!(broker.inspect().ledger.charged, 0);
}
