//! Fresh evidence at first publication, not another authorization or receipt.
//! This profile is enabled before any proposal and retained in the original
//! journal. Historical outcomes and original reconciliation always take priority.
pub mod witnesses;
pub mod capture;
pub(super) mod witness_gate;
use super::{Event, FileOversight, JournalError, Transition};
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::EndpointOutcome;
use crate::action::consequence::oversight::CommitteeInput;
use crate::{Error, Snapshot};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicationBasis {
    Revalidated,
    Rejected(Error),
    PreviouslyResolved,
    DeadlineElapsed,
}

/// Supervisor data after acknowledged canonical replacement. `publish_checked`
/// alone does not refund a sealed nonexecution; `complete_checked_publication`
/// also applies the original receipt reconciliation before acknowledging it.
/// This value cannot authorize another action or act as an endpoint key.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// use fa_reference::action::consequence::delivery::persistent::observed::publication::CheckedPublication;
/// fn authorize(result: CheckedPublication) -> FilePermit { result }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckedPublication {
    pub outcome: EndpointOutcome,
    pub basis: PublicationBasis,
}

/// Supervisor-owned inputs for completing an already reviewed, doubly approved
/// action. Both opaque keys must come from this live owner. `now` is a trusted
/// tick obtained AFTER capturing `current` and `snapshot`, not a saved timestamp.
/// Constructing this input neither creates a permit nor authenticates a source.
pub struct CheckedCompletion<'a> {
    pub automatic: &'a super::FilePermit,
    pub human: &'a super::FileHumanPermit,
    pub action: &'a crate::action::FrozenAction,
    pub current: &'a CommitteeInput,
    pub snapshot: Snapshot,
    pub now: ElapsedTick,
}

impl FileOversight {
    /// Install once before any actor/operator proposal, including refused actor
    /// admissions. No disable, post-review upgrade or weaker publication fallback
    /// exists for this owner. Legacy owners remain explicitly unguarded.
    pub fn enable_publication_guard(&mut self, revision: u64) -> Result<(), JournalError> {
        self.transact(revision, Event::PublicationGuard)?;
        Ok(())
    }

    pub fn publication_guard_required(&self) -> bool { self.machine.publication_guard }

    /// Supply a freshly captured whole input and policy snapshot, then the trusted
    /// elapsed tick obtained AFTER capture. A missing/stale basis seals the
    /// original endpoint request; it cannot be retried into execution with better
    /// evidence. The endpoint's retained outcome wins over a later missing source.
    ///
    /// This can execute only an ORIGINAL envelope retained by this live owner.
    /// Reopening discards sendable envelopes and cannot reconstruct one through
    /// this API. Clock, source authenticity and coverage remain host assumptions.
    pub fn publish_checked(&mut self, revision: u64, attempt: u64,
        current: Option<&CommitteeInput>, snapshot: Snapshot, now: ElapsedTick)
        -> Result<CheckedPublication, JournalError>
    {
        if let Some(input) = current { self.check_action(attempt, input.action())?; }
        let supplied = current.map(|input| input.views().clone());
        match self.transact(revision, Event::PublishChecked(attempt, supplied, snapshot, now))? {
            Transition::PublicationChecked(result) => Ok(result),
            _ => unreachable!("checked publication transition"),
        }
    }

