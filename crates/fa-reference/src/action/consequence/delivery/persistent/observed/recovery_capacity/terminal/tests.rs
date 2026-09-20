//! Real canonical files and original two-key/receipt reducers. Helper verdicts
//! are deterministic fixtures, NOT inference or native-provider qualification.
use super::*;
use crate::action::{ActionSpec, ActionState, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, FilePermit, JournalLimits, RecoveryReserve, Reconciliation};
use crate::action::consequence::delivery::persistent::observed::{FileHumanPermit, FileHumanRequest, FileHumanReviewer};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, ReviewWindow, action_frame};
use crate::action::consequence::oversight::human::{HumanDisposition, HumanReviewPolicy};
use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use crate::reducer::Caps;
use crate::round::{Verdict, commitment};
use crate::Snapshot;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
    JournalIo::Rename, JournalIo::DirectorySync];
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-two-key-terminal-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("two-key terminal cleanup: {error}"); }
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
fn stop_request(host: &FileOversight) -> StopRequest {
    let control = host.inspect().control;
    StopRequest { operation: 900, expected_control_sequence: control.sequence,
        expected_authority_epoch: control.ledger.epoch }
}
struct Ready {
    host: FileOversight, reviewer: FileHumanReviewer, action: FrozenAction,
    inputs: CommitteeInput, automatic: FilePermit, human: FileHumanPermit, request: FileHumanRequest,
}
impl Ready {
    fn new(root: &Directory, p: FileOversightProfile, reserved: bool) -> Self {
        let (mut host, reviewer) = FileOversight::create(root.store(), p.clone()).unwrap();
        if reserved { host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap(); }
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let action = host.propose(host.revision(), 1, ActionSpec { version: VERSION, scope: p.delivery.scope,
            target: Some(host.inspect().target), payload: b"visible".to_vec(), required_witnesses: Vec::new(),
            policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 }, snapshot()).unwrap();
        let helper = &p.committee.members()["reviewer"];
        let mut bytes = action_frame(&action);
        let boundary = bytes.len();
        bytes.extend_from_slice(helper.question());
        let end = bytes.len();
        let actual = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
            SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
            SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
        ], Vec::new()).unwrap();
        let view = EvidenceViewManifest::new(actual, AuthorizationProjection { projection_id: 7,
            policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new() }, Vec::new()).unwrap();
        let inputs = CommitteeInput::capture(&action, &p.committee, BTreeMap::from([("reviewer".to_owned(), view)])).unwrap();
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
        Self { host, reviewer, action, inputs, automatic, human, request }
    }
    fn dispatch(&mut self) {
        self.host.dispatch(self.host.revision(), &self.automatic, &self.human,
            &self.action, &self.inputs, snapshot()).unwrap();
    }
}

#[test]
fn live_terminal_cut_distinguishes_reserved_sent_and_executed_work() {
    for mode in 0..3 {
        let root = Directory::new();
        let mut r = Ready::new(&root, profile(), false);
        if mode > 0 { r.dispatch(); }
        if mode == 2 { assert_eq!(r.host.publish(r.host.revision(), 1), Ok(EndpointOutcome::Executed { resulting_version: 2 })); }
        let before = r.host.inspect();
        let request = stop_request(&r.host);
        let sweep = r.host.stop_and_drain(before.revision, request, ElapsedTick(2)).unwrap();
        let after = r.host.inspect();
        assert_eq!(after.revision, before.revision + 2);
        assert!(sweep.progress.drained());
        assert_eq!(sweep.progress.receipt.request(), request);
        assert_eq!(after.executions, u64::from(mode == 2));
        assert_eq!(after.control.ledger.reserved, 0);
        assert_eq!(after.control.ledger.charged, if mode == 2 { 16 } else { 0 });
        assert_eq!(after.control.ledger.available, if mode == 2 { 84 } else { 100 });
        match mode {
            0 => {
                assert!(sweep.outcomes.is_empty());
                assert_eq!(after.control.ledger.stages[&1], ActionState::Cancelled);
                assert_eq!(r.host.human_status(1001).unwrap().disposition, HumanDisposition::Revoked);
            }
            1 => assert_eq!(sweep.outcomes[&1], Ok(Reconciliation::Resolved(EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed }))),
            _ => assert_eq!(sweep.outcomes[&1], Ok(Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }))),
        }
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), after);
        assert!(r.host.dispatch(r.host.revision(), &r.automatic, &r.human, &r.action, &r.inputs, snapshot()).is_err());
        assert!(r.host.publish(r.host.revision(), 1).is_err());
        assert_eq!(r.host.inspect(), after);
    }
}

