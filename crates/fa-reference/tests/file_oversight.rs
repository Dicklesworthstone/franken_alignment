//! The public durable host exercises the original full-input and two-key gates.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod fixture;
use fixture::*;
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::*;
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fa_reference::Error;

#[test]
fn complete_native_review_and_both_keys_publish_once_and_reconcile_the_original_charge() {
    let root = Directory::new();
    let (mut host, reviewer) = create(&root);
    let keys = ready(&mut host, &reviewer, 1, b"visible");
    assert_eq!(keys.request.evidence().inputs(), &keys.inputs);
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Approved);
    assert_eq!(host.inspect().control.ledger.reserved, 16);
    assert_eq!(host.inspect().control.ledger.charged, 0);
    assert!(host.publish(host.revision(), 1).is_err());
    dispatch(&mut host, &keys);
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Consumed);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).is_err());
    let outcome = EndpointOutcome::Executed { resulting_version: 2 };
    assert_eq!(host.publish(host.revision(), 1).unwrap(), outcome);
    assert_eq!(host.publish(host.revision(), 1).unwrap(), outcome);
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(outcome));
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(host.inspect().control.ledger.available, 84);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
    assert_eq!(disk.payload, b"visible");
    assert_eq!(disk.executions, 1);
    assert_eq!(disk.control, host.inspect().control);
}

#[test]
fn foreign_reviewer_and_keys_cannot_cross_owner_brands_with_identical_numeric_ids() {
    let first = Directory::new();
    let second = Directory::new();
    let (mut a, reviewer_a) = create(&first);
    let (mut b, reviewer_b) = create(&second);
    let keys_a = ready(&mut a, &reviewer_a, 1, b"same action");
    let keys_b = ready(&mut b, &reviewer_b, 1, b"same action");
    let before_a = a.inspect();
    let before_b = b.inspect();
    let revision = a.revision();
    assert_eq!(reviewer_b.approve(&mut a, revision, &keys_a.request).unwrap_err(), JournalError::Contract(Error::Binding));
    assert_eq!(a.dispatch(a.revision(), &keys_a.automatic, &keys_b.human, &keys_a.action, &keys_a.inputs, snapshot()).unwrap_err(),
        JournalError::Contract(Error::Binding));
    assert_eq!(a.dispatch(a.revision(), &keys_b.automatic, &keys_a.human, &keys_a.action, &keys_a.inputs, snapshot()).unwrap_err(),
        JournalError::Contract(Error::Binding));
    assert_eq!(a.inspect(), before_a);
    assert_eq!(b.inspect(), before_b);
    dispatch(&mut a, &keys_a);
    a.publish(a.revision(), 1).unwrap();
    assert_eq!(a.inspect().executions, 1);
    assert_eq!(b.inspect(), before_b);
}

