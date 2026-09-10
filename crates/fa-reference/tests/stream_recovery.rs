//! Streaming recovery must never confuse an unknown release with an unseen one.

#[path = "support/stream_fixture.rs"]
pub mod support;

use support::*;
use fa_reference::action::consequence::delivery::{EndpointOutcome, EndpointStatus, NonExecutionReason};
use fa_reference::action::consequence::gate::{ReviewBinding, TargetCeiling};
use fa_reference::action::consequence::gate::containment::ResetRequest;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fa_reference::Error;

#[test]
fn definitive_seal_refunds_only_the_unseen_unit_and_blocks_its_delayed_message() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let first = publish(&mut broker, &mut endpoint, &contracts, 1, Some("prefix"));
    let (action, inputs, permit) = ready(&mut broker, &contracts, 2, Some("never"));
    let delayed = broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    broker.acknowledgment_lost(2).unwrap();
    assert_eq!(broker.cancel(2), Err(Error::WrongState));
    let query = broker.status_query(2).unwrap();
    assert_eq!(endpoint.status(&query).unwrap(), EndpointStatus::AwaitingResolution);
    let receipt = endpoint.seal_unexecuted(&query).unwrap();
    assert_eq!(receipt.outcome(), EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed });
    broker.accept_receipt(receipt.clone()).unwrap();
    assert_eq!(broker.inspect().ledger.charged, first.request().units());
    assert_eq!(endpoint.payload(), b"prefix");
    assert!(!endpoint.stream_view().unwrap().finished());
    assert_eq!(broker.stream_pending(), None);

    let (action, inputs, permit) = ready(&mut broker, &contracts, 3, Some(" ok"));
    let replacement = broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    assert_eq!(endpoint.deliver(&delayed).unwrap(), receipt);
    assert_eq!(broker.accept_receipt(receipt), Ok(false));
    assert_eq!(broker.stream_pending(), Some(3));
    broker.accept_receipt(endpoint.deliver(&replacement).unwrap()).unwrap();
    assert_eq!(endpoint.payload(), b"prefix ok");
    assert_eq!(endpoint.execution_count(), 2);
    conserved(&broker);
}

#[test]
fn sealing_after_execution_reports_the_real_disclosure_instead_of_a_refund() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let (action, inputs, permit) = ready(&mut broker, &contracts, 1, Some("already visible"));
    let envelope = broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    let executed = endpoint.deliver(&envelope).unwrap();
    broker.acknowledgment_lost(1).unwrap();
    let sealed = endpoint.seal_unexecuted(&broker.status_query(1).unwrap()).unwrap();
    assert_eq!(sealed, executed);
    broker.accept_receipt(sealed).unwrap();
    assert_eq!(broker.inspect().ledger.charged, action.spec().units);
    assert_eq!(broker.stream_state().unwrap().1.visible(), b"already visible");
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Confirmed);
    conserved(&broker);
}

#[test]
fn dispatcher_recovery_preserves_both_orders_of_delivery_and_fence_installation() {
    for delivered_before_fence in [false, true] {
        let (mut broker, mut endpoint, contracts) = fixture();
        let (action, inputs, permit) = ready(&mut broker, &contracts, 1, Some("first"));
        let envelope = broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
        let fence = broker.restart_dispatcher().unwrap();
        assert_eq!(broker.stream_pending(), Some(1));
        if delivered_before_fence { endpoint.deliver(&envelope).unwrap(); }
        let acknowledgment = endpoint.install_fence(fence).unwrap();
        broker.confirm_fence(acknowledgment).unwrap();
        assert_eq!(endpoint.deliver(&envelope).unwrap_err(), Error::Stale);
        let query = broker.status_query(1).unwrap();
        let status = endpoint.status(&query).unwrap();
        broker.reconcile_status(&query, status).unwrap();
        if !delivered_before_fence {
            assert_eq!(broker.stream_pending(), Some(1));
            broker.accept_receipt(endpoint.seal_unexecuted(&query).unwrap()).unwrap();
        }
        assert_eq!(broker.stream_pending(), None);
        assert_eq!(broker.stream_state().unwrap().1.message_count(), if delivered_before_fence { 1 } else { 0 });
        publish(&mut broker, &mut endpoint, &contracts, 2, Some("next"));
        assert_eq!(endpoint.payload(), if delivered_before_fence { &b"firstnext"[..] } else { &b"next"[..] });
        conserved(&broker);
    }
}