#[test]
fn terminal_tail_remains_usable_when_ordinary_capacity_is_exactly_exhausted() {
    for recover in [false, true] {
        let root = Directory::new();
        let mut p = profile(); p.delivery.limits.events = 15;
        let mut r = Ready::new(&root, p.clone(), true);
        assert_eq!(r.host.revision(), 12);
        assert_eq!(r.host.journal_capacity().unwrap().ordinary_remaining().events, 0);
        let before = r.host.inspect();
        assert_eq!(r.host.observe_time(r.host.revision(), ElapsedTick(2)), Err(Error::Limit.into()));
        assert_eq!(r.host.inspect(), before);
        let request = stop_request(&r.host);
        let (host, sweep) = if recover {
            drop(r.host);
            FileOversight::open_stopped(root.store(), p.clone(), request, ElapsedTick(2)).unwrap()
        } else {
            let sweep = r.host.stop_and_drain(r.host.revision(), request, ElapsedTick(2)).unwrap();
            (r.host, sweep)
        };
        assert!(sweep.progress.drained());
        assert_eq!(host.revision(), before.revision + if recover { 3 } else { 2 });
        assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, 0);
        assert_eq!(host.inspect().control.ledger.available, 100);
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(FileOversight::read_publication(root.store(), &p).unwrap(), host.inspect());
    }
}

#[test]
fn validation_refusals_do_not_acknowledge_a_partial_stop_or_revoke_keys() {
    let root = Directory::new();
    let mut r = Ready::new(&root, profile(), false);
    let request = stop_request(&r.host);
    let before = r.host.inspect();
    let mut wrong = request; wrong.expected_authority_epoch += 1;
    for (revision, stop, tick) in [(before.revision - 1, request, 2),
        (before.revision, wrong, 2), (before.revision, request, 0)] {
        assert_eq!(r.host.stop_and_drain(revision, stop, ElapsedTick(tick)), Err(Error::Stale.into()));
        assert_eq!(r.host.inspect(), before);
        assert_eq!(r.host.human_status(1001).unwrap().disposition, HumanDisposition::Approved);
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), before);
        assert!(r.host.storage_failure().is_none());
    }
    // Near-identical permitted control: the ORIGINAL two keys still dispatch.
    r.dispatch();
    assert_eq!(r.host.publish(r.host.revision(), 1), Ok(EndpointOutcome::Executed { resulting_version: 2 }));
}

#[test]
fn exact_event_neighbors_preflight_the_whole_recovery_sequence() {
    for recover in [false, true] {
        for enough in [false, true] {
            let root = Directory::new();
            let mut p = profile();
            let needed = if recover { 3 } else { 2 };
            p.delivery.limits.events = 11 + needed - usize::from(!enough);
            let mut r = Ready::new(&root, p.clone(), false);
            assert_eq!(r.host.revision(), 11);
            let before = r.host.inspect(); let request = stop_request(&r.host);
            let result = if recover {
                drop(r.host);
                FileOversight::open_stopped(root.store(), p.clone(), request, ElapsedTick(2)).map(|(_, sweep)| sweep)
            } else { r.host.stop_and_drain(before.revision, request, ElapsedTick(2)) };
            if enough { assert!(result.unwrap().progress.drained()); }
            else {
                assert_eq!(result, Err(Error::Limit.into()));
                assert_eq!(FileOversight::read_publication(root.store(), &p).unwrap(), before);
            }
        }
    }
}

