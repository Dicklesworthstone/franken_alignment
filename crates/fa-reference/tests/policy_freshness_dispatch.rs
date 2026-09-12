//! Source expiry traverses the original actor/congress/permit and recovery path.
#[path = "support/fresh_policy.rs"]
mod support;
use support::{Ready, publish, snapshot, source};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::PublicationEndpoint;
use fa_reference::action::consequence::oversight::{ReviewWindow, human::HumanDisposition};
use fa_reference::action::consequence::oversight::policy_state::{StateEvent, StateFreshness, StateLimits};
use fa_reference::round::Verdict;
use fa_reference::Error;

#[test]
fn expired_capture_preserves_both_keys_and_a_new_observation_reuses_the_original_permit() {
    for two_key in [false, true] {
        let mut ready = Ready::new(two_key);
        ready.supervisor.broker_mut().observe_time(ElapsedTick(5)).unwrap();
        let before = ready.supervisor.broker().inspect();
        assert_eq!(ready.dispatch().unwrap_err(), Error::Stale);
        assert_eq!(ready.supervisor.broker().inspect(), before);
        assert_eq!(before.ledger.reserved, 16);
        assert_eq!(ready.endpoint.execution_count(), 0);
        if two_key {
            assert_eq!(ready.supervisor.broker().human_status(88).unwrap().disposition, HumanDisposition::Approved);
        }
        publish(ready.writer.as_ref().unwrap(), 2, 5);
        let message = ready.dispatch().unwrap();
        ready.endpoint.observe_time(ElapsedTick(5)).unwrap();
        ready.supervisor.accept_receipt(ready.endpoint.deliver(&message).unwrap()).unwrap();
        assert_eq!(ready.endpoint.execution_count(), 1);
        assert_eq!(ready.supervisor.broker().inspect().ledger.charged, 16);
        let cut = ready.supervisor.broker().delivery_policy_state(1).unwrap().unwrap();
        assert_eq!(cut.frontier().through, 2);
        assert_eq!(cut.lease().unwrap().expires_at(), ElapsedTick(9));
    }
}

#[test]
fn expired_source_cannot_create_or_apply_a_permitting_review_but_a_hold_survives() {
    for verdict in [Verdict::Allow, Verdict::Hold] {
        let mut ready = Ready::new(false);
        let mut session = ready.supervisor.broker_mut().begin_review(1, 12, [3; 32], ReviewWindow {
            commit_by: ElapsedTick(2), reveal_by: ElapsedTick(4),
        }, &snapshot()).unwrap();
        let commitment = session.commitment("secret-helper", verdict, b"salt").unwrap();
        session.commit("secret-helper", commitment, ElapsedTick(1)).unwrap();
        session.open_reveals(ElapsedTick(1)).unwrap();
        session.reveal("secret-helper", verdict, b"salt", ElapsedTick(1)).unwrap();
        let review = session.finish(ElapsedTick(1)).unwrap();
        ready.supervisor.broker_mut().observe_time(ElapsedTick(5)).unwrap();
        let before = ready.supervisor.broker().inspect();
        let result = ready.supervisor.broker_mut().apply_review(review, Some(&ready.inputs), &snapshot());
        if verdict == Verdict::Allow {
            assert_eq!(result.unwrap_err(), Error::Stale);
            assert_eq!(ready.supervisor.broker().inspect(), before);
        } else { assert!(result.is_ok()); }
        assert_eq!(ready.supervisor.broker_mut().begin_review(1, 13, [3; 32], ReviewWindow {
            commit_by: ElapsedTick(6), reveal_by: ElapsedTick(8),
        }, &snapshot()).unwrap_err(), Error::Stale);
        let mut spec = ready.supervisor.action(42).unwrap().spec().clone();
        spec.payload = b"new action".to_vec();
        assert_eq!(ready.supervisor.broker_mut().propose(2, spec, &snapshot()).unwrap_err(), Error::Stale);
    }
}

#[test]
fn replacement_preserves_age_and_clock_floors_and_cancels_only_undispatched_rights() {
    let mut ready = Ready::new(true);
    ready.supervisor.broker_mut().observe_time(ElapsedTick(5)).unwrap();
    assert_eq!(ready.supervisor.broker().capture_policy_state(), Err(Error::Stale));
    let mut next = source(); next.generation = 2;
    let (writer, change) = ready.supervisor.broker_mut().replace_policy_state(1, 0, next, StateLimits::default()).unwrap();
    assert_eq!(change.cancelled, vec![1]);
    assert_eq!(change.refunded_units, 16);
    assert_eq!(ready.supervisor.broker().inspect().ledger.available, 100);
    assert_eq!(ready.supervisor.broker().policy_state_freshness(), Some(StateFreshness::new(4).unwrap()));
    assert_eq!(ready.supervisor.broker_mut().enable_policy_state(next, StateLimits::default()).unwrap_err(), Error::Duplicate);
    writer.record(1, &StateEvent::Snapshot { semantic_epoch: 1, values: snapshot().values }).unwrap();
    assert_eq!(writer.close(1, 1), Err(Error::Incomplete));
    assert_eq!(writer.close_observed(1, 1, ElapsedTick(0)), Err(Error::Stale));
    writer.close_observed(1, 1, ElapsedTick(1)).unwrap();
    assert_eq!(ready.supervisor.broker().capture_policy_state(), Err(Error::Stale));
    publish(&writer, 2, 5);
    assert!(ready.supervisor.broker().capture_policy_state().is_ok());
    publish(ready.writer.as_ref().unwrap(), 2, 6);
    assert_eq!(ready.supervisor.broker().capture_policy_state().unwrap().frontier().source, next);
    assert!(ready.dispatch().is_err());
    assert_eq!(ready.supervisor.broker().inspect().ledger.charged, 0);
}

#[test]
fn source_expiry_and_loss_do_not_block_a_real_late_execution_receipt() {
    let endpoint = PublicationEndpoint::new(support::proposal().target, b"old".to_vec(), 200, 128).unwrap();
    let mut ready = Ready::with_endpoint(endpoint, true);
    let message = ready.dispatch().unwrap();
    let receipt = ready.endpoint.deliver(&message).unwrap();
    ready.supervisor.acknowledgment_lost(42).unwrap();
    let original = ready.supervisor.broker().delivery_policy_state(1).unwrap().unwrap().clone();
    ready.supervisor.broker_mut().observe_time(ElapsedTick(6)).unwrap();
    assert_eq!(ready.supervisor.broker().capture_policy_state(), Err(Error::Stale));
    drop(ready.writer.take());
    ready.supervisor.broker_mut().inputs_unavailable(1, 1).unwrap();
    assert!(ready.supervisor.accept_receipt(receipt.clone()).unwrap());
    assert!(!ready.supervisor.accept_receipt(receipt).unwrap());
    assert_eq!(ready.supervisor.broker().inspect().ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(ready.supervisor.broker().inspect().ledger.charged, 16);
    assert_eq!(ready.supervisor.broker().delivery_policy_state(1).unwrap().unwrap(), &original);
    assert_eq!(ready.endpoint.execution_count(), 1);
}
