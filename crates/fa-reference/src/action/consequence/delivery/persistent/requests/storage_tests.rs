//! Reuse the original actual-file fault barriers, including successful rename.
use super::*;
use crate::action::consequence::delivery::persistent::requests::FileRequestDisposition;

#[test]
fn failed_request_commit_never_returns_a_candidate_and_recovery_preserves_visible_identity() {
    for stage in BARRIERS {
        for refused in [false, true] {
            let root = Directory::new(); let mut host = create(&root);
            let mut spec = ActionSpec { version: VERSION, scope: profile().scope,
                target: Some(host.inspect().target), payload: b"request".to_vec(), required_witnesses: Vec::new(),
                policy_epoch: 0, deadline: ElapsedTick(100), units: 16 };
            if refused { spec.target.as_mut().unwrap().object += 1; }
            let before = host.inspect(); host.store.fail_once(stage);
            let failed = host.submit_request(host.revision(), 10, spec.clone(), snapshot());
            let Err(JournalError::Io(failure)) = failed else { panic!("expected storage refusal"); };
            assert_eq!(failure.operation, stage);
            assert_eq!(host.inspect(), before);
            assert_eq!(host.retained_requests(), 0);
            assert_eq!(host.request_status(10), Err(JournalError::Unavailable));
            assert_eq!(host.submit_request(0, 10, spec.clone(), Snapshot::default()), Err(JournalError::Unavailable));
            drop(host);
            let mut recovered = FileDelivery::open(root.store(), profile()).unwrap();
            let replaced = stage == JournalIo::DirectorySync;
            assert_eq!(recovered.retained_requests(), usize::from(replaced));
            if replaced {
                let status = recovered.request_status(10).unwrap();
                let expected = if refused { FileRequestDisposition::NotAdmitted(Error::Binding) }
                    else { FileRequestDisposition::Admitted { attempt: 1, stage: ActionState::Cancelled } };
                assert_eq!(status.disposition, expected);
                let revision = recovered.revision();
                // A successful rename is recovered even though the caller saw
                // no acknowledgment and supplied its old predecessor/snapshot.
                assert_eq!(recovered.submit_request(0, 10, spec, Snapshot::default()).unwrap(), status);
                assert_eq!(recovered.revision(), revision);
            } else {
                assert_eq!(recovered.request_status(10), Err(JournalError::Contract(Error::Missing)));
                recovered.observe_time(recovered.revision(), ElapsedTick(2)).unwrap();
                spec.policy_epoch = recovered.inspect().control.ledger.epoch;
                let status = recovered.submit_request(recovered.revision(), 10, spec, snapshot()).unwrap();
                assert_eq!(matches!(status.disposition, FileRequestDisposition::NotAdmitted(_)), refused);
                assert_eq!(recovered.retained_requests(), 1);
            }
            assert_eq!(recovered.inspect().control.ledger.available, 100);
            assert_eq!(recovered.inspect().control.ledger.charged, 0);
            assert_eq!(recovered.inspect().executions, 0);
        }
    }
}
