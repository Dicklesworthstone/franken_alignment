//! Captured state is a prerequisite of original permits, not a second policy.
#[path = "support/actor_gateway.rs"]
mod support;
use support::{fixture, proposal, review, snapshot};
use fa_reference::action::{ActionState, ElapsedTick, Purpose, Scope};
use fa_reference::action::consequence::delivery::EndpointStatus;
use fa_reference::action::consequence::oversight::{DispatchKeys, ReviewWindow};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, IntakeLimits, Knowledge};
use fa_reference::action::consequence::oversight::human::HumanReviewPolicy;
use fa_reference::action::consequence::oversight::policy_state::*;
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::collections::BTreeMap;

fn source(generation: u64) -> StateSource {
    StateSource { scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5,
        purpose: Purpose::Effect }, source: 70, generation }
}
fn seed(writer: &PolicyStateWriter) {
    writer.record(1, &StateEvent::Snapshot { semantic_epoch: 1, values: snapshot().values }).unwrap();
    writer.close(1, 1).unwrap();
}
fn change(key: u64, before: Option<Vec<u8>>, after: Option<Vec<u8>>) -> StateEvent {
    StateEvent::Delta { semantic_epoch: 1, changes: vec![StateChange { key, before, after }] }
}

#[test]
fn both_dispatch_paths_refuse_unclosed_state_and_stale_snapshots_then_use_current_capture() {
    for two_key in [false, true] {
        let (port, mut supervisor, mut endpoint) = fixture(IntakeLimits::default());
        let writer = supervisor.broker_mut().enable_policy_state(source(1), StateLimits::default()).unwrap();
        seed(&writer);
        let reviewer = two_key.then(|| supervisor.broker_mut().enable_human_review(HumanReviewPolicy {
            reviewer_id: 90, max_validity_ticks: 20, max_requests: 16,
        }).unwrap());
        let ticket = port.submit(1, &proposal()).unwrap();
        supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
        let inputs = review(&mut supervisor, 1, 11);
        let automatic = supervisor.authorize_request(1, Some(&inputs), &snapshot()).unwrap();
        let human = reviewer.as_ref().map(|reviewer| {
            let request = supervisor.broker_mut().request_human_approval(101, 1, Some(&inputs), ElapsedTick(10)).unwrap();
            reviewer.approve(&request, ElapsedTick(1)).unwrap()
        });
        let keys = || DispatchKeys { automatic: &automatic, human: human.as_ref() };
        let before = supervisor.broker().inspect();
        // The policy reads key 7 only. This unrelated new key still makes the
        // old whole Snapshot stale, independently of policy-witness invalidation.
        writer.record(2, &change(99, None, Some(vec![1]))).unwrap();
        assert_eq!(supervisor.dispatch_request(1, keys(), Some(&inputs), &snapshot()).unwrap_err(), Error::Incomplete);
        assert_eq!(supervisor.broker().inspect(), before);
        writer.close(2, 2).unwrap();
        assert_eq!(supervisor.dispatch_request(1, keys(), Some(&inputs), &snapshot()).unwrap_err(), Error::Binding);
        let current = supervisor.broker().capture_policy_state().unwrap();
        let envelope = supervisor.dispatch_request(1, keys(), Some(&inputs), current.snapshot()).unwrap();
        let retained = supervisor.broker().delivery_policy_state(1).unwrap().unwrap().clone();
        assert_eq!(retained, current);
        assert_eq!(retained.frontier().through, 2);
        writer.withdraw();
        supervisor.accept_receipt(endpoint.deliver(&envelope).unwrap()).unwrap();
        assert_eq!(endpoint.execution_count(), 1);
        assert_eq!(supervisor.broker().delivery_policy_state(1).unwrap(), Some(&retained));
        assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    }
}

