//! Durable request identity over the original publication authority. Journal
//! replay recomputes admission, never imports a caller-asserted outcome or permit.

pub mod actor;

use super::{Event, FileDelivery, FrozenAction, JournalError, Machine, Transition};
use crate::action::{ActionSpec, ActionState, Scope};
use crate::action::consequence::gate::ControlInspection;
use crate::action::consequence::gate::containment::session::policy::controller::Proposal;
use crate::{Error, Snapshot};
use std::collections::BTreeMap;

pub const MAX_FILE_REQUESTS: usize = 128;
pub const MAX_FILE_REQUEST_BYTES: usize = 2 * 1024 * 1024;

/// Supervisor-only disposition. The actor gateway publishes a restricted
/// Knowledge projection, not the internal attempt ID or admission diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileRequestDisposition {
    NotAdmitted(Error),
    Admitted { attempt: u64, stage: ActionState },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileRequestStatus {
    pub request: u64,
    /// Request-local projection revision, not the shared journal sequence.
    pub generation: u64,
    pub disposition: FileRequestDisposition,
}

struct RequestRecord {
    spec: ActionSpec,
    allocated_attempt: u64,
    status: FileRequestStatus,
}
#[derive(Default)]
pub(super) struct RequestBook {
    records: BTreeMap<u64, RequestRecord>,
    bytes: usize,
}

/// Private preflight data. It cannot admit an effect; only an ORIGINAL broker's
/// proposal result completes it. Both durable profiles use this same request book.
pub(super) struct PreparedRequest {
    request: u64,
    attempt: u64,
    spec: ActionSpec,
    bytes: usize,
}
impl PreparedRequest {
    pub(super) fn attempt(&self) -> u64 { self.attempt }
}
impl RequestBook {
    pub(super) fn len(&self) -> usize { self.records.len() }
    pub(super) fn bytes(&self) -> usize { self.bytes }
    pub(super) fn status(&self, request: u64) -> Result<FileRequestStatus, Error> {
        Ok(self.records.get(&request).ok_or(Error::Missing)?.status)
    }
    pub(super) fn retry(&self, request: u64, spec: &ActionSpec) -> Result<Option<FileRequestStatus>, Error> {
        match self.records.get(&request) {
            Some(record) if &record.spec == spec => Ok(Some(record.status)),
            Some(_) => Err(Error::Binding),
            None => Ok(None),
        }
    }

    pub(super) fn prepare(&self, request: u64, spec: &ActionSpec, scope: Scope,
        inspection: &ControlInspection, stopping: bool) -> Result<PreparedRequest, Error>
    {
        if request == 0 { return Err(Error::InvalidInput); }
        if self.records.contains_key(&request) { return Err(Error::Duplicate); }
        if inspection.suspended || stopping { return Err(Error::WrongState); }
        if !spec.required_witnesses.is_empty() { return Err(Error::InvalidInput); }
        // Structure is not policy approval. Policy refusals are retained by
        // finish rather than retried under this key with a different snapshot.
        FrozenAction::freeze(spec.clone())?;
        if spec.scope != scope { return Err(Error::Binding); }
        if self.records.len() >= MAX_FILE_REQUESTS { return Err(Error::Limit); }
        let bytes = self.bytes.checked_add(spec.payload.len()).ok_or(Error::Limit)?;
        if bytes > MAX_FILE_REQUEST_BYTES { return Err(Error::Limit); }
        let attempt = self.next_attempt(inspection)?;
        Ok(PreparedRequest { request, attempt, spec: spec.clone(), bytes })
    }

    /// The SAME allocation rule can be inspected before a payload exists. This
    /// does not reserve an ID or admit a request; refused admissions still count.
    pub(super) fn next_attempt(&self, inspection: &ControlInspection) -> Result<u64, Error> {
        let maximum = inspection.ledger.stages.keys().copied()
            .chain(self.records.values().map(|row| row.allocated_attempt)).max().unwrap_or(0);
        maximum.checked_add(1).ok_or(Error::Overflow)
    }

    /// No policy evaluator or outcome reducer lives here. The original proposal
    /// result supplies both the frozen action and actual initial ledger stage.
    pub(super) fn finish(&mut self, prepared: PreparedRequest, result: Result<Proposal, Error>)
        -> Option<(u64, FrozenAction)>
    {
        let PreparedRequest { request, attempt, spec, bytes } = prepared;
        let (disposition, action) = match result {
            Ok(proposal) => (FileRequestDisposition::Admitted { attempt, stage: proposal.state },
                Some((attempt, proposal.action))),
            Err(error) => (FileRequestDisposition::NotAdmitted(error), None),
        };
        let generation = u64::from(!matches!(disposition,
            FileRequestDisposition::Admitted { stage: ActionState::Proposed | ActionState::Prepared
                | ActionState::Reviewing | ActionState::Authorized, .. }));
        self.records.insert(request, RequestRecord { spec, allocated_attempt: attempt,
            status: FileRequestStatus { request, generation, disposition } });
        self.bytes = bytes;
        action
    }

