//! Fleet enforcement composed with cumulative disclosure and optional human keys.

#[path = "support/stream_fixture.rs"]
mod support;
use support::*;

use fa_reference::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
use fa_reference::action::consequence::activation::monitor::{RefinementBudget, RefinementMonitor};
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::delivery::fleet::{FleetCoordinator, FleetDeliveryKnowledge, FleetScope, FencePropagation};
use fa_reference::action::consequence::gate::{ReviewBinding, TargetCeiling};
use fa_reference::action::consequence::gate::containment::ResetRequest;
use fa_reference::action::consequence::oversight::human::{HumanDisposition, HumanReviewPolicy};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fa_reference::Error;

#[test]
fn preissued_human_key_cannot_override_a_fleet_stop_but_can_be_withdrawn() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let reviewer = broker.enable_human_review(HumanReviewPolicy {
        reviewer_id: 77, max_validity_ticks: 20, max_requests: 4,
    }).unwrap();
    let mut fleet = FleetCoordinator::new(1, 2, 100).unwrap();
    broker.join_fleet(&mut fleet, 1, 0, ElapsedTick(50)).unwrap();
    let (action, inputs, permit) = ready(&mut broker, &contracts, 1, Some("first"));
    let request = broker.request_human_approval(1, 1, Some(&inputs), ElapsedTick(20)).unwrap();
    let human = reviewer.approve(&request, ElapsedTick(1)).unwrap();
    let message = broker.dispatch_with_human(&permit, &human, &action, Some(&inputs), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    let (second, input2, permit2) = ready(&mut broker, &contracts, 2, Some("second"));
    let request2 = broker.request_human_approval(2, 2, Some(&input2), ElapsedTick(20)).unwrap();
    let key2 = reviewer.approve(&request2, ElapsedTick(1)).unwrap();
    let command = fleet.issue_fence(10, fleet.revision(), FleetScope::All).unwrap();
    let before = broker.inspect();
    let ack = broker.install_fleet_fence(&command, before.sequence, before.ledger.epoch).unwrap();
    assert_eq!(ack.transition().refunded_units, second.spec().units);
    assert_eq!(broker.dispatch_with_human(&permit2, &key2, &second, Some(&input2), &snapshot()).unwrap_err(), Error::WrongState);
    assert_eq!(broker.human_status(2).unwrap().disposition, HumanDisposition::Approved);
    assert_eq!(reviewer.revoke_all(ElapsedTick(1)).unwrap().requests, vec![2]);
    assert_eq!(broker.human_status(2).unwrap().disposition, HumanDisposition::Revoked);
    assert_eq!(endpoint.payload(), b"first"); assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(broker.inspect().ledger.stages[&2], ActionState::Cancelled);
    fleet.acknowledge(ack).unwrap(); conserved(&broker);
}

#[test]
fn pending_disclosure_survives_fence_and_monitor_outage_until_its_real_receipt() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let mut fleet = FleetCoordinator::new(1, 2, 100).unwrap();
    broker.join_fleet(&mut fleet, 1, 0, ElapsedTick(50)).unwrap();
    publish(&mut broker, &mut endpoint, &contracts, 1, Some("A"));
    let (action, inputs, permit) = ready(&mut broker, &contracts, 2, Some("B"));
    let command = fleet.issue_fence(10, fleet.revision(), FleetScope::All).unwrap();
    let message = broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    let terminal = endpoint.deliver(&message).unwrap();
    broker.acknowledgment_lost(2).unwrap();
    let revision = broker.input_revision(2).unwrap(); broker.inputs_unavailable(2, revision).unwrap();
    let before = broker.inspect();
    let ack = broker.install_fleet_fence(&command, before.sequence, before.ledger.epoch).unwrap();
    assert_eq!(ack.transition().refunded_units, 0); assert_eq!(broker.stream_pending(), Some(2));
    assert_eq!(broker.stream_state().unwrap().0.expected_version, 2);
    assert_eq!(endpoint.payload(), b"AB"); assert_eq!(broker.cancel(2), Err(Error::WrongState));
    fleet.acknowledge(ack).unwrap(); fleet.observe_domain(broker.fleet_observation().unwrap()).unwrap();
    match &fleet.report(10).unwrap().domains[&1].deliveries {
        FleetDeliveryKnowledge::Observed { post_issue, unresolved, post_issue_admissions_complete, .. } => {
            assert!(*post_issue_admissions_complete); assert_eq!(post_issue.len(), 1);
            assert_eq!(unresolved.len(), 1); assert_eq!(unresolved[0].state, ActionState::Unknown);
        }
        _ => panic!("observed admission stop"),
    }
    assert_eq!(broker.accept_receipt(terminal), Ok(true));
    assert_eq!(broker.stream_pending(), None); assert_eq!(broker.stream_state().unwrap().0.expected_version, 3);
    assert_eq!(broker.stream_finish_spec(ElapsedTick(100)), Err(Error::WrongState));
    assert_eq!(endpoint.payload(), b"AB"); conserved(&broker);
}