#[test]
fn actor_reset_preserves_confirmed_prefix_and_the_pending_disclosure_obligation() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let checkpoint = broker.capture_checkpoint(1, 0).unwrap();
    publish(&mut broker, &mut endpoint, &contracts, 1, Some("public"));
    let (action, inputs, permit) = ready(&mut broker, &contracts, 2, Some(" queued"));
    let delayed = broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    let before = broker.inspect();
    broker.reset(ResetRequest {
        checkpoint, expected_control_sequence: before.sequence,
        expected_actor_revision: broker.actor_revision(),
        binding: ReviewBinding { round: 90, evidence_root: [9; 32], reducer_generation: 1 },
        retained_targets: TargetCeiling::new(&[target(2), target(3), target(4)]).unwrap(),
    }).unwrap();
    assert_eq!(broker.inspect().ledger.charged, before.ledger.charged);
    assert_eq!(broker.stream_state().unwrap().1.visible(), b"public");
    assert_eq!(broker.stream_pending(), Some(2));
    assert_eq!(broker.stream_message_spec("next", ElapsedTick(100)), Err(Error::Incomplete));
    broker.accept_receipt(endpoint.deliver(&delayed).unwrap()).unwrap();
    assert_eq!(broker.stream_state().unwrap().1.visible(), b"public queued");
    let next = publish(&mut broker, &mut endpoint, &contracts, 3, Some(" next"));
    assert_eq!(next.request().policy_epoch(), 1);
    assert_eq!(endpoint.payload(), b"public queued next");
    assert_eq!(broker.incident_count(), 1);
    conserved(&broker);
}

#[test]
fn policy_rotation_refunds_unsent_work_but_keeps_the_confirmed_stream_context() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let first = publish(&mut broker, &mut endpoint, &contracts, 1, Some("public"));
    let (old_action, old_input, old_permit) = ready(&mut broker, &contracts, 2, Some("old policy"));
    let inspection = broker.inspect();
    broker.replace_policy(inspection.sequence, inspection.ledger.epoch, policy(2)).unwrap();
    assert_eq!(broker.inspect().ledger.reserved, 0);
    assert_eq!(broker.inspect().ledger.charged, first.request().units());
    assert_eq!(broker.stream_state().unwrap().1.visible(), b"public");
    assert!(broker.dispatch(&old_permit, &old_action, Some(&old_input), &snapshot()).is_err());
    publish(&mut broker, &mut endpoint, &contracts, 3, Some(" new policy"));
    assert_eq!(endpoint.payload(), b"public new policy");
    conserved(&broker);
}

#[test]
fn retention_expiry_and_irrecoverable_unknown_never_advance_or_close_the_stream() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let (action, inputs, permit) = ready(&mut broker, &contracts, 1, Some("uncertain"));
    broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    endpoint.observe_time(ElapsedTick(201)).unwrap();
    broker.observe_time(ElapsedTick(201)).unwrap();
    let query = broker.status_query(1).unwrap();
    let status = endpoint.status(&query).unwrap();
    assert_eq!(status, EndpointStatus::RetentionExpired);
    broker.reconcile_status(&query, status).unwrap();
    assert_eq!(broker.inspect().ledger.charged, action.spec().units);
    assert_eq!(broker.stream_pending(), Some(1));
    assert_eq!(broker.stream_finish_spec(ElapsedTick(300)), Err(Error::Incomplete));
    broker.abandon_unknown(1).unwrap();
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::IrrecoverablyUnknown);
    assert_eq!(broker.stream_pending(), Some(1));
    assert!(!broker.stream_state().unwrap().1.finished());
    assert_eq!(broker.stream_message_spec("retry", ElapsedTick(300)), Err(Error::Incomplete));
    conserved(&broker);
}

