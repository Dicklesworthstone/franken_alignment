//! Durable updates through actual whole-input and mandatory-human-key authority.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::governance::PolicyUpdate;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::Error;

fn update(host: &FileOversight, operation: u64, generation: u64, limit: usize) -> PolicyUpdate {
    let control = host.inspect().control;
    PolicyUpdate::new(operation, control.sequence, control.ledger.epoch,
        Policy::new(generation, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() },
            Predicate::PayloadAtMost(limit), Predicate::All(vec![0, 1])]).unwrap()).unwrap()
}

#[test]
fn policy_change_withdraws_old_human_keys_then_fresh_review_and_both_keys_can_publish() {
    let root = Directory::new(); let (mut host, role) = create(&root);
    let old = ready(&mut host, &role, 1, b"old effect");
    let command = update(&host, 1, 2, 3);
    let receipt = host.replace_policy(host.revision(), &command).unwrap();
    assert_eq!(receipt.change().cancelled, vec![1]);
    assert_eq!(receipt.change().refunded_units, 16);
    assert_eq!(host.human_status(old.human.request()).unwrap().disposition, HumanDisposition::Revoked);
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert!(host.dispatch(host.revision(), &old.automatic, &old.human, &old.action, &old.inputs, snapshot()).is_err());
    let denied = host.propose(host.revision(), 2, spec(&host, b"too long"), snapshot()).unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&2], ActionState::Denied);
    let current = inputs(&denied, b"new context cannot override exact denial");
    assert!(host.authorize(host.revision(), 2, &current, snapshot()).is_err());
    let new = ready(&mut host, &role, 3, b"new");
    assert!(host.publish(host.revision(), 3).is_err());
    dispatch(&mut host, &new);
    assert_eq!(host.publish(host.revision(), 3).unwrap(), EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().payload, b"new");
}

#[test]
fn new_policy_does_not_unspend_a_consumed_key_or_revoke_earlier_dispatch() {
    let root = Directory::new(); let (mut host, role) = create(&root);
    let admitted = ready(&mut host, &role, 1, b"already admitted");
    dispatch(&mut host, &admitted);
    let command = update(&host, 2, 2, 0);
    let receipt = host.replace_policy(host.revision(), &command).unwrap();
    assert!(receipt.change().cancelled.is_empty());
    assert_eq!(receipt.change().refunded_units, 0);
    assert_eq!(host.human_status(admitted.human.request()).unwrap().disposition, HumanDisposition::Consumed);
    assert_eq!(host.publish(host.revision(), 1).unwrap(), EndpointOutcome::Executed { resulting_version: 2 });
    drop(host); drop(role);
    let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
    let revision = host.revision();
    assert_eq!(host.replace_policy(0, &command).unwrap(), receipt);
    assert_eq!(host.revision(), revision);
    assert_eq!(host.current_policy().unwrap(), command.policy());
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(),
        Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }));
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().executions, 1);
    assert!(host.publish(host.revision(), 1).is_err());
}

#[test]
fn an_incomplete_review_is_not_resumable_after_replacement_and_recovery() {
    let root = Directory::new(); let (mut host, _) = create(&root);
    let action = host.propose(host.revision(), 1, spec(&host, b"old"), snapshot()).unwrap();
    let evidence = inputs(&action, b"whole original evidence");
    host.record_inputs(host.revision(), 1, 0, evidence.clone()).unwrap();
    host.begin_review(host.revision(), 1, 101, ROOT, window(&host), snapshot()).unwrap();
    commit(&mut host, 101, "alpha", fa_reference::round::Verdict::Allow);
    let command = update(&host, 8, 2, 128);
    host.replace_policy(host.revision(), &command).unwrap();
    assert!(host.open_reveals(host.revision(), 101).is_err());
    assert!(host.finish_review(host.revision(), 101, Some(&evidence), snapshot()).is_err());
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    drop(host);
    let (mut host, role) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(host.current_policy().unwrap().generation(), 2);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert!(host.begin_review(host.revision(), 1, 101, ROOT, window(&host), snapshot()).is_err());
    let next = ready(&mut host, &role, 2, b"new"); dispatch(&mut host, &next);
    host.publish(host.revision(), 2).unwrap(); assert_eq!(host.inspect().executions, 1);
}

#[test]
fn rejected_governance_preserves_a_usable_original_human_key() {
    let root = Directory::new(); let (mut host, role) = create(&root);
    let keys = ready(&mut host, &role, 1, b"valid");
    let command = update(&host, 9, 2, 128);
    let wrong = PolicyUpdate::new(9, command.expected_control_sequence() + 1,
        command.expected_authority_epoch(), command.policy().clone()).unwrap();
    let before = host.inspect();
    assert_eq!(host.replace_policy(host.revision(), &wrong), Err(JournalError::Contract(Error::Stale)));
    assert_eq!(host.inspect(), before);
    assert_eq!(host.human_status(keys.human.request()).unwrap().disposition, HumanDisposition::Approved);
    dispatch(&mut host, &keys); host.publish(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().executions, 1);
}