    /// Consume both existing approvals, publish with fresh evidence, and accept
    /// the endpoint's original receipt in ONE canonical replacement. The four
    /// original events (Time, Dispatch, PublishChecked, Reconcile) remain in the
    /// unchanged journal format; the revision advances by four.
    ///
    /// No intermediate consumed key, payload, charge or refund is acknowledged.
    /// Admission/capacity failures leave the old cut intact. Any storage failure
    /// poisons the live owner, including an ambiguous post-rename sync failure;
    /// recovery reads the actual old-or-complete cut and never repeats dispatch.
    /// A repeated call cannot dispatch twice and does not mint replacement keys.
    ///
    /// This is ONLY for this host's journal-as-publication sink. It is not an
    /// atomic transaction over remote providers or independent file endpoints.
    /// Mandatory credential, witness, identity, source and consistency checks
    /// still run in the original machine. A stronger configured profile that
    /// requires another publication route refuses; there is no weaker fallback.
    pub fn complete_checked_publication(&mut self, revision: u64,
        completion: CheckedCompletion<'_>) -> Result<CheckedPublication, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        let CheckedCompletion { automatic, human, action, current, snapshot, now } = completion;
        if !std::rc::Rc::ptr_eq(&self.issuer, &automatic.issuer)
            || !std::rc::Rc::ptr_eq(&self.issuer, &human.issuer)
            || automatic.attempt != human.attempt
        { return Err(Error::Binding.into()); }
        self.check_action(automatic.attempt, action)?;
        self.check_action(automatic.attempt, current.action())?;
        let input_revision = self.current_reference(automatic.attempt, current)?;
        // Bound caller-owned data before duplicating it into two original events.
        if snapshot.values.len() > super::super::MAX_SNAPSHOT_ENTRIES {
            return Err(Error::Limit.into());
        }
        let snapshot_bytes = snapshot.values.values().try_fold(0_usize, |total, value| {
            total.checked_add(value.len())
        }).ok_or(Error::Limit)?;
        if snapshot_bytes > super::super::MAX_SNAPSHOT_BYTES { return Err(Error::Limit.into()); }
        let attempt = automatic.attempt;
        let transitions = self.commit_publication_cut(revision, &[
            Event::Core(super::BaseEvent::Time(now)),
            Event::Dispatch(attempt, human.request, input_revision, snapshot.clone()),
            Event::PublishChecked(attempt, Some(current.views().clone()), snapshot, now),
            Event::Core(super::BaseEvent::Reconcile(attempt)),
        ])?;
        match transitions.as_slice() {
            [Transition::Unit, Transition::Unit, Transition::PublicationChecked(publication),
                Transition::Reconciled(super::Reconciliation::Resolved(outcome))]
                if publication.outcome == *outcome => Ok(*publication),
            _ => unreachable!("checked publication followed by original receipt reconciliation"),
        }
    }

    /// Private, fixed original-event composition. Never an event import API.
    /// Every prefix retains canonical admission, recovery reserves, live source
    /// admission and original semantic gates. Speculation performs no sink I/O.
    pub(super) fn commit_publication_cut(&mut self, revision: u64, next: &[Event])
        -> Result<Vec<Transition>, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        match next {
            [Event::Core(super::BaseEvent::Time(start)), Event::Dispatch(dispatch, _, _, _),
                Event::PublishChecked(publish, _, _, finish), Event::Core(super::BaseEvent::Reconcile(reconcile))]
                if dispatch == publish && publish == reconcile && start == finish => {}
            _ => return Err(Error::InvalidInput.into()),
        }
        for event in next { self.check_source_admission(event)?; }
        let count = self.events.len().checked_add(next.len()).ok_or(Error::Overflow)?;
        if count > self.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        let mut history = Vec::new();
        history.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        history.extend(self.events.iter().cloned());
        let mut transitions = Vec::new();
        transitions.try_reserve_exact(next.len()).map_err(|_| Error::Limit)?;
        let mut candidate = super::Machine::replay(&self.profile, &self.events)?;
        let mut bytes = Vec::new();
        for event in next {
            bytes = super::journal::encode_appended(&self.profile, self.store.identity(), &history, event)?;
            candidate.preflight_consistency(event)?;
            transitions.push(candidate.apply(event)?);
            history.push(event.clone());
        }
        // Like persist_candidate, close admission BEFORE entering the storage
        // call so a caught unwind cannot reuse an unacknowledged effect key.
        self.fault = Some(super::JournalFailure { operation: super::JournalIo::Stage,
            kind: std::io::ErrorKind::Other, replacement_may_be_visible: true });
        if let Err(error) = self.store.replace(&bytes) {
            self.fault = Some(match &error {
                JournalError::Io(failure) => failure.clone(),
                _ => super::JournalFailure { operation: super::JournalIo::Stage,
                    kind: std::io::ErrorKind::Other, replacement_may_be_visible: true },
            });
            return Err(error);
        }
        for event in next { self.source_operation_committed(event); }
        self.events = history;
        self.machine = candidate;
        self.fault = None;
        Ok(transitions)
    }
}

