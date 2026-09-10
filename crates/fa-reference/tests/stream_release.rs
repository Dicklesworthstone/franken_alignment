//! Full cumulative review -> bounded append -> terminal receipt, using only
//! public reference APIs. No real model, network service or OS durability claim.

#[path = "support/stream_fixture.rs"]
pub mod support;

use support::*;
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::delivery::stream::{ReleaseFrame, StreamProfile, StreamView};
use fa_reference::action::consequence::oversight::human::{HumanDisposition, HumanReviewPolicy};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fa_reference::Error;

#[test]
fn complete_messages_are_reviewed_before_disclosure_and_finish_preserves_the_prefix() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let (action, inputs) = prepare(&mut broker, &contracts, 1, Some("Hello, "));
    assert!(endpoint.payload().is_empty());
    assert_eq!(broker.stream_state().unwrap().1.message_count(), 0);
    review(&mut broker, 1, 1, &inputs, Verdict::Allow);
    let permit = broker.authorize(1, Some(&inputs), &snapshot()).unwrap();
    assert!(endpoint.payload().is_empty());
    let envelope = broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    assert!(endpoint.payload().is_empty());
    let receipt = endpoint.deliver(&envelope).unwrap();
    assert_eq!(endpoint.payload(), b"Hello, ");
    assert!(broker.stream_state().unwrap().1.visible().is_empty());
    broker.accept_receipt(receipt).unwrap();

    let (action, inputs, permit) = ready(&mut broker, &contracts, 2, Some("world!"));
    let parsed = ReleaseFrame::decode(&action.spec().payload).unwrap();
    assert_eq!(parsed.prior_messages(), &["Hello, "]);
    assert_eq!(parsed.message(), Some("world!"));
    assert_eq!(inputs.views()["helper"].actual_input().part_bytes(0).unwrap(),
        fa_reference::action::consequence::oversight::action_frame(&action));
    let second = broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&second).unwrap()).unwrap();
    assert_eq!(endpoint.payload(), b"Hello, world!");
    assert_eq!(endpoint.stream_view().unwrap().messages().collect::<Vec<_>>(), vec!["Hello, ", "world!"]);
    assert_eq!(broker.stream_state().unwrap().1, endpoint.stream_view().unwrap());
    assert!(broker.retained_stream_bytes() >= b"Hello, world!".len());

    let finish = publish(&mut broker, &mut endpoint, &contracts, 3, None);
    assert!(ReleaseFrame::decode(finish.request().payload()).unwrap().is_finish());
    assert!(endpoint.stream_view().unwrap().finished());
    assert!(broker.stream_state().unwrap().1.finished());
    assert_eq!(endpoint.payload(), b"Hello, world!");
    assert_eq!(broker.stream_message_spec("too late", ElapsedTick(100)), Err(Error::WrongState));
    assert_eq!(endpoint.execution_count(), 3);
    let charged = envelope.request().units() + second.request().units() + finish.request().units();
    assert_eq!(broker.inspect().ledger.charged, charged);
    conserved(&broker);
}

#[test]
fn unresolved_release_blocks_a_preapproved_sibling_and_a_new_proposal() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let (first, first_input, first_permit) = ready(&mut broker, &contracts, 1, Some("first"));
    let (sibling, sibling_input, sibling_permit) = ready(&mut broker, &contracts, 2, Some("sibling"));
    let envelope = broker.dispatch(&first_permit, &first, Some(&first_input), &snapshot()).unwrap();
    let receipt = endpoint.deliver(&envelope).unwrap();
    broker.acknowledgment_lost(1).unwrap();
    let before = broker.inspect();
    assert_eq!(broker.stream_pending(), Some(1));
    assert_eq!(broker.stream_message_spec("next", ElapsedTick(100)), Err(Error::Incomplete));
    assert_eq!(broker.stream_finish_spec(ElapsedTick(100)), Err(Error::Incomplete));
    assert_eq!(broker.dispatch(&sibling_permit, &sibling, Some(&sibling_input), &snapshot()).unwrap_err(), Error::Incomplete);
    assert_eq!(broker.inspect(), before);
    assert_eq!(endpoint.payload(), b"first");
    broker.accept_receipt(receipt).unwrap();
    assert_eq!(broker.stream_pending(), None);
    assert_eq!(broker.dispatch(&sibling_permit, &sibling, Some(&sibling_input), &snapshot()).unwrap_err(), Error::Stale);
    broker.cancel(2).unwrap();
    publish(&mut broker, &mut endpoint, &contracts, 3, Some("next"));
    assert_eq!(endpoint.payload(), b"firstnext");
    assert_eq!(endpoint.execution_count(), 2);
    conserved(&broker);
}

