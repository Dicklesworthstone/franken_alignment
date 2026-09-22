//! Targeted, receipt-backed cancellation without stopping unrelated requests.

use super::super::{FileRequestDisposition, FileRequestStatus};
use super::super::super::{Event, FileDelivery, JournalError};
use crate::action::{ActionState, ElapsedTick};

impl FileDelivery {
    /// Settle a cancellation by durable request identity in one replacement.
    ///
    /// Unlike `cancel_request`, this operation can resolve already dispatched
    /// work. Before dispatch it uses the original reservation cancellation.
    /// After dispatch it asks the original endpoint to atomically seal the key
    /// and accepts its receipt. A real execution wins the race: its publication
    /// remains visible and charged. A nonexecution receipt both releases the
    /// charge and prevents a delayed envelope from executing afterward.
    ///
    /// The returned status distinguishes cancellation before dispatch from
    /// confirmed execution and confirmed nonexecution. `request_resolution`
    /// exposes the retained endpoint outcome where one exists. This is a trusted
    /// supervisor operation, not an actor-facing permission or a new permit.
    /// Unrelated requests are not cancelled, fenced, swept or resent.
    ///
    /// The elapsed observation and cancellation/seal are original events applied
    /// in a private RAM projection, then committed together. An expired retention
    /// interval, stale clock/predecessor or capacity failure leaves both clock
    /// and ledger unchanged; uncertainty is never converted into a refund.
    /// Storage failure poisons the owner and returns no candidate status.
    ///
    /// Already terminal requests return their historical status without writing,
    /// even with a stale predecessor/observation. This does not advance the clock.
    /// A faulted owner refuses even this historical path. No old process-local
    /// permit is needed, so the same request can be settled after exclusive reopen.
    ///
    /// This uses ordinary Time-event capacity; it does not promise progress after
    /// that capacity is exhausted. `stop_and_drain`/`open_stopped` remain the
    /// terminal-reserve paths for shutting down the entire owner. As with the
    /// other atomic request APIs, the sole effect sink is this journal, not an
    /// independently visible remote provider or arbitrary filesystem adapter.
    pub fn cancel_and_resolve_request(
        &mut self,
        revision: u64,
        request: u64,
        observed_tick: ElapsedTick,
    ) -> Result<FileRequestStatus, JournalError> {
        let status = self.request_status(request)?;
        let FileRequestDisposition::Admitted { attempt, stage } = status.disposition else {
            return Ok(status);
        };
        let cancellation = match stage {
            ActionState::Proposed | ActionState::Prepared | ActionState::Reviewing
            | ActionState::Authorized => Event::Cancel(attempt),
            ActionState::Dispatching | ActionState::Unknown
            | ActionState::IrrecoverablyUnknown => Event::Seal(attempt),
            ActionState::Confirmed | ActionState::ConfirmedNotExecuted
            | ActionState::Denied | ActionState::Cancelled => return Ok(status),
        };
        self.commit_request_events(revision, &[Event::Time(observed_tick), cancellation])?;
        self.request_status(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::super::super::{
        FileDeliveryProfile, FilePermit, JournalIo, JournalLimits, ReferenceBallot, ReferenceReview,
    };
    use crate::action::{ActionSpec, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
    use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
    use crate::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
    use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
    use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
    use crate::reducer::Caps;
    use crate::round::Verdict;
    use crate::{Error, Snapshot};
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let root = std::env::temp_dir().join(format!(
                "fa-request-cancellation-{}-{time}-{}",
                std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            std::fs::create_dir(&root).unwrap();
            Self(root)
        }
        fn store(&self) -> PathBuf { self.0.join("publication") }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            if let Err(error) = std::fs::remove_dir_all(&self.0) {
                eprintln!("request cancellation test cleanup: {error}");
            }
        }
    }

    fn profile() -> FileDeliveryProfile {
        let target = ResolvedTarget {
            adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1,
        };
        FileDeliveryProfile {
            scope: Scope {
                tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect,
            },
            total: 100,
            max_attempts: 8,
            actor: ActorState::new(RestartProfile {
                id: 1, generation: 1, host_generation: 1, model_generation: 1,
                tokenizer_generation: 1, state_schema_generation: 1,
                grade: RestartGrade::FunctionalRestart,
            }, vec![1], vec![2], vec![3], 1).unwrap(),
            suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::ExactValue {
                key: 7, value: b"ok".to_vec(),
            }]).unwrap(),
            congress: CongressPolicy {
                generation: 1,
                members: BTreeMap::from([("reviewer".to_owned(), MemberPolicy {
                    cohort: "reference".to_owned(), weight: 1,
                })]),
                caps: Caps { per_member: 1, per_cohort: 1 },
                continue_minimum: 1, continue_hold_maximum: 0, narrow_at: 2,
                suspend_at: 3, minimum_members: 1, minimum_cohorts: 1,
            },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99,
            limits: JournalLimits::default(),
        }
    }