#[test]
fn lease_expiry_blocks_both_keys_without_spending_them_or_refunding_reservations() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let reviewer = broker.enable_human_review(HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 30, max_requests: 2 }).unwrap();
    let mut fleet = FleetCoordinator::new(1, 1, 100).unwrap();
    broker.join_fleet(&mut fleet, 1, 0, ElapsedTick(10)).unwrap();
    let (action, inputs, permit) = ready(&mut broker, &contracts, 1, Some("not yet"));
    let request = broker.request_human_approval(1, 1, Some(&inputs), ElapsedTick(20)).unwrap();
    let human = reviewer.approve(&request, ElapsedTick(1)).unwrap();
    fleet.issue_fence(10, fleet.revision(), FleetScope::All).unwrap();
    fleet.observe_time(ElapsedTick(10)).unwrap();
    let before = broker.inspect();
    assert_eq!(broker.dispatch_with_human(&permit, &human, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::Stale);
    assert_eq!(broker.inspect(), before); assert_eq!(broker.human_status(1).unwrap().disposition, HumanDisposition::Approved);
    broker.observe_time(ElapsedTick(10)).unwrap();
    assert_eq!(broker.dispatch_with_human(&permit, &human, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::Stale);
    let dispatcher_fence = broker.restart_dispatcher().unwrap();
    broker.confirm_fence(endpoint.install_fence(dispatcher_fence).unwrap()).unwrap();
    assert_eq!(broker.dispatch_with_human(&permit, &human, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::Stale);
    assert_eq!(broker.inspect().ledger.reserved, action.spec().units);
    broker.cancel(1).unwrap(); assert_eq!(broker.inspect().ledger.available, TOTAL);
    assert_eq!(endpoint.execution_count(), 0); conserved(&broker);
}

#[test]
fn reset_and_policy_replacement_cannot_reopen_the_stopped_authority() {
    let (mut broker, _, contracts) = fixture(); let mut fleet = FleetCoordinator::new(1, 2, 100).unwrap();
    broker.join_fleet(&mut fleet, 1, 0, ElapsedTick(50)).unwrap();
    let checkpoint = broker.capture_checkpoint(1, 0).unwrap();
    let (action, inputs, permit) = ready(&mut broker, &contracts, 1, Some("held"));
    let command = fleet.issue_fence(10, fleet.revision(), FleetScope::All).unwrap();
    let before = broker.inspect(); broker.install_fleet_fence(&command, before.sequence, before.ledger.epoch).unwrap();
    let stopped = broker.inspect();
    assert_eq!(broker.reset(ResetRequest {
        checkpoint, expected_control_sequence: stopped.sequence, expected_actor_revision: broker.actor_revision(),
        binding: ReviewBinding { round: 90, evidence_root: [9; 32], reducer_generation: 1 },
        retained_targets: TargetCeiling::new(&[target(1)]).unwrap(),
    }).unwrap_err(), Error::WrongState);
    assert_eq!(broker.inspect(), stopped);
    broker.replace_policy(stopped.sequence, stopped.ledger.epoch, policy(2)).unwrap();
    assert!(broker.inspect().suspended); assert_eq!(broker.fleet_floor(), Some(command.floor()));
    assert!(broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).is_err());
    assert_eq!(broker.stream_message_spec("new", ElapsedTick(100)), Err(Error::WrongState));
    conserved(&broker);
}

#[test]
fn activation_outage_cannot_block_fencing_or_settlement_of_prior_disclosure() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let profile = CaptureProfile { tenant: 1, model: 1, model_generation: 1, tap: 1, layout_generation: 1 };
    let probe = LinearProbe::new(1, 1, profile, &[-1.0], 0.0, 0.0).unwrap();
    let monitor = RefinementMonitor::new(vec![probe], vec![0, 23], RefinementBudget { encoded_bytes: 4096, probe_coordinates: 64 }).unwrap();
    broker.enable_activation_tripwire(monitor, 1, 4).unwrap();
    let mut fleet = FleetCoordinator::new(1, 2, 100).unwrap(); broker.join_fleet(&mut fleet, 1, 0, ElapsedTick(50)).unwrap();
    let (action, inputs) = prepare(&mut broker, &contracts, 1, Some("visible"));
    let source = SourceFrame::capture(FrameIdentity { profile, stream: 1, sequence: 1, position: 0 }, &[1.0]).unwrap();
    broker.record_activation(1, broker.input_revision(1).unwrap(), broker.actor_revision(), &source).unwrap();
    review(&mut broker, 1, 1, &inputs, Verdict::Allow);
    let permit = broker.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let message = broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    let terminal = endpoint.deliver(&message).unwrap(); broker.acknowledgment_lost(1).unwrap();
    broker.activation_unavailable(1, broker.input_revision(1).unwrap()).unwrap();
    let command = fleet.issue_fence(10, fleet.revision(), FleetScope::All).unwrap();
    let before = broker.inspect(); let ack = broker.install_fleet_fence(&command, before.sequence, before.ledger.epoch).unwrap();
    assert_eq!(ack.transition().refunded_units, 0); assert_eq!(broker.inspect().ledger.charged, action.spec().units);
    broker.accept_receipt(terminal).unwrap(); fleet.acknowledge(ack).unwrap();
    fleet.observe_domain(broker.fleet_observation().unwrap()).unwrap();
    match &fleet.report(10).unwrap().domains[&1].deliveries {
        FleetDeliveryKnowledge::Observed { post_issue, unresolved, .. } => { assert!(post_issue.is_empty()); assert!(unresolved.is_empty()); }
        _ => panic!("observed settlement"),
    }
    assert_eq!(endpoint.payload(), b"visible"); conserved(&broker);
}

