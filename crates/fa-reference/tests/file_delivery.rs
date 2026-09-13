#![cfg(unix)]
#[path = "support/file_delivery.rs"] mod fixture;
use fixture::*;
use fa_reference::action::consequence::delivery::persistent::*;
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::{ActionState, ElapsedTick, FrozenAction};
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::fs;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

#[test]
fn original_reviews_permits_endpoint_and_receipts_drive_one_persistent_publication() {
    let root = Directory::new(); let mut host = create(&root);
    let (action, key) = approved(&mut host, 1, b"published");
    assert_eq!(host.inspect().control.ledger.reserved, 16);
    host.dispatch(host.revision(), &key, &action, snapshot()).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(FileDelivery::read_publication(root.store(), &profile()).unwrap().payload, b"initial");
    let outcome = EndpointOutcome::Executed { resulting_version: 2 };
    assert_eq!(host.publish(host.revision(), 1).unwrap(), outcome);
    assert_eq!(host.publish(host.revision(), 1).unwrap(), outcome);
    let visible = FileDelivery::read_publication(root.store(), &profile()).unwrap();
    assert_eq!(visible.payload, b"published"); assert_eq!(visible.executions, 1);
    assert_eq!(visible.control.ledger.stages[&1], ActionState::Dispatching);
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(outcome));
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(outcome));
    assert_eq!(host.inspect().control.ledger.available, 84);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Confirmed);
    assert!(host.dispatch(host.revision(), &key, &action, snapshot()).is_err());
}

#[test]
fn exact_policy_and_missing_congress_evidence_cannot_be_replaced_by_a_publish_request() {
    let root = Directory::new(); let mut host = create(&root);
    let mut absent = snapshot(); absent.complete = false;
    let before = host.inspect();
    assert!(host.propose(host.revision(), 1, spec(&host, b"x"), absent).is_err());
    assert_eq!(host.inspect(), before);
    let mut bad = snapshot(); bad.values.insert(7, b"different".to_vec());
    host.propose(host.revision(), 1, spec(&host, b"x"), bad).unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Denied);
    assert!(host.authorize(host.revision(), 1, snapshot()).is_err());
    assert!(host.publish(host.revision(), 1).is_err());
    host.propose(host.revision(), 2, spec(&host, b"x"), snapshot()).unwrap();
    let mut missing = review(2, 102, Verdict::Allow); missing.ballots.remove("beta");
    host.review(host.revision(), missing).unwrap();
    assert!(host.authorize(host.revision(), 2, snapshot()).is_err());
    host.review(host.revision(), review(2, 103, Verdict::Allow)).unwrap();
    let key = host.authorize(host.revision(), 2, snapshot()).unwrap();
    assert_eq!(key.attempt(), 2); assert_eq!(host.inspect().executions, 0);
}

#[test]
fn altered_action_snapshot_and_stale_revision_preserve_the_original_reservation() {
    let root = Directory::new(); let mut host = create(&root);
    let (action, key) = approved(&mut host, 1, b"original");
    let mut changed = action.spec().clone(); changed.payload = b"altered".to_vec();
    let changed = FrozenAction::freeze(changed).unwrap();
    let before = host.inspect();
    assert_eq!(host.dispatch(host.revision(), &key, &changed, snapshot()).unwrap_err(), JournalError::Contract(Error::Binding));
    let mut stale = snapshot(); stale.values.insert(7, b"changed".to_vec());
    assert!(host.dispatch(host.revision(), &key, &action, stale).is_err());
    assert_eq!(host.dispatch(host.revision() - 1, &key, &action, snapshot()).unwrap_err(), JournalError::Contract(Error::Stale));
    assert_eq!(host.inspect(), before); assert!(host.storage_failure().is_none());
    host.dispatch(host.revision(), &key, &action, snapshot()).unwrap();
    assert!(host.publish(host.revision(), 1).is_ok());
}

#[test]
fn full_owner_loss_preserves_executed_but_unacknowledged_effect_without_resending() {
    let root = Directory::new(); let mut host = create(&root);
    dispatched(&mut host, 1, b"visible"); host.publish(host.revision(), 1).unwrap();
    let before = host.inspect(); drop(host);
    let mut resumed = FileDelivery::open(root.store(), profile()).unwrap();
    assert!(!resumed.clock_ready());
    let recovered = resumed.inspect();
    assert_eq!(recovered.payload, b"visible"); assert_eq!(recovered.executions, 1);
    assert_eq!(recovered.control.ledger.stages[&1], ActionState::Unknown);
    assert_eq!(recovered.control.ledger.charged, 16);
    assert!(recovered.control.ledger.epoch > before.control.ledger.epoch);
    assert!(recovered.dispatcher_epoch > before.dispatcher_epoch);
    assert!(resumed.reconcile(resumed.revision(), 1).is_err());
    resumed.observe_time(resumed.revision(), ElapsedTick(2)).unwrap();
    assert!(resumed.publish(resumed.revision(), 1).is_err());
    assert!(matches!(resumed.reconcile(resumed.revision(), 1).unwrap(),
        Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 })));
    assert_eq!(resumed.inspect().control.ledger.charged, 16);
    assert_eq!(resumed.inspect().executions, 1);
    drop(resumed);
    let again = FileDelivery::open(root.store(), profile()).unwrap();
    assert_eq!(again.inspect().control.ledger.charged, 16);
    assert_eq!(again.inspect().control.ledger.stages[&1], ActionState::Confirmed);
}