    /// A projection of original ledger stages, never a second outcome ledger.
    /// Private review/reservation transitions remain the same visible phase.
    pub(super) fn refresh(&mut self, inspection: &ControlInspection) -> Result<(), Error> {
        for record in self.records.values_mut() {
            if let FileRequestDisposition::Admitted { attempt, stage } = record.status.disposition {
                let current = *inspection.ledger.stages.get(&attempt).ok_or(Error::Missing)?;
                if current != stage {
                    if projection_class(current) != projection_class(stage) {
                        record.status.generation = record.status.generation.checked_add(1).ok_or(Error::Overflow)?;
                    }
                    record.status.disposition = FileRequestDisposition::Admitted { attempt, stage: current };
                }
            }
        }
        Ok(())
    }
}

impl FileDelivery {
    /// Exactly one original admission per key, including a recorded refusal.
    /// Exact retry returns its CURRENT durable disposition before checking the
    /// supplied predecessor, clock or snapshot; it performs no new admission.
    /// Changed action fields conflict. A storage fault never returns old state
    /// as a successful retry while a newer journal may already be visible.
    pub fn submit_request(&mut self, revision: u64, request: u64, spec: ActionSpec,
        snapshot: Snapshot) -> Result<FileRequestStatus, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if let Some(status) = self.machine.requests.retry(request, &spec)? { return Ok(status); }
        self.transact(revision, Event::SubmitRequest(request, spec, snapshot))?;
        self.request_status(request)
    }

    pub fn request_status(&self, request: u64) -> Result<FileRequestStatus, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.requests.status(request)?)
    }

    /// Frozen data for the trusted review/authorization owner. No key or
    /// sendable envelope is reconstructed by looking up this action.
    pub fn request_action(&self, request: u64) -> Result<&FrozenAction, JournalError> {
        let status = self.request_status(request)?;
        match status.disposition {
            FileRequestDisposition::NotAdmitted(_) => Err(Error::WrongState.into()),
            FileRequestDisposition::Admitted { attempt, .. } => {
                self.machine.actions.get(&attempt).ok_or_else(|| Error::Missing.into())
            }
        }
    }

    /// A cancellation request can release only an original undispatched
    /// reservation. Once sent, cancellation is an idempotent no-op: it does not
    /// assert nonexecution, release a charge, delete a key or create a resend.
    pub fn cancel_request(&mut self, revision: u64, request: u64) -> Result<(), JournalError> {
        let status = self.request_status(request)?;
        if let FileRequestDisposition::Admitted { attempt, stage } = status.disposition {
            if matches!(stage, ActionState::Proposed | ActionState::Prepared
                | ActionState::Reviewing | ActionState::Authorized)
            { return self.cancel(revision, attempt); }
        }
        Ok(())
    }

    pub fn retained_requests(&self) -> usize { self.machine.requests.len() }
    pub fn retained_request_bytes(&self) -> usize { self.machine.requests.bytes() }

    /// Complete one ALREADY authorized request with one durable replacement.
    ///
    /// The original dispatch, endpoint publication and receipt reconciliation
    /// all execute in a private RAM projection before the sole journal sink is
    /// replaced. No intermediate candidate payload, charge or refund is exposed.
    /// The three original events remain in the unchanged canonical format, and
    /// every prefix still passes journal/recovery-reserve admission. The journal
    /// revision advances by three, not by an invented batch event.
    ///
    /// This consumes the existing permit's dispatch opportunity: it does not
    /// authorize, review, mint a permit, invent a clock or accept a raw outcome.
    /// Exact same-owner retries of a terminal request return its retained
    /// historical receipt without another write, even with a stale predecessor.
    /// A reopened owner rejects old process-local keys. A storage error returns
    /// no candidate receipt and poisons the owner until exclusive recovery.
    ///
    /// This optimization is ONLY valid for FileDelivery's journal-as-publication
    /// sink. It must not be generalized to remote or independently visible
    /// filesystem adapters by replaying their effects inside a transaction.
    pub fn commit_request_publication(
        &mut self,
        revision: u64,
        request: u64,
        key: &super::FilePermit,
        exact_action: &FrozenAction,
        snapshot: Snapshot,
    ) -> Result<super::EndpointOutcome, JournalError> {
        if self.fault.is_some() {
            return Err(JournalError::Unavailable);
        }
        if !std::rc::Rc::ptr_eq(&self.issuer, &key.issuer)
            || self.machine.actions.get(&key.attempt) != Some(exact_action)
        {
            return Err(Error::Binding.into());
        }
        let status = self.request_status(request)?;
        let FileRequestDisposition::Admitted { attempt, stage } = status.disposition else {
            return Err(Error::WrongState.into());
        };
        if attempt != key.attempt {
            return Err(Error::Binding.into());
        }
        if matches!(stage, ActionState::Confirmed | ActionState::ConfirmedNotExecuted) {
            // This is the broker's already accepted receipt, not a fresh status
            // observation or renewed permission. Never reconcile it a second time.
            let receipt = self.machine.broker.records.get(&attempt)
                .and_then(|record| record.resolution.as_ref())
                .ok_or(Error::Incomplete)?;
            return Ok(receipt.outcome());
        }
        if stage != ActionState::Authorized {
            return Err(Error::WrongState.into());
        }
        let mut transitions = self.commit_request_events(revision, &[
            Event::Dispatch(attempt, snapshot),
            Event::Publish(attempt),
            Event::Reconcile(attempt),
        ])?;
        match transitions.pop() {
            Some(Transition::Reconciled(super::Reconciliation::Resolved(outcome))) => Ok(outcome),
            _ => unreachable!("publication followed by original receipt reconciliation"),
        }
    }

    /// Fixed, internal sequences only: no caller can import arbitrary events,
    /// saved rights, or a asserted terminal state through this path.
    fn commit_request_events(
        &mut self,
        revision: u64,
        next: &[Event],
    ) -> Result<Vec<Transition>, JournalError> {
        if self.fault.is_some() {
            return Err(JournalError::Unavailable);
        }
        if revision != self.revision() {
            return Err(Error::Stale.into());
        }
        if next.is_empty() || next.len() > 3 {
            return Err(Error::InvalidInput.into());
        }
        let count = self.events.len().checked_add(next.len()).ok_or(Error::Overflow)?;
        if count > self.profile.limits.events {
            return Err(Error::Limit.into());
        }
        // Validate the first new input before cloning any history. Every next
        // prefix uses the existing codec, including its recovery-capacity law.
        let mut bytes = super::codec::encode_appended(
            &self.profile, self.store.identity(), &self.events, &next[0],
        )?;
        let mut history = Vec::new();
        history.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        history.extend(self.events.iter().cloned());
        for (index, event) in next.iter().enumerate() {
            if index != 0 {
                bytes = super::codec::encode_appended(
                    &self.profile, self.store.identity(), &history, event,
                )?;
            }
            history.push(event.clone());
        }
        let mut transitions = Vec::new();
        transitions.try_reserve_exact(next.len()).map_err(|_| Error::Limit)?;
        let mut candidate = Machine::replay(&self.profile, &self.events)?;
        for event in next {
            transitions.push(candidate.apply(event)?);
        }
        if let Err(error) = self.store.replace(&bytes) {
            self.fault = Some(match &error {
                JournalError::Io(failure) => failure.clone(),
                _ => super::JournalFailure {
                    operation: super::JournalIo::Stage,
                    kind: std::io::ErrorKind::Other,
                    replacement_may_be_visible: true,
                },
            });
            return Err(error);
        }
        // Everything fallible finished before publication. Install by moving
        // the complete original-event history and its original-reducer result.
        self.events = history;
        self.machine = candidate;
        Ok(transitions)
    }
}