#[test]
fn exact_byte_neighbors_refuse_before_a_partial_terminal_write() {
    let reference = Directory::new();
    let r = Ready::new(&reference, profile(), false);
    let request = stop_request(&r.host);
    for enough in [false, true] {
        let root = Directory::new();
        let mut p = profile();
        let mut events = r.host.events.clone();
        events.extend([Event::Core(BaseEvent::Stop(request)), Event::Core(BaseEvent::StopProgress(ElapsedTick(2)))]);
        let exact = journal::encode(&p, &root.store(), &events).unwrap().len();
        p.delivery.limits.bytes = exact - usize::from(!enough);
        let mut control = Ready::new(&root, p.clone(), false);
        let before = control.host.inspect();
        let result = control.host.stop_and_drain(before.revision, request, ElapsedTick(2));
        if enough {
            assert!(result.unwrap().progress.drained());
            assert_eq!(control.host.journal_capacity().unwrap().used().bytes, exact);
        } else {
            assert_eq!(result, Err(Error::Limit.into()));
            assert_eq!(control.host.inspect(), before);
            assert_eq!(FileOversight::read_publication(root.store(), &p).unwrap(), before);
        }
    }
}

#[test]
fn every_storage_barrier_hides_candidate_refunds_and_recovery_uses_actual_disk() {
    for barrier in BARRIERS {
        let root = Directory::new(); let mut r = Ready::new(&root, profile(), false);
        r.dispatch();
        let before = r.host.inspect(); let request = stop_request(&r.host);
        r.host.store.fail_once(barrier);
        let error = r.host.stop_and_drain(before.revision, request, ElapsedTick(2)).unwrap_err();
        let JournalError::Io(failure) = error else { panic!("selected storage barrier"); };
        assert_eq!(failure.operation, barrier);
        assert_eq!(failure.replacement_may_be_visible, matches!(barrier, JournalIo::Rename | JournalIo::DirectorySync));
        assert_eq!(r.host.inspect(), before);
        assert!(!r.host.clock_ready());
        assert_eq!(r.host.stop_and_drain(before.revision, request, ElapsedTick(2)), Err(JournalError::Unavailable));
        let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
        let visible = barrier == JournalIo::DirectorySync;
        assert_eq!(disk.revision, before.revision + if visible { 2 } else { 0 });
        assert_eq!(disk.control.ledger.charged, if visible { 0 } else { 16 });
        assert_eq!(disk.stop.is_some(), visible);
        drop(r.host);
        let (recovered, sweep) = FileOversight::open_stopped(root.store(), profile(), request, ElapsedTick(3)).unwrap();
        assert!(sweep.progress.drained());
        assert_eq!(recovered.inspect().executions, 0);
        assert_eq!(recovered.inspect().control.ledger.available, 100);
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), recovered.inspect());
    }
}

#[test]
fn recovered_two_key_owner_keeps_execution_and_expired_liabilities_distinct() {
    for mode in 0..3 {
        let root = Directory::new(); let mut r = Ready::new(&root, profile(), false);
        r.dispatch();
        if mode == 1 { r.host.publish(r.host.revision(), 1).unwrap(); }
        let before = r.host.inspect(); let request = stop_request(&r.host);
        drop(r.host);
        let tick = if mode == 2 { 1001 } else { 2 };
        let (mut recovered, sweep) = FileOversight::open_stopped(root.store(), profile(), request, ElapsedTick(tick)).unwrap();
        assert_eq!(recovered.revision(), before.revision + 3);
        assert!(recovered.inspect().dispatcher_epoch > before.dispatcher_epoch);
        assert_eq!(sweep.progress.drained(), mode != 2);
        assert_eq!(recovered.inspect().executions, u64::from(mode == 1));
        assert_eq!(recovered.inspect().control.ledger.charged, if mode == 0 { 0 } else { 16 });
        if mode == 2 {
            assert_eq!(sweep.outcomes[&1], Ok(Reconciliation::RetentionExpired));
            assert_eq!(sweep.progress.unresolved, vec![1]);
            assert_eq!(sweep.progress.irrecoverable, vec![1]);
        }
        assert_eq!(recovered.dispatch(recovered.revision(), &r.automatic, &r.human, &r.action, &r.inputs, snapshot()), Err(Error::Binding.into()));
        let revision = recovered.revision();
        assert!(r.reviewer.approve(&mut recovered, revision, &r.request).is_err());
        assert!(recovered.publish(recovered.revision(), 1).is_err());
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), recovered.inspect());
    }
}

