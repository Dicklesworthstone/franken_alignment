//! Actual monitored inference is a prerequisite, not a replacement permit.
#[path = "support/decoder_control.rs"]
mod support;
use support::*;
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::activation::monitor::decoder::observation::DecoderAvailability;
use fa_reference::action::consequence::oversight::decoder_monitoring::{DecoderBindingLimits, DecoderBindingUsage};
use fa_reference::action::consequence::oversight::human::HumanReviewPolicy;
use fa_reference::action::consequence::delivery::EndpointStatus;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fa_reference::Error;

fn attached(two_key: bool) -> (Fixture, fa_reference::action::consequence::activation::monitor::decoder::MonitoredDecoder) {
    let mut f = Fixture::new(two_key); let mut source = run();
    f.broker.enable_decoder_monitoring(source.observation(), DecoderBindingLimits::default()).unwrap();
    source.advance(0, 1, numerical::compute()).unwrap();
    (f, source)
}

#[test]
fn empty_or_wrong_same_length_token_prefix_cannot_admit_but_matching_live_prefix_can_publish() {
    let mut f = Fixture::new(false); let mut source = run();
    f.broker.enable_decoder_monitoring(source.observation(), DecoderBindingLimits::default()).unwrap();
    let proposed = spec(&f);
    assert!(matches!(f.broker.propose(1, proposed.clone(), &snapshot()), Err(Error::Incomplete)));
    source.advance(0, 2, numerical::compute()).unwrap();
    assert!(matches!(f.broker.propose(1, proposed, &snapshot()), Err(Error::Binding)));
    assert!(f.broker.inspect().ledger.stages.is_empty());
    assert_eq!(f.broker.decoder_binding_usage(), Some(DecoderBindingUsage::default()));
    f.broker.replace_actor_state(f.broker.actor_revision(), actor(&[2])).unwrap();
    let (action, inputs) = admit(&mut f, 1);
    assert!(f.broker.authorize(1, Some(&inputs), &snapshot()).is_err());
    assert!(matches!(f.broker.dispatched_decoder_evidence(1), Err(Error::Missing)));
    let review = ballot(&mut f.broker, 1, 11, Verdict::Allow);
    f.broker.apply_review(review, Some(&inputs), &snapshot()).unwrap();
    let key = f.broker.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let message = f.broker.dispatch(&key, &action, Some(&inputs), &snapshot()).unwrap();
    let receipt = f.endpoint.deliver(&message).unwrap(); f.broker.accept_receipt(receipt).unwrap();
    assert_eq!(f.endpoint.payload(), b"publish"); assert_eq!(f.endpoint.execution_count(), 1);
    let consumed = f.broker.dispatched_decoder_evidence(1).unwrap().unwrap();
    assert_eq!(consumed.tokens(), &[2]); assert_eq!(consumed.review().unreviewed_layers(), 0);
    assert_eq!(f.broker.inspect().ledger.charged, 16);
}

#[test]
fn original_human_key_path_still_publishes_with_complete_monitored_inference() {
    let (mut f, source) = attached(true);
    let message = f.dispatch(1, Some(50));
    assert!(message.request().approval().is_some());
    let receipt = f.endpoint.deliver(&message).unwrap(); f.broker.accept_receipt(receipt).unwrap();
    assert_eq!(f.endpoint.execution_count(), 1);
    assert_eq!(f.broker.dispatched_decoder_evidence(1).unwrap().unwrap().tokens(), &[1]);
    assert_eq!(source.observation().availability(), DecoderAvailability::Ready);
}

