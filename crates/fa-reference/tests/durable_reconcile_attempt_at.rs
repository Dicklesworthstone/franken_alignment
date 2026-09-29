//! Original two-key journal integration, not helper/provider qualification.
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits, RecoveryReserve, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileOversightProfile};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, ReviewWindow, action_frame};
use fa_reference::action::consequence::oversight::human::HumanReviewPolicy;
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::{Verdict, commitment};
use fa_reference::{Error, Snapshot};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-clocked-reconcile-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("clocked reconciliation cleanup: {error}"); }
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
fn dispatched(root: &Directory, p: FileOversightProfile, reserved: bool, publish: bool) -> FileOversight {
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
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    if publish {
        assert_eq!(host.publish(host.revision(), 1), Ok(EndpointOutcome::Executed { resulting_version: 2 }));
    }
    host
}

#[test]
fn clocked_reconciliation_matches_original_time_then_query_without_resending() {
    for publish in [false, true] {
        let root = Directory::new();
        let control_root = Directory::new();
        let mut host = dispatched(&root, profile(), false, publish);
        let mut control = dispatched(&control_root, profile(), false, publish);
        let before = host.inspect();
        control.observe_time(control.revision(), ElapsedTick(2)).unwrap();
        let expected = control.reconcile(control.revision(), 1).unwrap();
        let actual = host.reconcile_attempt_at(host.revision(), 1, ElapsedTick(2)).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(host.revision(), before.revision + 2);
        assert_eq!(host.inspect().control, control.inspect().control);
        assert_eq!(host.inspect().executions, u64::from(publish));
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(host.inspect().control.ledger.available, 84);
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
    }
}

#[test]
fn one_remaining_slot_refuses_both_records_without_advancing_clock() {
    let baseline = Directory::new();
    let count = dispatched(&baseline, profile(), false, true).revision() as usize;
    for spare in [1, 2] {
        let root = Directory::new();
        let mut p = profile();
        p.delivery.limits.events = count + spare;
        let mut host = dispatched(&root, p.clone(), false, true);
        let before = host.inspect();
        let result = host.reconcile_attempt_at(before.revision, 1, ElapsedTick(2));
        if spare == 1 {
            assert_eq!(result, Err(Error::Limit.into()));
            assert_eq!(host.inspect(), before);
            assert!(host.storage_failure().is_none());
        } else {
            assert_eq!(result, Ok(Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 })));
            assert_eq!(host.revision(), before.revision + 2);
        }
        assert_eq!(FileOversight::read_publication(root.store(), &p).unwrap(), host.inspect());
    }
}

#[test]
fn live_current_clock_uses_only_one_remaining_record() {
    let baseline = Directory::new();
    let count = dispatched(&baseline, profile(), false, true).revision() as usize;
    let root = Directory::new();
    let mut p = profile();
    p.delivery.limits.events = count + 1;
    let mut host = dispatched(&root, p.clone(), false, true);
    let revision = host.revision();
    assert_eq!(host.reconcile_attempt_at(revision, 1, ElapsedTick(1)),
        Ok(Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 })));
    assert_eq!(host.revision(), revision + 1);
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(FileOversight::read_publication(root.store(), &p).unwrap(), host.inspect());
}

#[test]
fn stale_predecessor_tick_and_missing_attempt_leave_no_partial_time_record() {
    let root = Directory::new();
    let mut host = dispatched(&root, profile(), false, true);
    let before = host.inspect();
    for (revision, attempt, tick) in [(before.revision - 1, 1, 2),
        (before.revision, 1, 0), (before.revision, 999, 2)] {
        assert!(host.reconcile_attempt_at(revision, attempt, ElapsedTick(tick)).is_err());
        assert_eq!(host.inspect(), before);
        assert!(host.storage_failure().is_none());
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), before);
    }
    assert!(host.reconcile_attempt_at(before.revision, 1, ElapsedTick(2)).is_ok());
}

#[test]
fn ordinary_reconciliation_cannot_spend_the_terminal_reserve() {
    let baseline = Directory::new();
    let count = dispatched(&baseline, profile(), true, true).revision() as usize;
    let root = Directory::new();
    let mut p = profile();
    // The terminal reserve retains three records; only one ordinary record fits.
    p.delivery.limits.events = count + 4;
    let mut host = dispatched(&root, p.clone(), true, true);
    assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, 1);
    let before = host.inspect();
    assert_eq!(host.reconcile_attempt_at(before.revision, 1, ElapsedTick(2)), Err(Error::Limit.into()));
    assert_eq!(host.inspect(), before);
    assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, 1);
    assert_eq!(FileOversight::read_publication(root.store(), &p).unwrap(), before);
}

#[test]
fn recovered_clock_requires_a_fresh_record_even_when_the_saved_tick_matches() {
    let root = Directory::new();
    let host = dispatched(&root, profile(), false, true);
    drop(host);
    let (mut recovered, _reviewer) = FileOversight::open(root.store(), profile()).unwrap();
    assert!(!recovered.clock_ready());
    let before = recovered.revision();
    assert_eq!(recovered.reconcile_attempt_at(before, 1, ElapsedTick(1)),
        Ok(Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 })));
    assert_eq!(recovered.revision(), before + 2);
    assert_eq!(recovered.inspect().executions, 1);
    assert_eq!(recovered.inspect().control.ledger.charged, 16);
    assert_eq!(recovered.inspect().control.ledger.stages[&1], ActionState::Confirmed);
}

#[test]
fn unresolved_dispatch_stays_charged_across_repeated_queries() {
    let root = Directory::new();
    let mut host = dispatched(&root, profile(), false, false);
    for tick in [2, 3, 4] {
        host.reconcile_attempt_at(host.revision(), 1, ElapsedTick(tick)).unwrap();
        let state = host.inspect();
        assert_eq!(state.executions, 0);
        assert_eq!(state.control.ledger.charged, 16);
        assert_eq!(state.control.ledger.available, 84);
        assert!(matches!(state.control.ledger.stages[&1], ActionState::Dispatching | ActionState::Unknown));
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), state);
    }
}
