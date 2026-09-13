//! Original policy replacement, durable recovery and independently gated effects.
#![cfg(unix)]
#[path = "support/file_delivery.rs"] mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::persistent::{FileDelivery, JournalError, JournalLimits};
use fa_reference::action::consequence::delivery::persistent::governance::PolicyUpdate;
use fa_reference::action::consequence::delivery::{EndpointOutcome, StopRequest};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::MAX_POLICY_CHANGES;
use fa_reference::round::Verdict;
use fa_reference::{Error, Snapshot};

fn policy(generation: u64, limit: usize) -> Policy {
    Policy::new(generation, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() },
        Predicate::PayloadAtMost(limit), Predicate::All(vec![0, 1])]).unwrap()
}
fn update(host: &FileDelivery, id: u64, generation: u64, limit: usize) -> PolicyUpdate {
    let state = host.inspect().control;
    PolicyUpdate::new(id, state.sequence, state.ledger.epoch, policy(generation, limit)).unwrap()
}

#[test]
fn replacement_cancels_only_undispatched_work_and_new_work_uses_the_new_policy() {
    let root = Directory::new(); let mut host = create(&root);
    let (old_action, old_key) = approved(&mut host, 1, b"old approved");
    dispatched(&mut host, 2, b"already admitted");
    let command = update(&host, 77, 2, 3);
    let receipt = host.replace_policy(host.revision(), &command).unwrap();
    assert_eq!(receipt.change().cancelled, vec![1]);
    assert_eq!(receipt.change().refunded_units, 16);
    assert_eq!(receipt.change().revocation_floor, 1);
    assert_eq!(host.inspect().control.ledger.available, 84);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert!(host.dispatch(host.revision(), &old_key, &old_action, snapshot()).is_err());
    // Policy is ordered AFTER dispatch: it cannot retroactively revoke it.
    assert_eq!(host.publish(host.revision(), 2).unwrap(), EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 2).unwrap();
    let denied = host.propose(host.revision(), 3, spec(&host, b"too long"), snapshot()).unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&3], ActionState::Denied);
    assert!(host.authorize(host.revision(), 3, snapshot()).is_err());
    assert_eq!(denied.spec().policy_epoch, 1);
    let (action, key) = approved(&mut host, 4, b"new");
    host.dispatch(host.revision(), &key, &action, snapshot()).unwrap();
    host.publish(host.revision(), 4).unwrap();
    assert_eq!(host.inspect().executions, 2);
    assert_eq!(host.current_policy().unwrap(), command.policy());
}

#[test]
fn exact_operation_retry_survives_later_updates_recovery_and_missing_current_time() {
    let root = Directory::new(); let mut host = create(&root);
    let first = update(&host, 900, 2, 128);
    let receipt = host.replace_policy(host.revision(), &first).unwrap();
    let second = update(&host, 3, 3, 64);
    host.replace_policy(host.revision(), &second).unwrap();
    let before = host.inspect();
    assert_eq!(host.replace_policy(0, &first).unwrap(), receipt);
    assert_eq!(host.inspect(), before);
    assert_eq!(host.current_policy().unwrap().generation(), 3);
    drop(host);
    let mut recovered = FileDelivery::open(root.store(), profile()).unwrap();
    assert!(!recovered.clock_ready());
    let before = recovered.inspect();
    assert_eq!(recovered.replace_policy(0, &first).unwrap(), receipt);
    assert_eq!(recovered.policy_update_receipt(900).unwrap(), &receipt);
    assert_eq!(recovered.inspect(), before);
    assert_eq!(recovered.current_policy().unwrap(), second.policy());
    assert_eq!(recovered.inspect().control.ledger.epoch, 3);
    // Independent bootstrap stays the original one, not the last edited policy.
    drop(recovered);
    let mut substituted = profile(); substituted.policy = policy(3, 64);
    assert!(FileDelivery::open(root.store(), substituted).is_err());
}