#[test]
fn live_source_change_during_review_blocks_continue_but_does_not_block_a_restrictive_review() {
    for verdict in [Verdict::Allow, Verdict::Hold] {
        let (mut f, mut source) = attached(false);
        let (_, inputs) = admit(&mut f, 1);
        let review = ballot(&mut f.broker, 1, 11, verdict);
        source.advance(1, 2, numerical::compute()).unwrap();
        let result = f.broker.apply_review(review, Some(&inputs), &snapshot());
        if verdict == Verdict::Allow {
            assert!(matches!(result, Err(Error::Stale)));
            assert!(!f.broker.inspect().decisions.contains_key(&1));
        } else {
            assert_ne!(result.unwrap().policy.control.decision.consequence, Consequence::Continue);
        }
        assert_eq!(f.broker.inspect().ledger.reserved, 0);
        assert_eq!(f.endpoint.execution_count(), 0);
    }
}

#[test]
fn both_key_dispatches_recheck_live_source_without_consuming_or_refunding_reserved_authority() {
    for two_key in [false, true] {
        let (mut f, mut source) = attached(false);
        let reviewer = two_key.then(|| f.broker.enable_human_review(HumanReviewPolicy {
            reviewer_id: 90, max_validity_ticks: 100, max_requests: 16,
        }).unwrap());
        let (action, inputs, key) = approved(&mut f, 1);
        let human = reviewer.as_ref().map(|reviewer| {
            let request = f.broker.request_human_approval(101, 1, Some(&inputs), ElapsedTick(50)).unwrap();
            reviewer.approve(&request, ElapsedTick(1)).unwrap()
        });
        source.advance(1, 2, numerical::compute()).unwrap();
        let before = f.broker.inspect();
        let result = match &human {
            Some(human) => f.broker.dispatch_with_human(&key, human, &action, Some(&inputs), &snapshot()),
            None => f.broker.dispatch(&key, &action, Some(&inputs), &snapshot()),
        };
        assert!(matches!(result, Err(Error::Stale)));
        assert_eq!(f.broker.inspect(), before);
        assert_eq!(before.ledger.reserved, 16); assert_eq!(before.ledger.charged, 0);
        assert_eq!(f.endpoint.execution_count(), 0);
        assert_eq!(f.broker.decoder_evidence(1).unwrap().unwrap().tokens(), &[1]);
    }
}

#[test]
fn owner_loss_is_not_repaired_by_a_lookalike_source_or_a_copied_quiet_report() {
    let (mut f, source) = attached(false);
    let (action, inputs, key) = approved(&mut f, 1);
    let retained = f.broker.decoder_evidence(1).unwrap().unwrap().clone();
    drop(source);
    let mut replacement = run(); replacement.advance(0, 1, numerical::compute()).unwrap();
    assert_eq!(replacement.observation().capture().unwrap().tokens(), retained.tokens());
    assert_eq!(f.broker.enable_decoder_monitoring(replacement.observation(), DecoderBindingLimits::default()), Err(Error::Duplicate));
    assert!(matches!(f.broker.dispatch(&key, &action, Some(&inputs), &snapshot()), Err(Error::Incomplete)));
    assert_eq!(f.broker.inspect().ledger.reserved, 16);
    assert_eq!(f.endpoint.execution_count(), 0);
    assert_eq!(retained.review().unreviewed_layers(), 0);
}

#[test]
fn actor_replacement_cannot_reuse_a_permit_even_when_the_original_token_ids_are_identical() {
    let (mut f, _source) = attached(false);
    let (action, inputs, key) = approved(&mut f, 1);
    f.broker.replace_actor_state(f.broker.actor_revision(), actor(&[1])).unwrap();
    assert!(matches!(f.broker.dispatch(&key, &action, Some(&inputs), &snapshot()), Err(Error::Stale)));
    assert_eq!(f.broker.inspect().ledger.reserved, 16);
    assert_eq!(f.broker.inspect().ledger.charged, 0);
}