impl Machine {
    pub(super) fn apply_request(&mut self, request: u64, spec: &ActionSpec,
        snapshot: &Snapshot) -> Result<Transition, Error>
    {
        let prepared = self.requests.prepare(request, spec, self.broker.scope,
            &self.broker.inspect(), self.broker.stop_receipt().is_some())?;
        let result = self.broker.propose(prepared.attempt(), spec.clone(), snapshot);
        if let Some((attempt, action)) = self.requests.finish(prepared, result) {
            self.actions.insert(attempt, action);
        }
        Ok(Transition::Unit)
    }

    pub(super) fn refresh_requests(&mut self) -> Result<(), Error> {
        self.requests.refresh(&self.broker.inspect())
    }
}

fn projection_class(stage: ActionState) -> u8 {
    match stage {
        ActionState::Proposed | ActionState::Prepared | ActionState::Reviewing | ActionState::Authorized => 0,
        ActionState::Dispatching | ActionState::Unknown | ActionState::IrrecoverablyUnknown => 1,
        ActionState::Confirmed => 2,
        ActionState::Denied => 3,
        ActionState::Cancelled => 4,
        ActionState::ConfirmedNotExecuted => 5,
    }
}

#[cfg(test)]
mod completion_tests {
    use super::*;
    use super::super::{
        FileDeliveryProfile, FilePermit, JournalIo, JournalLimits, ReferenceBallot, ReferenceReview,
    };
    use crate::action::{ElapsedTick, Purpose, ResolvedTarget, VERSION};
    use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
    use crate::action::consequence::delivery::EndpointOutcome;
    use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
    use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
    use crate::reducer::Caps;
    use crate::round::Verdict;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let root = std::env::temp_dir().join(format!(
                "fa-request-completion-{}-{time}-{}",
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
                eprintln!("request completion test cleanup: {error}");
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
            narrowed_targets: vec![target],
            target,
            initial_payload: b"initial".to_vec(),
            retention_ticks: 1000,
            max_deliveries: 8,
            clock_domain: 99,
            limits: JournalLimits::default(),
        }
    }