#[test]
fn changed_policy_input_cannot_be_hidden_by_replaying_the_old_complete_snapshot() {
    let (port, mut supervisor, endpoint) = fixture(IntakeLimits::default());
    let writer = supervisor.broker_mut().enable_policy_state(source(1), StateLimits::default()).unwrap();
    seed(&writer);
    port.submit(1, &proposal()).unwrap();
    supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
    let inputs = review(&mut supervisor, 1, 11);
    let permit = supervisor.authorize_request(1, Some(&inputs), &snapshot()).unwrap();
    writer.record(2, &change(7, Some(vec![9]), Some(vec![8]))).unwrap();
    writer.close(2, 2).unwrap();
    assert_eq!(supervisor.dispatch_request(1, DispatchKeys::single(&permit), Some(&inputs), &snapshot()).unwrap_err(), Error::Binding);
    let current = supervisor.broker().capture_policy_state().unwrap();
    assert!(supervisor.dispatch_request(1, DispatchKeys::single(&permit), Some(&inputs), current.snapshot()).is_err());
    assert_eq!(supervisor.broker().inspect().ledger.reserved, 16);
    assert_eq!(endpoint.execution_count(), 0);
    supervisor.broker_mut().cancel(1).unwrap();
    assert_eq!(supervisor.broker().inspect().ledger.available, 100);
}

