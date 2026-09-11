//! Checked CPU captures through real reference congress, keys and disclosure.
//! Host bytes and helper verdicts are fixtures, not a deployed inference host.

#[path = "support/stream_fixture.rs"]
#[allow(dead_code)]
mod support;
use support::*;
use fa_reference::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
use fa_reference::action::consequence::activation::monitor::{MonitorOutcome, RefinementBudget, RefinementMonitor};
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::{BufferIdentity, ByteOrder, HostTensor, ScalarEncoding, TensorContract, TensorLayout, TokenSelection};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::oversight::OversightBroker;
use fa_reference::action::consequence::oversight::human::{HumanDisposition, HumanReviewPolicy};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fa_reference::Error;

fn profile() -> CaptureProfile {
    CaptureProfile { tenant: 1, model: 2, model_generation: 1, tap: 4, layout_generation: 5 }
}
fn enable(broker: &mut OversightBroker, captures: usize) {
    let probe = LinearProbe::new(1, 1, profile(), &[1.0, 0.0], 0.0, 0.0).unwrap();
    let monitor = RefinementMonitor::new(vec![probe], vec![0, 23], RefinementBudget {
        encoded_bytes: 1024, probe_coordinates: 4,
    }).unwrap();
    let contract = TensorContract::new(profile(), ScalarEncoding::BFloat16, ByteOrder::Little, 1, 2).unwrap();
    broker.enable_tensor_activation_tripwire(monitor, contract, 6, 1, captures).unwrap();
}
fn layout() -> TensorLayout {
    TensorLayout::new([2, 1, 1, 2], [8, 0, 0, 2], 0, ScalarEncoding::BFloat16, ByteOrder::Little).unwrap()
}
fn bytes(alarm: bool) -> Vec<u8> {
    let mut bytes = vec![0; 12];
    bytes[..2].copy_from_slice(&0x3f80_u16.to_le_bytes());
    bytes[8..10].copy_from_slice(&(if alarm { 0x3f80_u16 } else { 0xbf80_u16 }).to_le_bytes());
    bytes
}
fn selected(sequence: u64) -> TokenSelection {
    TokenSelection { batch: 1, token: 0, first_position: 0, stream: 6, sequence }
}
fn host<'a>(layout: &'a TensorLayout, bytes: &'a [u8], generation: u64) -> HostTensor<'a> {
    HostTensor { identity: BufferIdentity { object: 7, generation }, layout, bytes }
}
fn capture(broker: &mut OversightBroker, id: u64, sequence: u64, alarm: bool) {
    let report = broker.record_tensor_activation(id, broker.input_revision(id).unwrap(), broker.actor_revision(),
        host(&layout(), &bytes(alarm), 1), selected(sequence)).unwrap();
    assert_eq!(report.outcome(), if alarm { MonitorOutcome::Alarm } else { MonitorOutcome::NoAlarm });
}

#[test]
fn strided_bfloat_capture_congress_and_both_keys_publish_without_flattened_bypass() {
    let (mut broker, mut endpoint, contracts) = fixture();
    enable(&mut broker, 8);
    let reviewer = broker.enable_human_review(HumanReviewPolicy { reviewer_id: 9, max_validity_ticks: 20, max_requests: 8 }).unwrap();
    let (action, inputs) = prepare(&mut broker, &contracts, 1, Some("checked tensor"));
    let flat = SourceFrame::capture(FrameIdentity { profile: profile(), stream: 6, sequence: 1, position: 0 }, &[-1.0, 0.0]).unwrap();
    assert_eq!(broker.record_activation(1, 1, 0, &flat).unwrap_err(), Error::Binding);
    assert_eq!(broker.input_revision(1).unwrap(), 1);
    capture(&mut broker, 1, 1, false);
    let receipt = broker.tensor_capture_receipt(1).unwrap().unwrap();
    assert_eq!(receipt.selection().batch, 1);
    assert_eq!(receipt.source_bytes_read(), 4);
    assert_eq!(receipt.normalized_bytes(), 8);
    review(&mut broker, 1, 11, &inputs, Verdict::Allow);
    let automatic = broker.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let request = broker.request_human_approval(1, 1, Some(&inputs), ElapsedTick(10)).unwrap();
    let human = reviewer.approve(&request, ElapsedTick(1)).unwrap();
    let message = broker.dispatch_with_human(&automatic, &human, &action, Some(&inputs), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.payload(), b"checked tensor");
    assert_eq!(broker.human_status(1).unwrap().disposition, HumanDisposition::Consumed);
    conserved(&broker);
}