#[test]
fn late_duplicate_receipt_cannot_clear_a_different_pending_release() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let first = publish(&mut broker, &mut endpoint, &contracts, 1, Some("A"));
    let first_receipt = endpoint.deliver(&first).unwrap();
    let (action, inputs, permit) = ready(&mut broker, &contracts, 2, Some("B"));
    let second = broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    assert_eq!(broker.accept_receipt(first_receipt), Ok(false));
    assert_eq!(broker.stream_pending(), Some(2));
    assert_eq!(endpoint.payload(), b"A");
    let receipt = endpoint.deliver(&second).unwrap();
    assert_eq!(endpoint.deliver(&second).unwrap(), receipt);
    broker.accept_receipt(receipt).unwrap();
    assert_eq!(endpoint.payload(), b"AB");
    assert_eq!(endpoint.execution_count(), 2);
    conserved(&broker);
}

#[test]
fn cumulative_hold_does_not_erase_a_previously_disclosed_message() {
    let (mut broker, mut endpoint, contracts) = fixture();
    publish(&mut broker, &mut endpoint, &contracts, 1, Some("account: "));
    let (held, held_input) = prepare(&mut broker, &contracts, 2, Some("private suffix"));
    assert_eq!(ReleaseFrame::decode(&held.spec().payload).unwrap().prior_messages(), &["account: "]);
    let receipt = review(&mut broker, 2, 2, &held_input, Verdict::Hold);
    assert_eq!(receipt.policy.control.decision.consequence, Consequence::HoldEffect);
    assert!(broker.authorize(2, Some(&held_input), &snapshot()).is_err());
    broker.cancel(2).unwrap();
    assert_eq!(endpoint.payload(), b"account: ");
    assert!(!endpoint.stream_view().unwrap().finished());
    publish(&mut broker, &mut endpoint, &contracts, 3, Some("[withheld]"));
    assert_eq!(endpoint.payload(), b"account: [withheld]");
    conserved(&broker);
}

#[test]
fn exact_denial_of_a_later_unit_cannot_refund_the_released_prefix() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let first = publish(&mut broker, &mut endpoint, &contracts, 1, Some("released"));
    let spec = broker.stream_message_spec("denied", ElapsedTick(100)).unwrap();
    let mut changed = snapshot(); changed.values.insert(7, vec![8]);
    let denied = broker.propose(2, spec, &changed).unwrap();
    assert_eq!(denied.state, ActionState::Denied);
    assert_eq!(broker.inspect().ledger.charged, first.request().units());
    assert_eq!(endpoint.payload(), b"released");
    assert_eq!(broker.stream_pending(), None);
    publish(&mut broker, &mut endpoint, &contracts, 3, Some(" accepted"));
    assert_eq!(endpoint.payload(), b"released accepted");
    conserved(&broker);
}

