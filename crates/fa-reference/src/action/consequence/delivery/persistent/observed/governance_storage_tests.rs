//! Included under the original observed tests, retaining their native fixtures.
use super::*;
use crate::action::consequence::delivery::persistent::governance::PolicyUpdate;

#[test]
fn policy_and_human_withdrawal_share_one_acknowledged_journal_replacement() {
    for barrier in BARRIERS {
        let root = Directory::new(); let (mut host, reviewer) = create(&root);
        let (_, _, _, request) = prepared(&mut host);
        let revision = host.revision();
        let _human = reviewer.approve(&mut host, revision, &request).unwrap();
        let before = host.inspect();
        let update = PolicyUpdate::new(1, before.control.sequence, before.control.ledger.epoch,
            Policy::new(2, vec![Predicate::PayloadAtMost(128)]).unwrap()).unwrap();
        host.store.fail_once(barrier);
        let error = host.replace_policy(host.revision(), &update).unwrap_err();
        check_failure(error, barrier);
        assert_eq!(host.inspect(), before);
        assert_eq!(host.current_policy(), Err(JournalError::Unavailable));
        assert_eq!(host.replace_policy(0, &update), Err(JournalError::Unavailable));
        let disk = canonical(&host);
        let replaced = barrier == JournalIo::DirectorySync;
        assert_eq!(disk.policy_updates.current(&profile().delivery.policy).generation(), if replaced { 2 } else { 1 });
        assert_eq!(disk.broker.human_status(1001).unwrap().disposition,
            if replaced { HumanDisposition::Revoked } else { HumanDisposition::Approved });
        assert_eq!(disk.broker.inspect().ledger.reserved, if replaced { 0 } else { 16 });
        drop(host);
        let (mut recovered, _) = FileOversight::open(root.store(), profile()).unwrap();
        assert_eq!(recovered.human_status(1001).unwrap().disposition, HumanDisposition::Revoked);
        assert_eq!(recovered.inspect().control.ledger.available, 100);
        assert_eq!(recovered.current_policy().unwrap().generation(), if replaced { 2 } else { 1 });
        let before = recovered.inspect();
        if replaced {
            let receipt = recovered.replace_policy(0, &update).unwrap();
            assert_eq!(receipt.change().refunded_units, 16);
            assert_eq!(recovered.inspect(), before);
        } else {
            assert_eq!(recovered.policy_update_receipt(1), Err(JournalError::Contract(Error::Missing)));
            assert_eq!(recovered.replace_policy(recovered.revision(), &update), Err(JournalError::Contract(Error::Stale)));
        }
        assert_eq!(recovered.inspect().executions, 0);
    }
}
