//! Included below the original storage fixtures; reuse their real I/O barriers.
use super::*;
use crate::action::consequence::delivery::persistent::governance::PolicyUpdate;

#[test]
fn policy_faults_return_no_candidate_refunds_and_recover_the_actual_canonical_generation() {
    for barrier in BARRIERS {
        let root = Directory::new(); let mut host = create(&root);
        dispatch(&mut host, 1, b"executed");
        host.publish(host.revision(), 1).unwrap();
        let spec = ActionSpec { version: VERSION, scope: profile().scope,
            target: Some(host.inspect().target), payload: b"reserved".to_vec(), required_witnesses: Vec::new(),
            policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 };
        host.propose(host.revision(), 2, spec, snapshot()).unwrap();
        host.review(host.revision(), ReferenceReview { attempt: 2, round: 102, evidence_root: [9; 32],
            snapshot: snapshot(), ballots: BTreeMap::from([("reviewer".to_owned(),
                ReferenceBallot { verdict: Verdict::Allow, salt: b"salt".to_vec() })]) }).unwrap();
        let _reserved = host.authorize(host.revision(), 2, snapshot()).unwrap();
        let before = host.inspect();
        let update = PolicyUpdate::new(1, before.control.sequence, before.control.ledger.epoch,
            Policy::new(2, vec![Predicate::PayloadAtMost(128)]).unwrap()).unwrap();
        host.store.fail_once(barrier);
        let error = host.replace_policy(host.revision(), &update).unwrap_err();
        let JournalError::Io(failure) = error else { panic!("expected storage failure"); };
        assert_eq!(failure.operation, barrier);
        assert_eq!(host.inspect(), before);
        assert_eq!(host.current_policy(), Err(JournalError::Unavailable));
        assert_eq!(host.replace_policy(0, &update), Err(JournalError::Unavailable));
        assert_eq!(host.policy_update_receipt(1), Err(JournalError::Unavailable));
        let visible = FileDelivery::read_publication(root.store(), &profile()).unwrap();
        let replaced = barrier == JournalIo::DirectorySync;
        assert_eq!(visible.control.ledger.epoch, before.control.ledger.epoch + u64::from(replaced));
        assert_eq!(visible.control.ledger.reserved, if replaced { 0 } else { 16 });
        assert_eq!(visible.control.ledger.charged, 16);
        assert_eq!(visible.executions, 1);
        drop(host);
        let mut recovered = FileDelivery::open(root.store(), profile()).unwrap();
        assert_eq!(recovered.current_policy().unwrap().generation(), if replaced { 2 } else { 1 });
        let cut = recovered.inspect();
        if replaced {
            let receipt = recovered.replace_policy(0, &update).unwrap();
            assert_eq!(receipt.change().refunded_units, 16);
            assert_eq!(recovered.inspect(), cut);
        } else {
            assert_eq!(recovered.policy_update_receipt(1), Err(JournalError::Contract(Error::Missing)));
            assert_eq!(recovered.replace_policy(recovered.revision(), &update), Err(JournalError::Contract(Error::Stale)));
        }
        recovered.observe_time(recovered.revision(), ElapsedTick(2)).unwrap();
        assert!(matches!(recovered.reconcile(recovered.revision(), 1).unwrap(),
            Reconciliation::Resolved(EndpointOutcome::Executed { .. })));
        assert_eq!(recovered.inspect().control.ledger.available, 84);
        assert_eq!(recovered.inspect().control.ledger.charged, 16);
        assert_eq!(recovered.inspect().executions, 1);
    }
}