#[test]
fn source_replacement_fences_old_attempts_and_requires_a_new_closed_generation() {
    let (port, mut supervisor, mut endpoint) = fixture(IntakeLimits::default());
    let writer = supervisor.broker_mut().enable_policy_state(source(1), StateLimits::default()).unwrap();
    seed(&writer);
    let old_ticket = port.submit(1, &proposal()).unwrap();
    supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
    let old_inputs = review(&mut supervisor, 1, 11);
    let old = supervisor.authorize_request(1, Some(&old_inputs), &snapshot()).unwrap();
    let (next, replacement) = supervisor.broker_mut().replace_policy_state(1, 0, source(2), StateLimits::default()).unwrap();
    assert_eq!(replacement.cancelled, vec![1]);
    assert_eq!(replacement.refunded_units, 16);
    assert_eq!(replacement.revocation_floor, 1);
    assert_eq!(supervisor.broker().inspect().ledger.available, 100);
    supervisor.synchronize().unwrap();
    assert!(matches!(port.poll(&old_ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    // Updating the retired publisher cannot populate the newly bound domain.
    writer.record(2, &StateEvent::Snapshot { semantic_epoch: 1, values: snapshot().values }).unwrap();
    writer.close(2, 2).unwrap();
    assert_eq!(supervisor.broker().capture_policy_state(), Err(Error::Incomplete));
    seed(&next);
    assert!(supervisor.dispatch_request(1, DispatchKeys::single(&old), Some(&old_inputs), &snapshot()).is_err());
    let mut fresh = proposal(); fresh.expected_policy_epoch = 1;
    port.submit(2, &fresh).unwrap();
    supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
    let inputs = review(&mut supervisor, 2, 12);
    let permit = supervisor.authorize_request(2, Some(&inputs), &snapshot()).unwrap();
    supervisor.deliver_request(2, DispatchKeys::single(&permit), Some(&inputs), &snapshot(), &mut endpoint).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(supervisor.broker().delivery_policy_state(2).unwrap().unwrap().frontier().source.generation, 2);
}

#[test]
fn missing_source_never_suppresses_reconciliation_or_rewrites_the_consumed_capture() {
    let (port, mut supervisor, mut endpoint) = fixture(IntakeLimits::default());
    let writer = supervisor.broker_mut().enable_policy_state(source(1), StateLimits::default()).unwrap();
    seed(&writer);
    let ticket = port.submit(1, &proposal()).unwrap();
    supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
    let inputs = review(&mut supervisor, 1, 11);
    let permit = supervisor.authorize_request(1, Some(&inputs), &snapshot()).unwrap();
    let message = supervisor.dispatch_request(1, DispatchKeys::single(&permit), Some(&inputs), &snapshot()).unwrap();
    let receipt = endpoint.deliver(&message).unwrap();
    supervisor.acknowledgment_lost(1).unwrap();
    let retained = supervisor.broker().delivery_policy_state(1).unwrap().unwrap().clone();
    drop(writer);
    supervisor.broker_mut().inputs_unavailable(1, 1).unwrap();
    assert_eq!(supervisor.broker().inspect().ledger.charged, 16);
    let results = supervisor.reconcile_pending(&mut endpoint).unwrap();
    assert_eq!(results[&1], Ok(EndpointStatus::Resolved(receipt)));
    assert_eq!(supervisor.broker().delivery_policy_state(1).unwrap(), Some(&retained));
    assert_eq!(supervisor.broker().inspect().ledger.stages[&1], ActionState::Confirmed);
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
}

#[test]
fn stale_or_foreign_source_replacement_is_atomic_and_bootstrap_cannot_be_reenabled() {
    let (port, mut supervisor, _) = fixture(IntakeLimits::default());
    let mut wrong = source(1); wrong.scope.branch = 99;
    assert_eq!(supervisor.broker_mut().enable_policy_state(wrong, StateLimits::default()).unwrap_err(), Error::Binding);
    assert!(supervisor.broker().policy_state_status().is_none());
    let writer = supervisor.broker_mut().enable_policy_state(source(1), StateLimits::default()).unwrap();
    seed(&writer);
    port.submit(1, &proposal()).unwrap();
    supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
    let inputs = review(&mut supervisor, 1, 11);
    let _permit = supervisor.authorize_request(1, Some(&inputs), &snapshot()).unwrap();
    let before = supervisor.broker().inspect();
    for (generation, epoch, next) in [(2, 0, source(2)), (1, 1, source(2)), (1, 0, wrong)] {
        assert!(supervisor.broker_mut().replace_policy_state(generation, epoch, next, StateLimits::default()).is_err());
        assert_eq!(supervisor.broker().inspect(), before);
        assert_eq!(supervisor.broker().capture_policy_state().unwrap().frontier().source, source(1));
    }
    drop(writer);
    assert_eq!(supervisor.broker_mut().enable_policy_state(source(2), StateLimits::default()).unwrap_err(), Error::Duplicate);
}

#[test]
fn an_already_completed_restrictive_review_can_still_hold_after_observation_loss() {
    let (port, mut supervisor, _) = fixture(IntakeLimits::default());
    let writer = supervisor.broker_mut().enable_policy_state(source(1), StateLimits::default()).unwrap();
    seed(&writer);
    port.submit(1, &proposal()).unwrap();
    supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
    let inputs = review(&mut supervisor, 1, 11);
    let mut round = supervisor.broker_mut().begin_review(1, 12, [7; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(10),
    }, &snapshot()).unwrap();
    let commitment = round.commitment("secret-helper", Verdict::Hold, b"salt").unwrap();
    round.commit("secret-helper", commitment, ElapsedTick(1)).unwrap();
    round.open_reveals(ElapsedTick(1)).unwrap();
    round.reveal("secret-helper", Verdict::Hold, b"salt", ElapsedTick(1)).unwrap();
    let completed = round.finish(ElapsedTick(1)).unwrap();
    writer.withdraw();
    supervisor.broker_mut().apply_review(completed, Some(&inputs), &snapshot()).unwrap();
    assert_eq!(supervisor.broker().inspect().decisions[&1], fa_reference::action::consequence::Consequence::HoldEffect);
    assert!(supervisor.authorize_request(1, Some(&inputs), &snapshot()).is_err());
}

#[test]
fn source_generation_repair_cannot_lower_the_observed_semantic_floor() {
    let (_, mut supervisor, _) = fixture(IntakeLimits::default());
    let writer = supervisor.broker_mut().enable_policy_state(source(1), StateLimits::default()).unwrap();
    writer.record(1, &StateEvent::Snapshot { semantic_epoch: 9, values: BTreeMap::new() }).unwrap();
    writer.close(1, 1).unwrap();
    drop(writer);
    let (writer, change) = supervisor.broker_mut().replace_policy_state(1, 0, source(2), StateLimits::default()).unwrap();
    assert_eq!(change.minimum_semantic_epoch, Some(9));
    seed(&writer);
    assert_eq!(supervisor.broker().capture_policy_state(), Err(Error::Stale));
    writer.record(2, &StateEvent::Snapshot { semantic_epoch: 9, values: BTreeMap::new() }).unwrap();
    writer.close(2, 2).unwrap();
    assert_eq!(supervisor.broker().capture_policy_state().unwrap().snapshot().semantic_epoch, 9);
}