    fn snapshot() -> Snapshot {
        Snapshot {
            semantic_epoch: 1, complete: true,
            values: BTreeMap::from([(7, b"ok".to_vec())]),
        }
    }

    fn authorize(host: &mut FileDelivery, request: u64) -> (FilePermit, FrozenAction) {
        let spec = ActionSpec {
            version: VERSION, scope: profile().scope, target: Some(host.inspect().target),
            payload: b"visible".to_vec(), required_witnesses: Vec::new(),
            policy_epoch: host.inspect().control.ledger.epoch,
            deadline: ElapsedTick(100), units: 16,
        };
        let status = host.submit_request(host.revision(), request, spec, snapshot()).unwrap();
        let FileRequestDisposition::Admitted { attempt, .. } = status.disposition else {
            panic!("positive request admission");
        };
        let action = host.request_action(request).unwrap().clone();
        host.review(host.revision(), ReferenceReview {
            attempt, round: attempt + 100, evidence_root: [9; 32], snapshot: snapshot(),
            ballots: BTreeMap::from([("reviewer".to_owned(), ReferenceBallot {
                verdict: Verdict::Allow, salt: b"reference-salt".to_vec(),
            })]),
        }).unwrap();
        let key = host.authorize(host.revision(), attempt, snapshot()).unwrap();
        (key, action)
    }