#[test]
fn full_command_capacity_does_not_block_acknowledgment_or_old_obligations() {
    let (mut broker, mut endpoint, contracts) = fixture(); let mut fleet = FleetCoordinator::new(1, 1, 100).unwrap();
    broker.join_fleet(&mut fleet, 1, 0, ElapsedTick(50)).unwrap();
    let (action, inputs, permit) = ready(&mut broker, &contracts, 1, Some("maybe"));
    let message = broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    let command = fleet.issue_fence(10, fleet.revision(), FleetScope::All).unwrap();
    assert_eq!(fleet.issue_fence(11, fleet.revision(), FleetScope::All).unwrap_err(), Error::Limit);
    let before = broker.inspect(); let ack = broker.install_fleet_fence(&command, before.sequence, before.ledger.epoch).unwrap();
    fleet.acknowledge(ack).unwrap();
    let query = broker.status_query(1).unwrap(); broker.accept_receipt(endpoint.seal_unexecuted(&query).unwrap()).unwrap();
    assert!(matches!(endpoint.deliver(&message).unwrap().outcome(), fa_reference::action::consequence::delivery::EndpointOutcome::NotExecuted { .. }));
    fleet.observe_domain(broker.fleet_observation().unwrap()).unwrap();
    assert!(matches!(fleet.report(10).unwrap().domains[&1].propagation, FencePropagation::Acknowledged(_)));
    assert_eq!(broker.inspect().ledger.available, TOTAL); assert_eq!(endpoint.execution_count(), 0); conserved(&broker);
}

#[test]
fn dropping_the_coordinator_does_not_drop_the_enforced_lease() {
    let (mut broker, mut endpoint, contracts) = fixture(); let mut fleet = FleetCoordinator::new(1, 1, 100).unwrap();
    broker.join_fleet(&mut fleet, 1, 0, ElapsedTick(10)).unwrap();
    let (action, inputs, permit) = ready(&mut broker, &contracts, 1, Some("pending"));
    drop(fleet);
    broker.observe_time(ElapsedTick(10)).unwrap();
    assert_eq!(broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::Stale);
    let fence = broker.restart_dispatcher().unwrap(); broker.confirm_fence(endpoint.install_fence(fence).unwrap()).unwrap();
    assert_eq!(broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::Stale);
    let mut replacement = FleetCoordinator::new(1, 1, 100).unwrap();
    assert!(broker.join_fleet(&mut replacement, 1, 0, ElapsedTick(50)).is_err());
    broker.cancel(1).unwrap(); assert_eq!(broker.inspect().ledger.available, TOTAL); conserved(&broker);
}