#[test]
fn short_recapture_removes_quiet_but_fresh_review_can_reuse_the_original_reservation() {
    let (mut broker, mut endpoint, contracts) = fixture();
    enable(&mut broker, 8);
    let (action, inputs) = prepare(&mut broker, &contracts, 1, Some("recovered"));
    capture(&mut broker, 1, 1, false);
    review(&mut broker, 1, 11, &inputs, Verdict::Allow);
    let permit = broker.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let reserved = broker.inspect().ledger.reserved;
    let backing = bytes(false);
    assert_eq!(broker.record_tensor_activation(1, 2, 0, host(&layout(), &backing[..11], 1), selected(2)).unwrap_err(), Error::Incomplete);
    assert_eq!(broker.input_revision(1).unwrap(), 3);
    assert!(broker.activation_report(1).unwrap().is_none());
    assert!(broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).is_err());
    assert_eq!(broker.inspect().ledger.reserved, reserved);
    capture(&mut broker, 1, 3, false);
    assert!(broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).is_err());
    review(&mut broker, 1, 12, &inputs, Verdict::Allow);
    let message = broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
    conserved(&broker);
}

#[test]
fn wrong_batch_refuses_before_admission_but_changed_shape_is_a_capture_failure() {
    let (mut broker, _, contracts) = fixture();
    enable(&mut broker, 8);
    prepare(&mut broker, &contracts, 1, Some("held"));
    capture(&mut broker, 1, 1, false);
    let wrong = TokenSelection { batch: 0, ..selected(2) };
    assert_eq!(broker.record_tensor_activation(1, 2, 0, host(&layout(), &bytes(false), 1), wrong).unwrap_err(), Error::Binding);
    assert_eq!(broker.input_revision(1).unwrap(), 2);
    let changed = TensorLayout::new([2, 1, 2, 1], [8, 0, 2, 0], 0, ScalarEncoding::BFloat16, ByteOrder::Little).unwrap();
    // Same flattened dimension, different head/channel contract.
    assert_eq!(broker.record_tensor_activation(1, 2, 0, host(&changed, &bytes(false), 1), selected(2)).unwrap_err(), Error::Binding);
    assert_eq!(broker.input_revision(1).unwrap(), 3);
    assert!(broker.activation_report(1).unwrap().is_none());
    assert_eq!(broker.record_tensor_activation(1, 3, 0, host(&layout(), &bytes(false), 1), selected(2)).unwrap_err(), Error::Stale);
    capture(&mut broker, 1, 3, false);
}

#[test]
fn tensor_alarm_invalidates_preissued_human_and_automatic_keys() {
    let (mut broker, _, contracts) = fixture();
    enable(&mut broker, 8);
    let reviewer = broker.enable_human_review(HumanReviewPolicy { reviewer_id: 9, max_validity_ticks: 20, max_requests: 8 }).unwrap();
    let (action, inputs) = prepare(&mut broker, &contracts, 1, Some("not released"));
    capture(&mut broker, 1, 1, false);
    review(&mut broker, 1, 11, &inputs, Verdict::Allow);
    let automatic = broker.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let request = broker.request_human_approval(1, 1, Some(&inputs), ElapsedTick(10)).unwrap();
    let human = reviewer.approve(&request, ElapsedTick(1)).unwrap();
    capture(&mut broker, 1, 2, true);
    assert!(broker.dispatch_with_human(&automatic, &human, &action, Some(&inputs), &snapshot()).is_err());
    assert_eq!(broker.human_status(1).unwrap().disposition, HumanDisposition::Approved);
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Authorized);
    capture(&mut broker, 1, 3, false);
    review(&mut broker, 1, 12, &inputs, Verdict::Allow);
    assert_eq!(broker.dispatch_with_human(&automatic, &human, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::Stale);
    conserved(&broker);
}