    fn prepared(root: &Directory, p: &FileDeliveryProfile) -> (FileDelivery, FilePermit, FrozenAction) {
        let mut host = FileDelivery::create(root.store(), p.clone()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let (key, action) = authorize(&mut host, 700);
        (host, key, action)
    }

    fn stage(status: FileRequestStatus) -> ActionState {
        let FileRequestDisposition::Admitted { stage, .. } = status.disposition else {
            panic!("admitted request");
        };
        stage
    }

    #[test]
    fn cancels_authorized_reservation_and_terminal_retry_is_read_only() {
        let root = Directory::new();
        let p = profile();
        let (mut host, key, action) = prepared(&root, &p);
        let revision = host.revision();
        let status = host.cancel_and_resolve_request(revision, 700, ElapsedTick(2)).unwrap();
        assert_eq!(stage(status), ActionState::Cancelled);
        assert_eq!(host.revision(), revision + 2);
        assert_eq!(host.request_resolution(700), Ok(None));
        let after = host.inspect();
        assert_eq!(after.control.ledger.available, 100);
        assert_eq!(after.control.ledger.reserved, 0);
        assert_eq!(after.control.ledger.charged, 0);
        assert_eq!(after.executions, 0);
        assert_eq!(host.cancel_and_resolve_request(0, 700, ElapsedTick(0)), Ok(status));
        assert_eq!(host.inspect(), after);
        assert!(host.dispatch(host.revision(), &key, &action, snapshot()).is_err());
        assert_eq!(FileDelivery::read_publication(root.store(), &p).unwrap(), after);
    }

    #[test]
    fn seals_dispatched_key_and_delayed_publication_cannot_execute() {
        let root = Directory::new();
        let p = profile();
        let (mut host, key, action) = prepared(&root, &p);
        host.dispatch(host.revision(), &key, &action, snapshot()).unwrap();
        assert_eq!(host.inspect().control.ledger.charged, 16);
        let status = host.cancel_and_resolve_request(host.revision(), 700, ElapsedTick(2)).unwrap();
        let outcome = EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed };
        assert_eq!(stage(status), ActionState::ConfirmedNotExecuted);
        assert_eq!(host.request_resolution(700), Ok(Some(outcome)));
        assert_eq!(host.inspect().control.ledger.available, 100);
        assert_eq!(host.inspect().control.ledger.charged, 0);
        assert_eq!(host.publish(host.revision(), key.attempt()), Ok(outcome));
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().payload, b"initial");
        drop(host);
        let recovered = FileDelivery::open(root.store(), p).unwrap();
        assert_eq!(recovered.request_resolution(700), Ok(Some(outcome)));
        assert_eq!(recovered.inspect().control.ledger.available, 100);
    }

    #[test]
    fn execution_wins_cancellation_even_when_acknowledgment_was_lost() {
        let root = Directory::new();
        let (mut host, key, action) = prepared(&root, &profile());
        host.dispatch(host.revision(), &key, &action, snapshot()).unwrap();
        let outcome = EndpointOutcome::Executed { resulting_version: 2 };
        assert_eq!(host.publish(host.revision(), key.attempt()), Ok(outcome));
        assert_eq!(host.request_resolution(700), Ok(None));
        let status = host.cancel_and_resolve_request(host.revision(), 700, ElapsedTick(2)).unwrap();
        assert_eq!(stage(status), ActionState::Confirmed);
        assert_eq!(host.request_resolution(700), Ok(Some(outcome)));
        assert_eq!(host.inspect().executions, 1);
        assert_eq!(host.inspect().payload, b"visible");
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(host.inspect().control.ledger.available, 84);
    }

    #[test]
    fn cancellation_does_not_touch_another_requests_permit_or_reservation() {
        let root = Directory::new();
        let (mut host, first, action) = prepared(&root, &profile());
        let (second, second_action) = authorize(&mut host, 701);
        host.dispatch(host.revision(), &first, &action, snapshot()).unwrap();
        host.cancel_and_resolve_request(host.revision(), 700, ElapsedTick(2)).unwrap();
        assert_eq!(stage(host.request_status(701).unwrap()), ActionState::Authorized);
        assert_eq!(host.inspect().control.ledger.reserved, 16);
        assert_eq!(host.inspect().control.ledger.charged, 0);
        assert_eq!(host.commit_request_publication(host.revision(), 701, &second, &second_action, snapshot()),
            Ok(EndpointOutcome::Executed { resulting_version: 2 }));
    }

    #[test]
    fn stale_missing_and_expired_evidence_leave_clock_ledger_and_disk_unchanged() {
        let root = Directory::new();
        let p = profile();
        let (mut host, key, action) = prepared(&root, &p);
        host.dispatch(host.revision(), &key, &action, snapshot()).unwrap();
        let before = host.inspect();
        for (revision, request, tick, error) in [
            (host.revision() - 1, 700, 2, Error::Stale),
            (host.revision(), 700, 0, Error::Stale),
            (host.revision(), 999, 2, Error::Missing),
            (host.revision(), 700, 1002, Error::Stale),
        ] {
            assert_eq!(host.cancel_and_resolve_request(revision, request, ElapsedTick(tick)),
                Err(JournalError::Contract(error)));
            assert_eq!(host.inspect(), before);
            assert_eq!(FileDelivery::read_publication(root.store(), &p).unwrap(), before);
            assert!(host.storage_failure().is_none());
        }
        // A rejected future observation was not committed by the first event.
        assert!(host.cancel_and_resolve_request(host.revision(), 700, ElapsedTick(2)).is_ok());
    }

    #[test]
    fn both_event_slots_are_preflighted_before_any_refund() {
        for limit in [6, 7] {
            let root = Directory::new();
            let mut p = profile();
            p.limits.events = limit;
            let (mut host, key, action) = prepared(&root, &p);
            host.dispatch(host.revision(), &key, &action, snapshot()).unwrap();
            assert_eq!(host.revision(), 5);
            let before = host.inspect();
            let result = host.cancel_and_resolve_request(host.revision(), 700, ElapsedTick(2));
            if limit == 6 {
                assert_eq!(result, Err(JournalError::Contract(Error::Limit)));
                assert_eq!(host.inspect(), before);
                assert_eq!(FileDelivery::read_publication(root.store(), &p).unwrap(), before);
            } else {
                assert_eq!(stage(result.unwrap()), ActionState::ConfirmedNotExecuted);
                assert_eq!(host.revision(), 7);
                assert_eq!(host.inspect().control.ledger.charged, 0);
            }
        }
    }

    #[test]
    fn every_storage_barrier_exposes_only_old_or_fully_sealed_state() {
        for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
            JournalIo::Rename, JournalIo::DirectorySync]
        {
            let root = Directory::new();
            let p = profile();
            let (mut host, key, action) = prepared(&root, &p);
            host.dispatch(host.revision(), &key, &action, snapshot()).unwrap();
            let before = host.inspect();
            host.store.fail_once(barrier);
            assert!(matches!(host.cancel_and_resolve_request(host.revision(), 700, ElapsedTick(2)),
                Err(JournalError::Io(_))));
            assert_eq!(host.inspect(), before);
            assert_eq!(host.cancel_and_resolve_request(host.revision(), 700, ElapsedTick(2)),
                Err(JournalError::Unavailable));
            let disk = FileDelivery::read_publication(root.store(), &p).unwrap();
            let replaced = barrier == JournalIo::DirectorySync;
            assert_eq!(disk.executions, 0);
            assert_eq!(disk.control.ledger.charged, if replaced { 0 } else { 16 });
            assert_eq!(disk.control.ledger.available, if replaced { 100 } else { 84 });
            assert_eq!(disk.revision, before.revision + if replaced { 2 } else { 0 });
            drop(host);
            let mut recovered = FileDelivery::open(root.store(), p).unwrap();
            // An old process-local permit is unnecessary; settle the durable ID.
            let status = recovered.cancel_and_resolve_request(recovered.revision(), 700, ElapsedTick(3)).unwrap();
            assert_eq!(stage(status), ActionState::ConfirmedNotExecuted);
            assert_eq!(recovered.inspect().executions, 0);
            assert_eq!(recovered.inspect().control.ledger.charged, 0);
        }
    }
}