#[test]
fn changed_complete_input_cannot_reuse_an_old_congress_approval_or_human_key() {
    let root = Directory::new();
    let (mut host, reviewer) = create(&root);
    let keys = ready(&mut host, &reviewer, 1, b"pending");
    let changed = inputs(&keys.action, b"changed complete source view");
    let before = host.inspect();
    assert_eq!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &changed, snapshot()).unwrap_err(),
        JournalError::Contract(Error::Stale));
    assert_eq!(host.inspect(), before);
    let expected = host.input_revision(1).unwrap();
    assert_eq!(host.record_inputs(host.revision(), 1, expected, changed.clone()).unwrap(), expected + 1);
    assert!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &changed, snapshot()).is_err());
    assert!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).is_err());
    assert_eq!(host.inspect().control.ledger.reserved, 16);
    assert_eq!(host.inspect().control.ledger.charged, 0);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Approved);
    host.cancel(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn missing_helpers_remain_in_the_original_denominator_and_phase_deadlines_are_real() {
    let root = Directory::new();
    let (mut host, _) = create(&root);
    let action = host.propose(host.revision(), 1, spec(&host, b"pending"), snapshot()).unwrap();
    let input = inputs(&action, b"complete");
    host.record_inputs(host.revision(), 1, 0, input.clone()).unwrap();
    let window = window(&host);
    host.begin_review(host.revision(), 1, 101, ROOT, window, snapshot()).unwrap();
    commit(&mut host, 101, "alpha", Verdict::Allow);
    assert_eq!(host.open_reveals(host.revision(), 101).unwrap_err(), JournalError::Contract(Error::Incomplete));
    host.observe_time(host.revision(), window.commit_by).unwrap();
    host.open_reveals(host.revision(), 101).unwrap();
    host.reveal_review(host.revision(), 101, "alpha", Verdict::Allow, salt("alpha")).unwrap();
    assert_eq!(host.finish_review(host.revision(), 101, Some(&input), snapshot()).unwrap_err(), JournalError::Contract(Error::Incomplete));
    host.observe_time(host.revision(), window.reveal_by).unwrap();
    let receipt = host.finish_review(host.revision(), 101, Some(&input), snapshot()).unwrap().unwrap();
    assert_ne!(receipt.policy.control.decision.consequence, Consequence::Continue);
    assert!(host.authorize(host.revision(), 1, &input, snapshot()).is_err());
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn a_completed_stale_review_is_committed_as_refused_and_cannot_be_repaired_or_rerolled() {
    let root = Directory::new();
    let (mut host, _) = create(&root);
    let action = host.propose(host.revision(), 1, spec(&host, b"pending"), snapshot()).unwrap();
    let original = inputs(&action, b"original complete view");
    host.record_inputs(host.revision(), 1, 0, original.clone()).unwrap();
    host.begin_review(host.revision(), 1, 101, ROOT, window(&host), snapshot()).unwrap();
    votes(&mut host, 101, Verdict::Allow);
    let changed = inputs(&action, b"replacement complete view");
    let revision = host.revision();
    assert_eq!(host.finish_review(revision, 101, Some(&changed), snapshot()).unwrap(), Err(Error::Stale));
    assert_eq!(host.revision(), revision + 1);
    assert!(host.finish_review(host.revision(), 101, Some(&original), snapshot()).is_err());
    assert_eq!(host.begin_review(host.revision(), 1, 101, ROOT, window(&host), snapshot()).unwrap_err(),
        JournalError::Contract(Error::Duplicate));
    assert!(host.authorize(host.revision(), 1, &original, snapshot()).is_err());
    let receipt = review_existing(&mut host, 1, 201, &changed);
    assert_eq!(receipt.policy.control.decision.consequence, Consequence::Continue);
    assert_eq!(receipt.inputs.as_ref(), &changed);
    assert!(host.authorize(host.revision(), 1, &changed, snapshot()).is_ok());
    assert_eq!(host.inspect().control.ledger.reserved, 16);
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap().control.ledger.reserved, 16);
}

#[test]
fn second_key_expiry_is_enforced_at_publication_and_only_its_receipt_refunds() {
    let root = Directory::new();
    let (mut host, reviewer) = create(&root);
    let keys = ready(&mut host, &reviewer, 1, b"never visible");
    dispatch(&mut host, &keys);
    host.observe_time(host.revision(), keys.request.evidence().expires_at()).unwrap();
    let outcome = EndpointOutcome::NotExecuted { reason: NonExecutionReason::DeadlineElapsed };
    assert_eq!(host.publish(host.revision(), 1).unwrap(), outcome);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(outcome));
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::ConfirmedNotExecuted);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().payload, b"initial");
}

#[test]
fn human_revocation_withdraws_the_key_but_does_not_refund_an_automatic_reservation() {
    let root = Directory::new();
    let (mut host, reviewer) = create(&root);
    let keys = ready(&mut host, &reviewer, 1, b"withdrawn");
    let revision = host.revision();
    reviewer.revoke(&mut host, revision, &keys.request).unwrap();
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Revoked);
    assert!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).is_err());
    assert_eq!(host.inspect().control.ledger.reserved, 16);
    assert_eq!(host.inspect().control.ledger.available, 84);
    host.cancel(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn a_conflicting_reveal_cannot_replace_the_original_commitment_but_a_matching_one_can_complete() {
    let root = Directory::new();
    let (mut host, _) = create(&root);
    let action = host.propose(host.revision(), 1, spec(&host, b"pending"), snapshot()).unwrap();
    let input = inputs(&action, b"complete");
    host.record_inputs(host.revision(), 1, 0, input.clone()).unwrap();
    host.begin_review(host.revision(), 1, 101, ROOT, window(&host), snapshot()).unwrap();
    for member in MEMBERS { commit(&mut host, 101, member, Verdict::Allow); }
    host.open_reveals(host.revision(), 101).unwrap();
    let before = host.inspect();
    assert_eq!(host.reveal_review(host.revision(), 101, "alpha", Verdict::Deny, salt("alpha")).unwrap_err(),
        JournalError::Contract(Error::Binding));
    assert_eq!(host.inspect(), before);
    for member in MEMBERS { host.reveal_review(host.revision(), 101, member, Verdict::Allow, salt(member)).unwrap(); }
    let receipt = host.finish_review(host.revision(), 101, Some(&input), snapshot()).unwrap().unwrap();
    assert_eq!(receipt.policy.control.decision.consequence, Consequence::Continue);
    assert!(host.authorize(host.revision(), 1, &input, snapshot()).is_ok());
}