#[test]
fn previously_retained_terminal_evidence_can_resolve_after_the_lookup_retention_window() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let (action, inputs, permit) = ready(&mut broker, &contracts, 1, Some("known"));
    let envelope = broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    let retained = endpoint.deliver(&envelope).unwrap();
    endpoint.observe_time(ElapsedTick(201)).unwrap();
    broker.observe_time(ElapsedTick(201)).unwrap();
    let query = broker.status_query(1).unwrap();
    broker.reconcile_status(&query, endpoint.status(&query).unwrap()).unwrap();
    assert_eq!(broker.stream_pending(), Some(1));
    broker.accept_receipt(retained).unwrap();
    assert_eq!(broker.stream_pending(), None);
    assert_eq!(broker.stream_state().unwrap().1.visible(), b"known");
    let finish = broker.stream_finish_spec(ElapsedTick(300)).unwrap();
    let proposal = broker.propose(2, finish, &snapshot()).unwrap();
    let inputs = input(&proposal.action, &contracts);
    broker.record_inputs(2, 0, inputs.clone()).unwrap();
    review(&mut broker, 2, 2, &inputs, Verdict::Allow);
    let permit = broker.authorize(2, Some(&inputs), &snapshot()).unwrap();
    let finish = broker.dispatch(&permit, &proposal.action, Some(&inputs), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&finish).unwrap()).unwrap();
    assert!(endpoint.stream_view().unwrap().finished());
    assert_eq!(endpoint.payload(), b"known");
    conserved(&broker);
}

#[test]
fn input_outage_blocks_new_disclosure_without_blocking_receipt_reconciliation() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let (action, inputs, permit) = ready(&mut broker, &contracts, 1, Some("first"));
    let envelope = broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    let receipt = endpoint.deliver(&envelope).unwrap();
    broker.acknowledgment_lost(1).unwrap();
    broker.inputs_unavailable(1, broker.input_revision(1).unwrap()).unwrap();
    broker.accept_receipt(receipt).unwrap();
    assert_eq!(broker.stream_state().unwrap().1.visible(), b"first");
    let (next, inputs, permit) = ready(&mut broker, &contracts, 2, Some("second"));
    let before = broker.inspect();
    assert_eq!(broker.dispatch(&permit, &next, None, &snapshot()).unwrap_err(), Error::Incomplete);
    assert_eq!(broker.inspect(), before);
    assert_eq!(endpoint.payload(), b"first");
    let envelope = broker.dispatch(&permit, &next, Some(&inputs), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&envelope).unwrap()).unwrap();
    assert_eq!(endpoint.payload(), b"firstsecond");
    conserved(&broker);
}

#[test]
fn lost_finish_acknowledgment_is_reconciled_and_reset_cannot_reopen_the_audience_stream() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let checkpoint = broker.capture_checkpoint(1, 0).unwrap();
    publish(&mut broker, &mut endpoint, &contracts, 1, Some("final text"));
    let (action, inputs, permit) = ready(&mut broker, &contracts, 2, None);
    let envelope = broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    endpoint.deliver(&envelope).unwrap();
    assert!(endpoint.stream_view().unwrap().finished());
    assert!(!broker.stream_state().unwrap().1.finished());
    let fence = broker.restart_dispatcher().unwrap();
    broker.confirm_fence(endpoint.install_fence(fence).unwrap()).unwrap();
    let query = broker.status_query(2).unwrap();
    broker.reconcile_status(&query, endpoint.status(&query).unwrap()).unwrap();
    assert!(broker.stream_state().unwrap().1.finished());
    let inspection = broker.inspect();
    broker.reset(ResetRequest {
        checkpoint, expected_control_sequence: inspection.sequence,
        expected_actor_revision: broker.actor_revision(),
        binding: ReviewBinding { round: 91, evidence_root: [9; 32], reducer_generation: 1 },
        retained_targets: TargetCeiling::new(&[target(3)]).unwrap(),
    }).unwrap();
    assert_eq!(broker.stream_message_spec("resurrect", ElapsedTick(100)), Err(Error::WrongState));
    assert_eq!(endpoint.payload(), b"final text");
    assert!(endpoint.stream_view().unwrap().finished());
    conserved(&broker);
}