#[test]
fn conflicting_keys_and_stale_predecessors_leave_the_original_reservation_intact() {
    let root = Directory::new(); let mut host = create(&root);
    let _ = approved(&mut host, 1, b"still valid");
    let wanted = update(&host, 1, 2, 64);
    let before = host.inspect();
    for candidate in [
        PolicyUpdate::new(1, wanted.expected_control_sequence() + 1, wanted.expected_authority_epoch(), policy(2, 64)).unwrap(),
        PolicyUpdate::new(1, wanted.expected_control_sequence(), wanted.expected_authority_epoch() + 1, policy(2, 64)).unwrap(),
        PolicyUpdate::new(1, wanted.expected_control_sequence(), wanted.expected_authority_epoch(), policy(1, 64)).unwrap(),
    ] {
        assert_eq!(host.replace_policy(host.revision(), &candidate), Err(JournalError::Contract(Error::Stale)));
        assert_eq!(host.inspect(), before);
    }
    let receipt = host.replace_policy(host.revision(), &wanted).unwrap();
    let before = host.inspect();
    for candidate in [
        PolicyUpdate::new(1, wanted.expected_control_sequence(), wanted.expected_authority_epoch(), policy(2, 65)).unwrap(),
        PolicyUpdate::new(1, wanted.expected_control_sequence() + 1, wanted.expected_authority_epoch(), policy(2, 64)).unwrap(),
        PolicyUpdate::new(1, wanted.expected_control_sequence(), wanted.expected_authority_epoch() + 1, policy(2, 64)).unwrap(),
    ] {
        assert_eq!(host.replace_policy(0, &candidate), Err(JournalError::Contract(Error::Binding)));
        assert_eq!(host.inspect(), before);
    }
    assert_eq!(host.replace_policy(0, &wanted).unwrap(), receipt);
}

#[test]
fn recorded_requests_are_not_rebased_onto_the_replacement_policy() {
    let root = Directory::new(); let mut host = create(&root);
    let original = spec(&host, b"original");
    let old = host.submit_request(host.revision(), 99, original.clone(), snapshot()).unwrap();
    let id = match old.disposition {
        fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition::Admitted { attempt, .. } => attempt,
        other => panic!("expected admission: {other:?}"),
    };
    host.review(host.revision(), review(id, 100, Verdict::Allow)).unwrap();
    host.authorize(host.revision(), id, snapshot()).unwrap();
    let command = update(&host, 4, 2, 128);
    host.replace_policy(host.revision(), &command).unwrap();
    let revision = host.revision();
    let retried = host.submit_request(0, 99, original, Snapshot::default()).unwrap();
    assert_eq!(retried.disposition,
        fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition::Admitted { attempt: id, stage: ActionState::Cancelled });
    assert_eq!(host.revision(), revision);
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(host.retained_requests(), 1);
}

#[test]
fn suspended_domains_stay_closed_and_policy_cannot_widen_the_registered_resource() {
    let root = Directory::new(); let mut host = create(&root);
    let before = host.inspect().control;
    host.request_stop(host.revision(), StopRequest { operation: 1,
        expected_control_sequence: before.sequence, expected_authority_epoch: before.ledger.epoch }).unwrap();
    let command = update(&host, 10, 2, 65536);
    host.replace_policy(host.revision(), &command).unwrap();
    assert!(host.inspect().control.suspended);
    assert!(host.propose(host.revision(), 1, spec(&host, b"new"), snapshot()).is_err());
    assert!(host.progress_stop(host.revision(), ElapsedTick(1)).unwrap().progress.drained());
    let other = Directory::new(); let mut healthy = create(&other);
    let mut foreign = healthy.inspect().target; foreign.object += 1;
    let state = healthy.inspect().control;
    let command = PolicyUpdate::new(1, state.sequence, state.ledger.epoch,
        Policy::new(2, vec![Predicate::TargetIs(foreign)]).unwrap()).unwrap();
    healthy.replace_policy(healthy.revision(), &command).unwrap();
    let mut action = spec(&healthy, b"foreign"); action.target = Some(foreign);
    assert!(healthy.propose(healthy.revision(), 1, action, snapshot()).is_err());
    assert_eq!(healthy.inspect().control.ledger.available, 100);
    assert_eq!(healthy.inspect().executions, 0);
}

#[test]
fn original_change_limit_and_journal_limit_do_not_break_exact_retry() {
    let root = Directory::new(); let mut host = create(&root);
    let first = update(&host, 1, 2, 128);
    let receipt = host.replace_policy(host.revision(), &first).unwrap();
    for id in 2..=MAX_POLICY_CHANGES as u64 {
        let command = update(&host, id, id + 1, 128);
        host.replace_policy(host.revision(), &command).unwrap();
    }
    let before = host.inspect();
    let extra = update(&host, 1000, 1000, 128);
    assert_eq!(host.replace_policy(host.revision(), &extra), Err(JournalError::Contract(Error::Limit)));
    assert_eq!(host.inspect(), before);
    assert_eq!(host.replace_policy(0, &first).unwrap(), receipt);
    let root = Directory::new(); let mut p = profile();
    p.limits = JournalLimits { events: 1, ..JournalLimits::default() };
    let mut capped = FileDelivery::create(root.store(), p).unwrap();
    let command = update(&capped, 3, 2, 128);
    let receipt = capped.replace_policy(0, &command).unwrap();
    assert_eq!(capped.replace_policy(0, &command).unwrap(), receipt);
    assert_eq!(capped.revision(), 1);
}