#[test]
fn buffer_generation_floor_survives_capture_loss_and_cannot_be_rolled_back() {
    let (mut broker, _, contracts) = fixture();
    enable(&mut broker, 8);
    prepare(&mut broker, &contracts, 1, Some("generation"));
    broker.record_tensor_activation(1, 1, 0, host(&layout(), &bytes(false), 3), selected(1)).unwrap();
    broker.activation_unavailable(1, 2).unwrap();
    assert_eq!(broker.record_tensor_activation(1, 3, 0, host(&layout(), &bytes(false), 2), selected(2)).unwrap_err(), Error::Stale);
    assert!(broker.activation_report(1).unwrap().is_none());
    broker.record_tensor_activation(1, 3, 0, host(&layout(), &bytes(false), 4), selected(2)).unwrap();
    assert_eq!(broker.tensor_capture_receipt(1).unwrap().unwrap().buffer().generation, 4);
}

#[test]
fn nonfinite_recapture_holds_without_refunding_or_hiding_existing_disclosure() {
    let (mut broker, mut endpoint, contracts) = fixture();
    enable(&mut broker, 8);
    let (action, inputs) = prepare(&mut broker, &contracts, 1, Some("visible"));
    capture(&mut broker, 1, 1, false);
    review(&mut broker, 1, 11, &inputs, Verdict::Allow);
    let permit = broker.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let message = broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    let receipt = endpoint.deliver(&message).unwrap();
    broker.acknowledgment_lost(1).unwrap();
    broker.activation_unavailable(1, 2).unwrap();
    broker.accept_receipt(receipt).unwrap();
    let (_, next_inputs) = prepare(&mut broker, &contracts, 2, Some("blocked"));
    capture(&mut broker, 2, 2, false);
    review(&mut broker, 2, 12, &next_inputs, Verdict::Allow);
    let _permit = broker.authorize(2, Some(&next_inputs), &snapshot()).unwrap();
    let before = broker.inspect().ledger;
    let mut invalid = bytes(false);
    invalid[8..10].copy_from_slice(&0x7fc0_u16.to_le_bytes());
    assert_eq!(broker.record_tensor_activation(2, 2, 0, host(&layout(), &invalid, 2), selected(3)).unwrap_err(), Error::InvalidInput);
    assert_eq!(broker.inspect().ledger, before);
    assert!(broker.activation_report(2).unwrap().is_none());
    broker.cancel(2).unwrap();
    assert_eq!(endpoint.payload(), b"visible");
    assert!(broker.inspect().ledger.charged > 0);
    conserved(&broker);
}

#[test]
fn capacity_failure_invalidates_quiet_before_returning_limit() {
    let (mut broker, _, contracts) = fixture();
    enable(&mut broker, 1);
    prepare(&mut broker, &contracts, 1, Some("bounded"));
    capture(&mut broker, 1, 1, false);
    assert_eq!(broker.record_tensor_activation(1, 2, 0, host(&layout(), &bytes(false), 1), selected(2)).unwrap_err(), Error::Limit);
    assert_eq!(broker.input_revision(1).unwrap(), 3);
    assert!(broker.activation_report(1).unwrap().is_none());
    broker.cancel(1).unwrap();
    conserved(&broker);
}

#[test]
fn actor_advancement_requires_the_new_token_not_an_old_row_with_a_new_sequence() {
    let (mut broker, mut endpoint, contracts) = fixture();
    enable(&mut broker, 8);
    let (action, inputs) = prepare(&mut broker, &contracts, 1, Some("new token"));
    capture(&mut broker, 1, 1, false);
    review(&mut broker, 1, 11, &inputs, Verdict::Allow);
    let permit = broker.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let actor = ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
        model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
        grade: RestartGrade::FunctionalRestart }, vec![1, 2], vec![2], vec![3], 2).unwrap();
    broker.replace_actor_state(0, actor).unwrap();
    assert_eq!(broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::Stale);
    assert_eq!(broker.record_tensor_activation(1, 2, 1, host(&layout(), &bytes(false), 2), selected(2)).unwrap_err(), Error::Binding);
    let next_layout = TensorLayout::new([2, 2, 1, 2], [16, 8, 0, 2], 0,
        ScalarEncoding::BFloat16, ByteOrder::Little).unwrap();
    let mut next_bytes = vec![0; 28];
    next_bytes[24..26].copy_from_slice(&0xbf80_u16.to_le_bytes());
    broker.record_tensor_activation(1, 2, 1, host(&next_layout, &next_bytes, 2),
        TokenSelection { token: 1, ..selected(2) }).unwrap();
    review(&mut broker, 1, 12, &inputs, Verdict::Allow);
    let message = broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.payload(), b"new token");
}