#[test]
fn open_requires_exclusive_owner_exact_profile_and_original_stop_preconditions() {
    let root = Directory::new(); let r = Ready::new(&root, profile(), false);
    let before = r.host.inspect(); let request = stop_request(&r.host);
    assert!(matches!(FileOversight::open_stopped(root.store(), profile(), request, ElapsedTick(2)), Err(JournalError::Busy)));
    drop(r.host);
    let mut different = profile(); different.delivery.total += 1;
    assert!(matches!(FileOversight::open_stopped(root.store(), different, request, ElapsedTick(2)), Err(JournalError::Contract(Error::Binding))));
    let mut wrong = request; wrong.expected_control_sequence += 1;
    for (stop, tick) in [(wrong, 2), (request, 0)] {
        assert!(matches!(FileOversight::open_stopped(root.store(), profile(), stop, ElapsedTick(tick)), Err(JournalError::Contract(Error::Stale))));
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), before);
    }
    let (recovered, sweep) = FileOversight::open_stopped(root.store(), profile(), request, ElapsedTick(2)).unwrap();
    assert!(sweep.progress.drained()); assert_eq!(recovered.inspect().control.ledger.available, 100);
    assert_eq!(recovered.human_status(1001).unwrap().disposition, HumanDisposition::Revoked);
}

#[test]
fn exact_stopped_recovery_keeps_original_receipt_but_advances_current_fence() {
    let root = Directory::new(); let r = Ready::new(&root, profile(), false);
    let request = stop_request(&r.host); drop(r.host);
    let (recovered, first) = FileOversight::open_stopped(root.store(), profile(), request, ElapsedTick(2)).unwrap();
    let before = recovered.inspect(); drop(recovered);
    let mut wrong = request; wrong.operation += 1;
    assert!(matches!(FileOversight::open_stopped(root.store(), profile(), wrong, ElapsedTick(3)), Err(JournalError::Contract(Error::Duplicate))));
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), before);
    let (mut recovered, second) = FileOversight::open_stopped(root.store(), profile(), request, ElapsedTick(3)).unwrap();
    assert_eq!(first.progress.receipt, second.progress.receipt);
    assert!(second.progress.dispatcher_epoch > first.progress.dispatcher_epoch);
    assert!(second.progress.drained());
    assert_eq!(recovered.inspect().control.ledger.available, 100);
    assert!(recovered.propose(recovered.revision(), 2, r.action.spec().clone(), snapshot()).is_err());
    assert_eq!(recovered.inspect().executions, 0);
}

#[test]
fn interrupted_source_latch_is_not_a_prerequisite_for_terminal_drain() {
    for interrupted in [false, true] {
        let root = Directory::new(); let mut r = Ready::new(&root, profile(), false);
        // Causal injection of the native acquisition latch, not a producer test.
        r.host.source_interrupted = interrupted;
        let request = stop_request(&r.host);
        let sweep = r.host.stop_and_drain(r.host.revision(), request, ElapsedTick(2)).unwrap();
        assert!(sweep.progress.drained());
        assert_eq!(r.host.source_interrupted, interrupted);
        assert_eq!(r.host.inspect().control.ledger.available, 100);
    }
}

#[test]
fn stored_publication_guard_survives_source_free_terminal_recovery() {
    let root = Directory::new();
    let (mut host, _reviewer) = FileOversight::create(root.store(), profile()).unwrap();
    host.enable_publication_guard(host.revision()).unwrap();
    let request = stop_request(&host);
    drop(host);
    let (host, sweep) = FileOversight::open_stopped(root.store(), profile(), request, ElapsedTick(0)).unwrap();
    assert!(host.publication_guard_required());
    assert!(sweep.progress.drained());
    assert!(host.clock_ready());
    assert_eq!(host.inspect().executions, 0);
}
