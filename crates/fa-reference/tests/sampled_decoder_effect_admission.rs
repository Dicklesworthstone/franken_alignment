//! The sampled owner supplies the existing mandatory decoder-evidence gate.
//! Reuse the original congress, human key and endpoint, not a substitute permit.
#[path = "support/decoder_control.rs"]
#[allow(dead_code)]
mod support;
use support::*;
use fa_reference::action::consequence::activation::monitor::{RefinementBudget, RefinementMonitor};
use fa_reference::action::consequence::activation::monitor::decoder::MonitoredStep;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::{MonitoredSampledDecoder, MonitoredSampledStep};
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, DecoderIdentity, DecoderProfile, MAX_DECODER_PRODUCTS};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{SampleBudget, SamplingBudget, SamplingPolicy, SamplingStart};
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use fa_reference::action::consequence::oversight::human::HumanReviewPolicy;
use fa_reference::action::consequence::delivery::EndpointStatus;
use fa_reference::action::ElapsedTick;
use fa_reference::Error;
use std::collections::BTreeMap;

fn compute() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
fn sample_budget() -> SampleBudget { SampleBudget { decoder: compute(), sampling: SamplingBudget { vocabulary: 6 } } }
fn source(one_prefix_only: bool) -> MonitoredSampledDecoder {
    let p = numerical::fixture::profile(16);
    let p = DecoderProfile::new(DecoderIdentity { model_generation: 1, tokenizer_generation: 1, ..p.identity() },
        p.shape(), p.epsilon(), p.theta()).unwrap();
    let model = numerical::fixture::model(p);
    let local = RefinementBudget { encoded_bytes: 1_000_000, probe_coordinates: 1_000_000 };
    let monitors = (1..=model.profile().shape().layers as u64).map(|layer| {
        let contract = model.residual_contract(layer).unwrap();
        let mut weights = vec![0.0; contract.dimensions()]; weights[0] = 1.0;
        let probe = LinearProbe::new(1, 1, contract.profile(), &weights, 0.0, 1_000_000.0).unwrap();
        (layer, RefinementMonitor::new(vec![probe], vec![23], local).unwrap())
    }).collect::<BTreeMap<_, _>>();
    let budget = if one_prefix_only { RefinementBudget {
        encoded_bytes: 2 * (fa_reference::action::consequence::activation::HEADER_BYTES + 16), probe_coordinates: 8,
    } } else { local };
    MonitoredSampledDecoder::new(model, 7, 11, monitors, budget, SamplingStart {
        policy: SamplingPolicy::new(1, 1, 6, 0.7, 4, 1.0).unwrap(), stream: 99, seed: 0,
    }).unwrap()
}

#[test]
fn actual_sampled_prefix_supports_original_congress_and_both_publication_key_modes() {
    for two_key in [false, true] {
        let mut f = Fixture::new(two_key); let mut run = source(false);
        f.broker.enable_decoder_monitoring(run.observation(), DecoderBindingLimits::default()).unwrap();
        assert!(matches!(run.advance_forced(0, 1, compute()).unwrap(), MonitoredStep::Released(_)));
        let MonitoredSampledStep::Released(step) = run.advance_sampled(1, sample_budget()).unwrap() else { panic!("quiet sampled source held"); };
        let tokens = vec![1, step.choice().token];
        assert_eq!(run.observation().capture().unwrap().tokens(), tokens.as_slice());
        f.broker.replace_actor_state(f.broker.actor_revision(), actor(&tokens)).unwrap();
        let message = f.dispatch(1, two_key.then_some(50));
        let receipt = f.endpoint.deliver(&message).unwrap(); f.broker.accept_receipt(receipt).unwrap();
        assert_eq!(f.endpoint.payload(), b"publish"); assert_eq!(f.endpoint.execution_count(), 1);
        assert_eq!(f.broker.inspect().ledger.charged, 16);
        let evidence = f.broker.dispatched_decoder_evidence(1).unwrap().unwrap();
        assert_eq!(evidence.tokens(), tokens.as_slice()); run.observation().validate(evidence).unwrap();
        assert_eq!(run.sampled_draws(), 1);
    }
}