#[test]
fn advanced_quiet_inference_can_support_new_work_but_cannot_refresh_an_old_permit() {
    let (mut f, mut source) = attached(false);
    let (old_action, old_inputs, old_key) = approved(&mut f, 1);
    source.advance(1, 2, numerical::compute()).unwrap();
    f.broker.replace_actor_state(f.broker.actor_revision(), actor(&[1, 2])).unwrap();
    assert!(matches!(f.broker.dispatch(&old_key, &old_action, Some(&old_inputs), &snapshot()), Err(Error::Stale)));
    f.broker.cancel(1).unwrap();
    assert_eq!(f.broker.inspect().ledger.available, 100);
    let (action, inputs, key) = approved(&mut f, 2);
    let message = f.broker.dispatch(&key, &action, Some(&inputs), &snapshot()).unwrap();
    let receipt = f.endpoint.deliver(&message).unwrap(); f.broker.accept_receipt(receipt).unwrap();
    assert_eq!(f.endpoint.execution_count(), 1);
    assert_eq!(f.broker.dispatched_decoder_evidence(2).unwrap().unwrap().tokens(), &[1, 2]);
    assert_eq!(f.broker.decoder_evidence(1).unwrap().unwrap().tokens(), &[1]);
    assert_eq!(f.broker.inspect().ledger.stages[&1], ActionState::Cancelled);
    assert!(f.broker.dispatch(&old_key, &old_action, Some(&old_inputs), &snapshot()).is_err());
}

#[test]
fn uncertain_effects_reconcile_after_decoder_and_helper_input_loss_without_reexecution() {
    for two_key in [false, true] {
        for executed in [false, true] {
            let (mut f, source) = attached(two_key);
            let message = f.dispatch(1, two_key.then_some(50));
            if executed { let _lost_receipt = f.endpoint.deliver(&message).unwrap(); }
            f.broker.acknowledgment_lost(1).unwrap();
            drop(source);
            f.broker.inputs_unavailable(1, 1).unwrap();
            // Drop the fixture's private reviewer role. Recovery must use only
            // the surviving original controller and endpoint, never new keys.
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
}

#[test]
fn retained_binding_limits_are_cumulative_and_failed_policy_admission_does_not_spend_them() {
    let mut f = Fixture::new(false); let mut source = run();
    f.broker.enable_decoder_monitoring(source.observation(), DecoderBindingLimits {
        token_ids: 1, ..DecoderBindingLimits::default()
    }).unwrap();
    source.advance(0, 1, numerical::compute()).unwrap();
    let mut expired = spec(&f); expired.deadline = ElapsedTick(1);
    assert!(f.broker.propose(1, expired, &snapshot()).is_err());
    assert_eq!(f.broker.decoder_binding_usage(), Some(DecoderBindingUsage::default()));
    let (action, inputs, key) = approved(&mut f, 1);
    let usage = f.broker.decoder_binding_usage().unwrap();
    assert_eq!(usage.token_ids, 1); assert!(usage.score_words > 0);
    let proposed = spec(&f);
    assert!(matches!(f.broker.propose(2, proposed, &snapshot()), Err(Error::Limit)));
    assert_eq!(f.broker.decoder_binding_usage(), Some(usage));
    let message = f.broker.dispatch(&key, &action, Some(&inputs), &snapshot()).unwrap();
    let receipt = f.endpoint.deliver(&message).unwrap(); f.broker.accept_receipt(receipt).unwrap();
    assert_eq!(f.endpoint.execution_count(), 1);
}

#[test]
fn incompatible_model_identity_refuses_without_installing_a_partial_gate() {
    let mut f = Fixture::new(false);
    let mismatched = numerical::quiet(); // model/tokenizer generations 3/4, not this controller's 1/1
    assert_eq!(f.broker.enable_decoder_monitoring(mismatched.observation(), DecoderBindingLimits::default()), Err(Error::Binding));
    assert!(!f.broker.decoder_monitoring_required());
    let mut source = run(); source.advance(0, 1, numerical::compute()).unwrap();
    f.broker.enable_decoder_monitoring(source.observation(), DecoderBindingLimits::default()).unwrap();
    let message = f.dispatch(1, None);
    assert!(f.endpoint.deliver(&message).is_ok());
}