#[test]
fn malformed_or_fabricated_prefixes_refuse_before_any_release() {
    let (mut broker, mut endpoint, contracts) = fixture();
    publish(&mut broker, &mut endpoint, &contracts, 1, Some("prior"));
    let valid = broker.stream_message_spec("é", ElapsedTick(100)).unwrap();
    let before = broker.inspect();
    let mut truncated = valid.clone(); truncated.payload.pop();
    assert!(broker.propose(2, truncated, &snapshot()).is_err());
    let mut invalid_utf8 = valid.clone(); *invalid_utf8.payload.last_mut().unwrap() = 0xff;
    assert!(broker.propose(2, invalid_utf8, &snapshot()).is_err());
    let mut raw_tool = valid.clone(); raw_tool.payload = b"{\"tool\":\"execute\"}".to_vec();
    assert!(broker.propose(2, raw_tool, &snapshot()).is_err());
    let mut omitted = valid.clone();
    let empty = StreamView::empty(broker.stream_state().unwrap().1.profile());
    omitted.payload = empty.encode_message("é").unwrap();
    assert_eq!(broker.propose(2, omitted, &snapshot()).unwrap_err(), Error::Binding);
    assert_eq!(broker.inspect(), before);
    assert_eq!(endpoint.payload(), b"prior");
    let proposal = broker.propose(2, valid, &snapshot()).unwrap();
    let inputs = input(&proposal.action, &contracts);
    broker.record_inputs(2, 0, inputs.clone()).unwrap();
    review(&mut broker, 2, 2, &inputs, Verdict::Allow);
    let permit = broker.authorize(2, Some(&inputs), &snapshot()).unwrap();
    let envelope = broker.dispatch(&permit, &proposal.action, Some(&inputs), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&envelope).unwrap()).unwrap();
    assert_eq!(endpoint.payload(), "prioré".as_bytes());
    conserved(&broker);
}

#[test]
fn reaching_message_and_byte_limits_does_not_turn_cancel_into_finish() {
    let (mut broker, mut endpoint, contracts) = fixture_with(StreamProfile::new(9, 2, 2, 4, 5).unwrap());
    publish(&mut broker, &mut endpoint, &contracts, 1, Some("123"));
    publish(&mut broker, &mut endpoint, &contracts, 2, Some("45"));
    assert_eq!(broker.stream_message_spec("6", ElapsedTick(100)), Err(Error::Limit));
    let (finish, inputs, permit) = ready(&mut broker, &contracts, 3, None);
    broker.cancel(3).unwrap();
    assert!(!endpoint.stream_view().unwrap().finished());
    assert!(broker.dispatch(&permit, &finish, Some(&inputs), &snapshot()).is_err());
    publish(&mut broker, &mut endpoint, &contracts, 4, None);
    assert!(endpoint.stream_view().unwrap().finished());
    assert_eq!(endpoint.payload(), b"12345");
    conserved(&broker);
}

#[test]
fn each_stream_unit_still_needs_its_own_human_key() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let reviewer = broker.enable_human_review(HumanReviewPolicy {
        reviewer_id: 90, max_validity_ticks: 20, max_requests: 8,
    }).unwrap();
    let (first, inputs, automatic) = ready(&mut broker, &contracts, 1, Some("first"));
    assert_eq!(broker.dispatch(&automatic, &first, Some(&inputs), &snapshot()).unwrap_err(), Error::Incomplete);
    let request = broker.request_human_approval(101, 1, Some(&inputs), ElapsedTick(10)).unwrap();
    let human = reviewer.approve(&request, ElapsedTick(1)).unwrap();
    let envelope = broker.dispatch_with_human(&automatic, &human, &first, Some(&inputs), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&envelope).unwrap()).unwrap();
    assert_eq!(broker.human_status(101).unwrap().disposition, HumanDisposition::Consumed);
    let (second, inputs, automatic) = ready(&mut broker, &contracts, 2, Some("second"));
    let before = broker.inspect();
    assert!(broker.dispatch_with_human(&automatic, &human, &second, Some(&inputs), &snapshot()).is_err());
    assert_eq!(broker.inspect(), before);
    let request = broker.request_human_approval(102, 2, Some(&inputs), ElapsedTick(10)).unwrap();
    let human = reviewer.approve(&request, ElapsedTick(1)).unwrap();
    let envelope = broker.dispatch_with_human(&automatic, &human, &second, Some(&inputs), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&envelope).unwrap()).unwrap();
    assert_eq!(endpoint.payload(), b"firstsecond");
    conserved(&broker);
}