#[cfg(test)]
mod completion_tests {
    use super::*;
    use super::super::{FileHumanPermit, FileHumanReviewer, FileOversightProfile, FilePermit,
        JournalIo, ReviewWindow};
    use super::super::super::{FileDeliveryProfile, JournalLimits};
    use crate::action::{ActionSpec, ActionState, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
    use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
    use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
    use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
    use crate::action::consequence::oversight::{CommitteeContract, HelperContract, action_frame};
    use crate::action::consequence::oversight::human::{HumanDisposition, HumanReviewPolicy};
    use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
    use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
    use crate::reducer::Caps;
    use crate::round::{Verdict, commitment};
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let path = std::env::temp_dir().join(format!("fa-checked-completion-{}-{stamp}-{}",
                std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn store(&self) -> PathBuf { self.0.join("publication") }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("checked completion cleanup: {error}"); }
        }
    }
    fn profile() -> FileOversightProfile {
        let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
        FileOversightProfile {
            delivery: FileDeliveryProfile {
                scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
                total: 100, max_attempts: 8,
                actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                    model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
                    grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
                suspend_at_incident: 3,
                policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap(),
                congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".to_owned(),
                    MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
                    caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
                    narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
                narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
                retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
            },
            committee: CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(), HelperContract::new(
                InputProfileBinding { profile_id: 1, profile_bytes: b"exact-view-v1".to_vec(),
                    tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec(),
            ).unwrap())])).unwrap(),
            human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
        }
    }
    fn snapshot() -> Snapshot {
        Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
    }
    struct Ready {
        host: FileOversight,
        reviewer: FileHumanReviewer,
        action: FrozenAction,
        inputs: CommitteeInput,
        automatic: FilePermit,
        human: FileHumanPermit,
    }
    impl Ready {
        fn complete(&mut self, tick: u64) -> Result<CheckedPublication, JournalError> {
            self.host.complete_checked_publication(self.host.revision(), CheckedCompletion {
                automatic: &self.automatic, human: &self.human, action: &self.action,
                current: &self.inputs, snapshot: snapshot(), now: ElapsedTick(tick),
            })
        }
        fn bytes(&self) -> Vec<u8> { self.host.store.read(self.host.profile.delivery.limits.bytes).unwrap() }
    }
    fn ready(root: &Directory, p: FileOversightProfile) -> Ready {
        let (mut host, reviewer) = FileOversight::create(root.store(), p.clone()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        host.enable_publication_guard(host.revision()).unwrap();
        let action = host.propose(host.revision(), 1, ActionSpec { version: VERSION, scope: p.delivery.scope,
            target: Some(host.inspect().target), payload: b"visible".to_vec(), required_witnesses: Vec::new(),
            policy_epoch: 0, deadline: ElapsedTick(100), units: 16 }, snapshot()).unwrap();
        let helper = &p.committee.members()["reviewer"];
        let mut bytes = action_frame(&action);
        let boundary = bytes.len();
        bytes.extend_from_slice(helper.question());
        let end = bytes.len();
        let actual = ActualHelperInput::new(bytes, helper.profile_at(0), vec![
            SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
            SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
        ], Vec::new()).unwrap();
        let manifest = EvidenceViewManifest::new(actual, AuthorizationProjection {
            projection_id: 7, policy_epoch: 0, projected_originals: Vec::new(),
        }, Vec::new()).unwrap();
        let inputs = CommitteeInput::capture(&action, &p.committee,
            BTreeMap::from([("reviewer".to_owned(), manifest)])).unwrap();
        host.record_inputs(host.revision(), 1, 0, inputs.clone()).unwrap();
        host.begin_review(host.revision(), 1, 101, [9; 32], ReviewWindow {
            commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8),
        }, snapshot()).unwrap();
        let digest = commitment(101, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap();
        host.commit_review(host.revision(), 101, "reviewer", digest).unwrap();
        host.open_reveals(host.revision(), 101).unwrap();
        host.reveal_review(host.revision(), 101, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
        host.finish_review(host.revision(), 101, Some(&inputs), snapshot()).unwrap().unwrap();
        let automatic = host.authorize(host.revision(), 1, &inputs, snapshot()).unwrap();
        let request = host.request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(10)).unwrap();
        let revision = host.revision();
        let human = reviewer.approve(&mut host, revision, &request).unwrap();
        Ready { host, reviewer, action, inputs, automatic, human }
    }

    #[test]
    fn both_keys_complete_one_real_publication_and_survive_recovery() {
        let root = Directory::new();
        let mut r = ready(&root, profile());
        let revision = r.host.revision();
        assert_eq!(r.complete(2), Ok(CheckedPublication {
            outcome: EndpointOutcome::Executed { resulting_version: 2 }, basis: PublicationBasis::Revalidated,
        }));
        assert_eq!(r.host.revision(), revision + 4);
        let cut = r.host.inspect();
        assert_eq!(cut.payload, b"visible");
        assert_eq!(cut.executions, 1);
        assert_eq!(cut.control.ledger.stages[&1], ActionState::Confirmed);
        assert_eq!(cut.control.ledger.available, 84);
        assert_eq!(cut.control.ledger.reserved, 0);
        assert_eq!(cut.control.ledger.charged, 16);
        assert_eq!(r.host.human_status(1001).unwrap().disposition, HumanDisposition::Consumed);
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), cut);
        let bytes = r.bytes();
        assert!(r.complete(2).is_err());
        assert_eq!(r.host.inspect(), cut);
        assert_eq!(r.bytes(), bytes);
        drop(r);
        let (reopened, _) = FileOversight::open(root.store(), profile()).unwrap();
        assert_eq!(reopened.inspect().executions, 1);
        assert_eq!(reopened.inspect().control.ledger.stages[&1], ActionState::Confirmed);
        assert_eq!(reopened.inspect().control.ledger.charged, 16);
    }

    #[test]
    fn atomic_result_matches_the_four_original_sequential_transitions() {
        let a = Directory::new(); let b = Directory::new();
        let mut atomic = ready(&a, profile()); let mut sequential = ready(&b, profile());
        let expected = atomic.complete(2).unwrap();
        sequential.host.observe_time(sequential.host.revision(), ElapsedTick(2)).unwrap();
        sequential.host.dispatch(sequential.host.revision(), &sequential.automatic, &sequential.human,
            &sequential.action, &sequential.inputs, snapshot()).unwrap();
        let result = sequential.host.publish_checked(sequential.host.revision(), 1,
            Some(&sequential.inputs), snapshot(), ElapsedTick(2)).unwrap();
        assert_eq!(result, expected);
        assert_eq!(sequential.host.reconcile(sequential.host.revision(), 1).unwrap(),
            super::super::Reconciliation::Resolved(result.outcome));
        assert_eq!(atomic.host.inspect(), sequential.host.inspect());
    }

    #[test]
    fn foreign_automatic_or_human_key_never_reaches_the_sink() {
        let a = Directory::new(); let b = Directory::new();
        let mut r = ready(&a, profile()); let foreign = ready(&b, profile());
        let before = r.host.inspect(); let bytes = r.bytes();
        for (automatic, human) in [(&foreign.automatic, &r.human), (&r.automatic, &foreign.human)] {
            assert_eq!(r.host.complete_checked_publication(r.host.revision(), CheckedCompletion {
                automatic, human, action: &r.action, current: &r.inputs,
                snapshot: snapshot(), now: ElapsedTick(2),
            }), Err(JournalError::Contract(Error::Binding)));
        }
        assert_eq!(r.host.inspect(), before); assert_eq!(r.bytes(), bytes);
        assert!(r.complete(2).is_ok());
    }

    #[test]
    fn changed_payload_or_policy_snapshot_cannot_consume_either_approval() {
        let root = Directory::new(); let mut r = ready(&root, profile());
        let before = r.host.inspect(); let bytes = r.bytes();
        let mut spec = r.action.spec().clone(); spec.payload = b"substitution".to_vec();
        let changed = FrozenAction::freeze(spec).unwrap();
        assert_eq!(r.host.complete_checked_publication(r.host.revision(), CheckedCompletion {
            automatic: &r.automatic, human: &r.human, action: &changed, current: &r.inputs,
            snapshot: snapshot(), now: ElapsedTick(2),
        }), Err(JournalError::Contract(Error::Binding)));
        let mut stale = snapshot(); stale.values.insert(7, b"changed".to_vec());
        assert!(r.host.complete_checked_publication(r.host.revision(), CheckedCompletion {
            automatic: &r.automatic, human: &r.human, action: &r.action, current: &r.inputs,
            snapshot: stale, now: ElapsedTick(2),
        }).is_err());
        assert_eq!(r.host.inspect(), before); assert_eq!(r.bytes(), bytes);
        assert_eq!(r.host.human_status(1001).unwrap().disposition, HumanDisposition::Approved);
        assert!(r.complete(2).is_ok());
    }

    #[test]
    fn fresh_tick_checks_human_expiry_before_any_publication() {
        let root = Directory::new(); let mut r = ready(&root, profile());
        let before = r.host.inspect(); let bytes = r.bytes();
        assert!(r.complete(10).is_err());
        assert_eq!(r.host.inspect(), before); assert_eq!(r.bytes(), bytes);
        // Separate positive control, not a claim that time can be rolled back.
        let valid_root = Directory::new(); let mut valid = ready(&valid_root, profile());
        assert!(valid.complete(9).is_ok());
    }

    #[test]
    fn revoked_human_approval_cannot_be_reconstructed_by_batch_replay() {
        let root = Directory::new(); let mut r = ready(&root, profile());
        let revision = r.host.revision();
        r.reviewer.revoke_all(&mut r.host, revision).unwrap();
        let before = r.host.inspect(); let bytes = r.bytes();
        assert!(r.complete(2).is_err());
        assert_eq!(r.host.inspect(), before); assert_eq!(r.bytes(), bytes);
        assert_eq!(r.host.inspect().executions, 0);
    }

    #[test]
    fn stale_revision_and_interrupted_source_refuse_without_advancing_time() {
        let root = Directory::new(); let mut r = ready(&root, profile());
        let before = r.host.inspect(); let bytes = r.bytes();
        assert_eq!(r.host.complete_checked_publication(r.host.revision() - 1, CheckedCompletion {
            automatic: &r.automatic, human: &r.human, action: &r.action, current: &r.inputs,
            snapshot: snapshot(), now: ElapsedTick(2),
        }), Err(JournalError::Contract(Error::Stale)));
        r.host.source_interrupted = true;
        assert_eq!(r.complete(2), Err(JournalError::Contract(Error::Incomplete)));
        assert!(r.host.source_interrupted);
        assert_eq!(r.host.inspect(), before); assert_eq!(r.bytes(), bytes);
    }

    #[test]
    fn caller_snapshot_limits_are_checked_before_cloning_batch_events() {
        let root = Directory::new(); let mut r = ready(&root, profile());
        let before = r.host.inspect(); let bytes = r.bytes();
        let mut large_count = snapshot();
        large_count.values = (0..=super::super::super::MAX_SNAPSHOT_ENTRIES as u64)
            .map(|key| (key, Vec::new())).collect();
        let mut large_bytes = snapshot();
        large_bytes.values.insert(7, vec![0; super::super::super::MAX_SNAPSHOT_BYTES + 1]);
        for supplied in [large_count, large_bytes] {
            assert_eq!(r.host.complete_checked_publication(r.host.revision(), CheckedCompletion {
                automatic: &r.automatic, human: &r.human, action: &r.action, current: &r.inputs,
                snapshot: supplied, now: ElapsedTick(2),
            }), Err(JournalError::Contract(Error::Limit)));
        }
        assert_eq!(r.host.inspect(), before); assert_eq!(r.bytes(), bytes);
        assert!(r.complete(2).is_ok());
    }

    #[test]
    fn entire_four_event_cut_must_fit_before_any_prefix_is_committed() {
        let baseline_root = Directory::new(); let baseline = ready(&baseline_root, profile());
        let count = usize::try_from(baseline.host.revision()).unwrap();
        for spare in [3, 4] {
            let root = Directory::new(); let mut p = profile(); p.delivery.limits.events = count + spare;
            let mut r = ready(&root, p); let before = r.host.inspect(); let bytes = r.bytes();
            if spare == 3 {
                assert_eq!(r.complete(2), Err(JournalError::Contract(Error::Limit)));
                assert_eq!(r.host.inspect(), before); assert_eq!(r.bytes(), bytes);
                assert!(r.host.storage_failure().is_none());
            } else {
                assert!(r.complete(2).is_ok());
                assert_eq!(r.host.revision(), u64::try_from(count + 4).unwrap());
            }
        }
    }

    #[test]
    fn every_storage_barrier_recovers_old_or_fully_reconciled_never_partial() {
        for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
            JournalIo::Rename, JournalIo::DirectorySync] {
            let root = Directory::new(); let mut r = ready(&root, profile());
            let before = r.host.inspect();
            r.host.store.fail_once(barrier);
            let JournalError::Io(failure) = r.complete(2).unwrap_err() else { panic!("selected storage barrier"); };
            assert_eq!(failure.operation, barrier);
            assert_eq!(r.host.inspect(), before);
            assert_eq!(r.complete(2), Err(JournalError::Unavailable));
            assert!(!r.host.clock_ready());
            let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
            let replaced = barrier == JournalIo::DirectorySync;
            assert_eq!(disk.executions, u64::from(replaced));
            assert_eq!(disk.control.ledger.stages[&1],
                if replaced { ActionState::Confirmed } else { ActionState::Authorized });
            assert_eq!(disk.control.ledger.reserved, if replaced { 0 } else { 16 });
            assert_eq!(disk.control.ledger.charged, if replaced { 16 } else { 0 });
            drop(r);
            let (recovered, _) = FileOversight::open(root.store(), profile()).unwrap();
            assert_eq!(recovered.inspect().executions, u64::from(replaced));
            assert_eq!(recovered.inspect().control.ledger.reserved, 0);
            assert_eq!(recovered.inspect().control.ledger.available, if replaced { 84 } else { 100 });
            assert_eq!(recovered.inspect().control.ledger.stages[&1],
                if replaced { ActionState::Confirmed } else { ActionState::Cancelled });
        }
    }
}