    fn snapshot() -> Snapshot {
        Snapshot {
            semantic_epoch: 1, complete: true,
            values: BTreeMap::from([(7, b"ok".to_vec())]),
        }
    }

    fn authorize_request(host: &mut FileDelivery, request: u64) -> (FilePermit, FrozenAction) {
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
        let (key, action) = authorize_request(&mut host, 700);
        (host, key, action)
    }

    #[test]
    fn completion_is_durable_and_exact_retry_does_not_dispatch_or_charge_twice() {
        let root = Directory::new();
        let p = profile();
        let (mut host, key, action) = prepared(&root, &p);
        let revision = host.revision();
        let outcome = EndpointOutcome::Executed { resulting_version: 2 };
        assert_eq!(host.commit_request_publication(revision, 700, &key, &action, snapshot()), Ok(outcome));
        assert_eq!(host.revision(), revision + 3);
        let after = host.inspect();
        assert_eq!(after.executions, 1);
        assert_eq!(after.payload, b"visible");
        assert_eq!(after.control.ledger.available, 84);
        assert_eq!(after.control.ledger.reserved, 0);
        assert_eq!(after.control.ledger.charged, 16);
        assert_eq!(after.control.ledger.stages[&key.attempt()], ActionState::Confirmed);
        assert_eq!(host.request_status(700).unwrap().generation, 2);
        assert_eq!(FileDelivery::read_publication(root.store(), &p).unwrap(), after);
        // This is historical retry, not a fresh authorization against this
        // intentionally incomplete snapshot or stale journal predecessor.
        assert_eq!(host.commit_request_publication(revision, 700, &key, &action, Snapshot::default()), Ok(outcome));
        assert_eq!(host.inspect(), after);
        drop(host);
        let mut reopened = FileDelivery::open(root.store(), p).unwrap();
        assert_eq!(reopened.inspect().executions, 1);
        assert_eq!(reopened.inspect().control.ledger.charged, 16);
        assert_eq!(reopened.commit_request_publication(reopened.revision(), 700, &key, &action, snapshot()),
            Err(JournalError::Contract(Error::Binding)));
    }

    #[test]
    fn failed_witness_validation_keeps_both_disk_and_reservation_unchanged() {
        let root = Directory::new();
        let p = profile();
        let (mut host, key, action) = prepared(&root, &p);
        let before = host.inspect();
        let mut changed = snapshot();
        changed.values.insert(7, b"changed".to_vec());
        assert_eq!(host.commit_request_publication(host.revision(), 700, &key, &action, changed),
            Err(JournalError::Contract(Error::Binding)));
        assert_eq!(host.inspect(), before);
        assert_eq!(FileDelivery::read_publication(root.store(), &p).unwrap(), before);
        assert!(host.storage_failure().is_none());
        assert_eq!(before.control.ledger.reserved, 16);
        assert_eq!(host.commit_request_publication(host.revision(), 700, &key, &action, snapshot()),
            Ok(EndpointOutcome::Executed { resulting_version: 2 }));
    }

