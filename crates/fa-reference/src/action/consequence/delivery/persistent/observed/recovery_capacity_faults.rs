//! Exact original key withdrawal remains atomic with a reserve-backed stop.
use super::*;
use crate::action::consequence::delivery::persistent::RecoveryReserve;

#[test]
fn stop_faults_at_capacity_recover_human_withdrawal_and_preserve_sent_charges() {
    for stage in BARRIERS {
        for sent in [false, true] {
            let root = Directory::new();
            let mut p = profile(); p.delivery.limits.events = 32;
            let (mut host, reviewer) = FileOversight::create(root.store(), p.clone()).unwrap();
            host.enable_recovery_reserve(0, RecoveryReserve::terminal()).unwrap();
            host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
            let (action, input, automatic, evidence) = prepared(&mut host);
            let revision = host.revision();
            let human = reviewer.approve(&mut host, revision, &evidence).unwrap();
            if sent { host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap(); }
            while host.revision() < 29 { host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); }
            let before = host.inspect();
            let request = StopRequest { operation: 1, expected_control_sequence: before.control.sequence,
                expected_authority_epoch: before.control.ledger.epoch };
            host.store.fail_once(stage);
            check_failure(host.request_stop(host.revision(), request).unwrap_err(), stage);
            assert_eq!(host.inspect(), before);
            assert_eq!(host.journal_capacity(), Err(JournalError::Unavailable));
            let disk = canonical(&host);
            assert_eq!(disk.broker.stop_receipt().is_some(), stage == JournalIo::DirectorySync);
            let expected = if sent { HumanDisposition::Consumed }
                else if stage == JournalIo::DirectorySync { HumanDisposition::Revoked }
                else { HumanDisposition::Approved };
            assert_eq!(disk.broker.human_status(1001).unwrap().disposition, expected);
            drop(host); drop(reviewer);
            let (mut host, _) = FileOversight::open(root.store(), p).unwrap();
            if host.inspect().stop.is_none() {
                let control = host.inspect().control;
                host.request_stop(host.revision(), StopRequest { operation: 2,
                    expected_control_sequence: control.sequence, expected_authority_epoch: control.ledger.epoch }).unwrap();
            }
            assert!(host.progress_stop(host.revision(), ElapsedTick(2)).unwrap().progress.drained());
            assert_eq!(host.revision(), 32);
            assert_eq!(host.inspect().control.ledger.available, 100);
            assert_eq!(host.inspect().executions, 0);
            assert_eq!(host.human_status(1001).unwrap().disposition,
                if sent { HumanDisposition::Consumed } else { HumanDisposition::Revoked });
        }
    }
}