#[test]
fn unsent_dispatch_stays_charged_until_original_endpoint_seals_its_key() {
    let root = Directory::new(); let mut host = create(&root);
    dispatched(&mut host, 1, b"not sent"); drop(host);
    let mut resumed = FileDelivery::open(root.store(), profile()).unwrap();
    resumed.observe_time(resumed.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(resumed.reconcile(resumed.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
    assert_eq!(resumed.inspect().control.ledger.charged, 16);
    assert!(resumed.cancel(resumed.revision(), 1).is_err());
    assert!(resumed.publish(resumed.revision(), 1).is_err());
    assert_eq!(resumed.seal_unexecuted(resumed.revision(), 1).unwrap(),
        Reconciliation::Resolved(EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed }));
    assert_eq!(resumed.inspect().control.ledger.available, 100);
    assert_eq!(resumed.inspect().payload, b"initial");
    drop(resumed);
    let mut again = FileDelivery::open(root.store(), profile()).unwrap();
    again.observe_time(again.revision(), ElapsedTick(3)).unwrap();
    dispatched(&mut again, 2, b"fresh"); again.publish(again.revision(), 2).unwrap();
    assert_eq!(again.inspect().executions, 1);
    assert_eq!(again.inspect().control.ledger.stages[&1], ActionState::ConfirmedNotExecuted);
}

#[test]
fn reopening_cancels_unspent_reservations_but_cannot_revive_the_old_permit() {
    let root = Directory::new(); let mut host = create(&root);
    let (action, old_key) = approved(&mut host, 1, b"old"); drop(host);
    let mut resumed = FileDelivery::open(root.store(), profile()).unwrap();
    assert_eq!(resumed.inspect().control.ledger.available, 100);
    assert_eq!(resumed.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    resumed.observe_time(resumed.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(resumed.dispatch(resumed.revision(), &old_key, &action, snapshot()).unwrap_err(), JournalError::Contract(Error::Binding));
    assert!(resumed.propose(resumed.revision(), 1, spec(&resumed, b"reused"), snapshot()).is_err());
    dispatched(&mut resumed, 2, b"fresh"); resumed.publish(resumed.revision(), 2).unwrap();
    assert_eq!(resumed.inspect().payload, b"fresh");
}

#[test]
fn kernel_lock_and_exact_bootstrap_and_location_binding_prevent_accidental_second_owners() {
    let root = Directory::new(); let host = create(&root);
    assert_eq!(FileDelivery::open(root.store(), profile()).unwrap_err(), JournalError::Busy);
    assert!(FileDelivery::create(root.store(), profile()).is_err());
    drop(host);
    for field in 0..3 {
        let mut changed = profile();
        match field { 0 => changed.total += 1, 1 => changed.clock_domain += 1, _ => changed.congress.continue_minimum -= 1 }
        assert_eq!(FileDelivery::open(root.store(), changed).unwrap_err(), JournalError::Contract(Error::Binding));
    }
    let copy = root.0.join("copy"); fs::DirBuilder::new().mode(0o700).create(&copy).unwrap();
    fs::copy(root.store().join("delivery.bin"), copy.join("delivery.bin")).unwrap();
    fs::write(copy.join("delivery.lock"), b"").unwrap();
    assert_eq!(FileDelivery::open(&copy, profile()).unwrap_err(), JournalError::Contract(Error::Binding));
    assert!(FileDelivery::open(root.store(), profile()).is_ok());
}

#[test]
fn storage_failure_blocks_old_keys_and_staging_is_never_treated_as_publication() {
    let root = Directory::new(); let mut host = create(&root);
    let (action, key) = approved(&mut host, 1, b"blocked");
    fs::write(root.store().join("delivery.pending"), b"partial untrusted staging").unwrap();
    assert!(matches!(host.dispatch(host.revision(), &key, &action, snapshot()), Err(JournalError::Io(_))));
    assert!(host.storage_failure().is_some()); assert!(!host.clock_ready());
    assert_eq!(host.observe_time(host.revision(), ElapsedTick(2)).unwrap_err(), JournalError::Unavailable);
    assert_eq!(host.inspect().control.ledger.reserved, 16);
    assert_eq!(FileDelivery::read_publication(root.store(), &profile()).unwrap().payload, b"initial");
    drop(host);
    let resumed = FileDelivery::open(root.store(), profile()).unwrap();
    assert_eq!(resumed.inspect().control.ledger.available, 100);
    assert_eq!(resumed.inspect().executions, 0);
    assert!(!root.store().join("delivery.pending").exists());
}

#[test]
fn missing_truncated_or_suffixed_canonical_files_never_bootstrap_an_empty_ledger() {
    let root = Directory::new(); let host = create(&root); drop(host);
    let path = root.store().join("delivery.bin"); let original = fs::read(&path).unwrap();
    for cut in [0, 7, 8, original.len() / 2, original.len() - 1] {
        fs::write(&path, &original[..cut]).unwrap();
        assert!(FileDelivery::open(root.store(), profile()).is_err());
        assert!(FileDelivery::create(root.store(), profile()).is_err());
    }
    let mut suffix = original.clone(); suffix.push(0); fs::write(&path, suffix).unwrap();
    assert!(FileDelivery::open(root.store(), profile()).is_err());
    fs::remove_file(&path).unwrap(); fs::write(root.store().join("delivery.pending"), &original).unwrap();
    assert!(FileDelivery::open(root.store(), profile()).is_err());
    assert!(!path.exists());
    fs::write(&path, original).unwrap(); assert!(FileDelivery::open(root.store(), profile()).is_ok());
}

#[test]
fn elapsed_deadline_and_suspension_survive_reopening_without_a_saved_clock_fallback() {
    let root = Directory::new(); let mut host = create(&root);
    dispatched(&mut host, 1, b"expired"); drop(host);
    let mut resumed = FileDelivery::open(root.store(), profile()).unwrap();
    assert_eq!(resumed.observe_time(resumed.revision(), ElapsedTick(0)).unwrap_err(), JournalError::Contract(Error::Stale));
    resumed.observe_time(resumed.revision(), ElapsedTick(100)).unwrap();
    assert!(resumed.propose(resumed.revision(), 2, spec(&resumed, b"late"), snapshot()).is_err());
    assert_eq!(resumed.inspect().control.ledger.charged, 16);
    let other = Directory::new(); let mut stopped = create(&other);
    stopped.propose(stopped.revision(), 1, spec(&stopped, b"x"), snapshot()).unwrap();
    stopped.review(stopped.revision(), review(1, 101, Verdict::Deny)).unwrap();
    assert!(stopped.inspect().control.suspended); drop(stopped);
    let mut stopped = FileDelivery::open(other.store(), profile()).unwrap();
    assert!(stopped.inspect().control.suspended);
    stopped.observe_time(stopped.revision(), ElapsedTick(2)).unwrap();
    assert!(stopped.propose(stopped.revision(), 2, spec(&stopped, b"x"), snapshot()).is_err());
}

#[test]
fn resource_caps_remain_bound_and_exhaustion_does_not_create_a_recovery_allowance() {
    let root = Directory::new(); let mut p = profile(); p.limits.events = 5;
    let mut host = FileDelivery::create(root.store(), p.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    dispatched(&mut host, 1, b"pending"); assert_eq!(host.revision(), 5);
    assert_eq!(host.publish(host.revision(), 1).unwrap_err(), JournalError::Contract(Error::Limit));
    assert_eq!(host.inspect().control.ledger.charged, 16); drop(host);
    assert_eq!(FileDelivery::open(root.store(), p.clone()).unwrap_err(), JournalError::Contract(Error::Limit));
    let data = FileDelivery::read_publication(root.store(), &p).unwrap();
    assert_eq!(data.control.ledger.charged, 16); assert_eq!(data.executions, 0);
    p.limits.events += 1;
    assert_eq!(FileDelivery::open(root.store(), p).unwrap_err(), JournalError::Contract(Error::Binding));
}

#[test]
fn private_new_paths_and_foreign_same_named_permits_keep_independent_domains_distinct() {
    let first = Directory::new(); let second = Directory::new();
    let mut a = create(&first); let mut b = create(&second);
    let (action, key) = approved(&mut a, 1, b"same"); let _ = approved(&mut b, 1, b"same");
    assert_eq!(b.dispatch(b.revision(), &key, &action, snapshot()).unwrap_err(), JournalError::Contract(Error::Binding));
    assert_eq!(fs::metadata(first.store()).unwrap().permissions().mode() & 0o077, 0);
    assert_eq!(fs::metadata(first.store().join("delivery.bin")).unwrap().permissions().mode() & 0o077, 0);
    assert_eq!(b.inspect().control.ledger.reserved, 16);
    a.dispatch(a.revision(), &key, &action, snapshot()).unwrap(); a.publish(a.revision(), 1).unwrap();
    assert_eq!(a.inspect().executions, 1); assert_eq!(b.inspect().executions, 0);
}