    #[test]
    fn changed_action_foreign_key_and_other_request_cannot_redirect_a_permit() {
        let root = Directory::new();
        let other_root = Directory::new();
        let (mut host, key, action) = prepared(&root, &profile());
        let (_other, foreign, _) = prepared(&other_root, &profile());
        let before = host.inspect();
        let mut changed = action.spec().clone();
        changed.payload = b"substituted".to_vec();
        let changed = FrozenAction::freeze(changed).unwrap();
        assert_eq!(host.commit_request_publication(host.revision(), 700, &key, &changed, snapshot()),
            Err(JournalError::Contract(Error::Binding)));
        assert_eq!(host.commit_request_publication(host.revision(), 700, &foreign, &action, snapshot()),
            Err(JournalError::Contract(Error::Binding)));
        assert_eq!(host.commit_request_publication(host.revision(), 999, &key, &action, snapshot()),
            Err(JournalError::Contract(Error::Missing)));
        assert_eq!(host.inspect(), before);
        assert!(host.commit_request_publication(host.revision(), 700, &key, &action, snapshot()).is_ok());
    }

    #[test]
    fn exact_deadline_and_stale_predecessor_refuse_before_any_publication() {
        let root = Directory::new();
        let p = profile();
        let (mut host, key, action) = prepared(&root, &p);
        let before = host.inspect();
        assert_eq!(host.commit_request_publication(host.revision() - 1, 700, &key, &action, snapshot()),
            Err(JournalError::Contract(Error::Stale)));
        assert_eq!(host.inspect(), before);
        host.observe_time(host.revision(), ElapsedTick(100)).unwrap();
        let expired = host.inspect();
        assert_eq!(host.commit_request_publication(host.revision(), 700, &key, &action, snapshot()),
            Err(JournalError::Contract(Error::Stale)));
        assert_eq!(host.inspect(), expired);
        assert_eq!(FileDelivery::read_publication(root.store(), &p).unwrap(), expired);
        host.cancel_request(host.revision(), 700).unwrap();
        assert_eq!(host.inspect().control.ledger.available, 100);
        assert_eq!(host.inspect().executions, 0);
    }

    #[test]
    fn all_three_event_slots_are_preflighted_before_consuming_the_permit() {
        for limit in [6, 7] {
            let root = Directory::new();
            let mut p = profile();
            p.limits.events = limit;
            let (mut host, key, action) = prepared(&root, &p);
            assert_eq!(host.revision(), 4);
            let before = host.inspect();
            let result = host.commit_request_publication(host.revision(), 700, &key, &action, snapshot());
            if limit == 6 {
                assert_eq!(result, Err(JournalError::Contract(Error::Limit)));
                assert_eq!(host.inspect(), before);
                assert_eq!(FileDelivery::read_publication(root.store(), &p).unwrap(), before);
                assert!(host.storage_failure().is_none());
            } else {
                assert_eq!(result, Ok(EndpointOutcome::Executed { resulting_version: 2 }));
                assert_eq!(host.revision(), 7);
                assert_eq!(FileDelivery::read_publication(root.store(), &p).unwrap().executions, 1);
            }
        }
    }

    #[test]
    fn every_storage_barrier_preserves_atomic_visibility_and_poisons_retries() {
        for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
            JournalIo::Rename, JournalIo::DirectorySync]
        {
            let root = Directory::new();
            let p = profile();
            let (mut host, key, action) = prepared(&root, &p);
            let before = host.inspect();
            host.store.fail_once(barrier);
            assert!(matches!(host.commit_request_publication(host.revision(), 700, &key, &action, snapshot()),
                Err(JournalError::Io(_))));
            assert_eq!(host.inspect(), before);
            assert!(!host.clock_ready());
            assert_eq!(host.commit_request_publication(host.revision(), 700, &key, &action, snapshot()),
                Err(JournalError::Unavailable));
            let disk = FileDelivery::read_publication(root.store(), &p).unwrap();
            let replaced = barrier == JournalIo::DirectorySync;
            assert_eq!(disk.executions, u64::from(replaced));
            assert_eq!(disk.control.ledger.charged, if replaced { 16 } else { 0 });
            assert_eq!(disk.control.ledger.reserved, if replaced { 0 } else { 16 });
            assert_eq!(disk.control.ledger.stages[&key.attempt()],
                if replaced { ActionState::Confirmed } else { ActionState::Authorized });
            assert_eq!(disk.revision, before.revision + if replaced { 3 } else { 0 });
            drop(host);
            let recovered = FileDelivery::open(root.store(), p).unwrap();
            assert_eq!(recovered.inspect().executions, u64::from(replaced));
            assert_eq!(recovered.inspect().control.ledger.available, if replaced { 84 } else { 100 });
            assert_eq!(recovered.inspect().control.ledger.reserved, 0);
        }
    }
}