#[test]
fn sampled_monitor_exhaustion_blocks_preissued_automatic_and_human_keys_without_refund() {
    for two_key in [false, true] {
        let mut f = Fixture::new(false); let mut run = source(true);
        f.broker.enable_decoder_monitoring(run.observation(), DecoderBindingLimits::default()).unwrap();
        run.advance_forced(0, 1, compute()).unwrap();
        let reviewer = two_key.then(|| f.broker.enable_human_review(HumanReviewPolicy {
            reviewer_id: 90, max_validity_ticks: 100, max_requests: 16,
        }).unwrap());
        let (action, inputs, key) = approved(&mut f, 1);
        let human = reviewer.as_ref().map(|reviewer| {
            let request = f.broker.request_human_approval(101, 1, Some(&inputs), ElapsedTick(50)).unwrap();
            reviewer.approve(&request, ElapsedTick(1)).unwrap()
        });
        assert!(matches!(run.advance_sampled(1, sample_budget()).unwrap(), MonitoredSampledStep::Held(_)));
        assert_eq!(run.position(), 2); assert_eq!(run.sampled_draws(), 1);
        let before = f.broker.inspect();
        let result = match &human {
            Some(human) => f.broker.dispatch_with_human(&key, human, &action, Some(&inputs), &snapshot()),
            None => f.broker.dispatch(&key, &action, Some(&inputs), &snapshot()),
        };
        assert!(matches!(result, Err(Error::Incomplete)));
        assert_eq!(f.broker.inspect(), before); assert_eq!(before.ledger.reserved, 16);
        assert_eq!(before.ledger.charged, 0); assert_eq!(f.endpoint.execution_count(), 0);
        assert_eq!(f.broker.decoder_evidence(1).unwrap().unwrap().tokens(), &[1]);
        f.broker.cancel(1).unwrap(); assert_eq!(f.broker.inspect().ledger.available, 100);
    }
}

#[test]
fn already_sent_effect_reconciles_after_sample_hold_writer_loss_and_helper_input_loss() {
    for executed in [false, true] {
        let mut f = Fixture::new(true); let mut run = source(true);
        f.broker.enable_decoder_monitoring(run.observation(), DecoderBindingLimits::default()).unwrap();
        run.advance_forced(0, 1, compute()).unwrap();
        let message = f.dispatch(1, Some(50));
        if executed { let _lost = f.endpoint.deliver(&message).unwrap(); }
        f.broker.acknowledgment_lost(1).unwrap();
        assert!(matches!(run.advance_sampled(1, sample_budget()).unwrap(), MonitoredSampledStep::Held(_)));
        assert_eq!(run.sampled_draws(), 1); drop(run);
        f.broker.inputs_unavailable(1, 1).unwrap();
        let (mut broker, mut endpoint) = surviving_owners(f);
        broker.observe_time(ElapsedTick(100)).unwrap(); endpoint.observe_time(ElapsedTick(100)).unwrap();
        let fence = broker.restart_dispatcher().unwrap();
        broker.confirm_fence(endpoint.install_fence(fence).unwrap()).unwrap();
        let outcomes = broker.reconcile_pending(&mut endpoint).unwrap();
        assert!(matches!(outcomes[&1], Ok(EndpointStatus::Resolved(_))));
        assert_eq!(broker.inspect().ledger.charged, if executed { 16 } else { 0 });
        assert_eq!(broker.inspect().ledger.available, if executed { 84 } else { 100 });
        assert_eq!(endpoint.execution_count(), if executed { 1 } else { 0 });
        assert_eq!(broker.dispatched_decoder_evidence(1).unwrap().unwrap().tokens(), &[1]);
        assert!(broker.reconcile_pending(&mut endpoint).unwrap().is_empty());
    }
}
