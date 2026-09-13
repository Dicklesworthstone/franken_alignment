//! Original broker transitions and real canonical replacements under I/O faults.
//! These are deterministic barrier scenarios, not hardware power-cut evidence.
use super::*;
use super::super::JournalLimits;
use crate::action::{ActionState, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{HelperContract, action_frame};
use crate::action::consequence::oversight::human::HumanDisposition;
use crate::evidence_view::AuthorizationProjection;
use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use crate::reducer::Caps;
use crate::round::commitment;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
    JournalIo::Rename, JournalIo::DirectorySync];
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-observed-barrier-{}-{time}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("observed test cleanup: {error}"); }
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
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) } }
fn create(root: &Directory) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = FileOversight::create(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, reviewer)
}
fn prepared(host: &mut FileOversight) -> (FrozenAction, CommitteeInput, FilePermit, FileHumanRequest) {
    let action = host.propose(host.revision(), 1, ActionSpec { version: VERSION, scope: profile().delivery.scope,
        target: Some(host.inspect().target), payload: b"visible".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 }, snapshot()).unwrap();
    let contracts = profile().committee;
    let helper = &contracts.members()["reviewer"];
    let mut bytes = action_frame(&action);
    let boundary = bytes.len();
    bytes.extend_from_slice(helper.question());
    let end = bytes.len();
    let actual = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
    ], Vec::new()).unwrap();
    let manifest = EvidenceViewManifest::new(actual, AuthorizationProjection { projection_id: 7,
        policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new() }, Vec::new()).unwrap();
    let inputs = CommitteeInput::capture(&action, &contracts, BTreeMap::from([("reviewer".to_owned(), manifest)])).unwrap();
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
    (action, inputs, automatic, request)
}
fn canonical(host: &FileOversight) -> Machine {
    let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    let events = journal::decode(&host.profile, host.store.identity(), &bytes).unwrap();
    Machine::replay(&host.profile, &events).unwrap()
}
fn check_failure(error: JournalError, stage: JournalIo) {
    let JournalError::Io(failure) = error else { panic!("expected the selected storage fault"); };
    assert_eq!(failure.operation, stage);
    assert_eq!(failure.replacement_may_be_visible, matches!(stage, JournalIo::Rename | JournalIo::DirectorySync));
}

#[test]
fn approval_failure_never_returns_a_key_and_reopening_revokes_even_a_visible_unacknowledged_approval() {
    for stage in BARRIERS {
        let root = Directory::new();
        let (mut host, reviewer) = create(&root);
        let (_, _, _, request) = prepared(&mut host);
        let before = host.inspect();
        host.store.fail_once(stage);
        let revision = host.revision();
        check_failure(reviewer.approve(&mut host, revision, &request).unwrap_err(), stage);
        assert_eq!(host.inspect(), before);
        assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Pending);
        assert!(!host.clock_ready());
        let revision = host.revision();
        assert_eq!(reviewer.revoke_all(&mut host, revision).unwrap_err(), JournalError::Unavailable);
        let disk = canonical(&host);
        let expected = if stage == JournalIo::DirectorySync { HumanDisposition::Approved } else { HumanDisposition::Pending };
        assert_eq!(disk.broker.human_status(1001).unwrap().disposition, expected);
        assert_eq!(disk.broker.inspect().ledger.reserved, 16);
        drop(host);
        let (mut recovered, role) = FileOversight::open(root.store(), profile()).unwrap();
        assert_eq!(recovered.human_status(1001).unwrap().disposition, HumanDisposition::Revoked);
        assert_eq!(recovered.inspect().control.ledger.available, 100);
        assert_eq!(recovered.inspect().executions, 0);
        recovered.observe_time(recovered.revision(), ElapsedTick(2)).unwrap();
        let request = recovered.human_request(1001).unwrap();
        let revision = recovered.revision();
        assert!(role.approve(&mut recovered, revision, &request).is_err());
    }
}

#[test]
fn dispatch_failure_cannot_expose_candidate_consumption_or_erase_a_durable_unknown_charge() {
    for stage in BARRIERS {
        let root = Directory::new();
        let (mut host, reviewer) = create(&root);
        let (action, inputs, automatic, request) = prepared(&mut host);
        let revision = host.revision();
        let human = reviewer.approve(&mut host, revision, &request).unwrap();
        let before = host.inspect();
        host.store.fail_once(stage);
        check_failure(host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap_err(), stage);
        assert_eq!(host.inspect(), before);
        assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Approved);
        let disk = canonical(&host);
        let replaced = stage == JournalIo::DirectorySync;
        assert_eq!(disk.broker.inspect().ledger.charged, if replaced { 16 } else { 0 });
        assert_eq!(disk.broker.human_status(1001).unwrap().disposition,
            if replaced { HumanDisposition::Consumed } else { HumanDisposition::Approved });
        drop(host);
        let (mut recovered, _) = FileOversight::open(root.store(), profile()).unwrap();
        assert_eq!(recovered.inspect().control.ledger.reserved, 0);
        assert_eq!(recovered.inspect().control.ledger.charged, if replaced { 16 } else { 0 });
        assert_eq!(recovered.inspect().control.ledger.stages[&1], if replaced { ActionState::Unknown } else { ActionState::Cancelled });
        recovered.observe_time(recovered.revision(), ElapsedTick(2)).unwrap();
        let outcomes = recovered.reconcile_pending(recovered.revision()).unwrap();
        if replaced {
            assert_eq!(outcomes[&1], Ok(Reconciliation::AwaitingResolution));
            assert_eq!(recovered.inspect().control.ledger.charged, 16);
            recovered.seal_unexecuted(recovered.revision(), 1).unwrap();
        } else { assert!(outcomes.is_empty()); }
        assert_eq!(recovered.inspect().control.ledger.available, 100);
        assert_eq!(recovered.inspect().executions, 0);
        assert!(recovered.publish(recovered.revision(), 1).is_err());
    }
}

#[test]
fn publication_failure_recovers_the_actual_replacement_without_returning_or_reissuing_a_receipt() {
    for stage in BARRIERS {
        let root = Directory::new();
        let (mut host, reviewer) = create(&root);
        let (action, inputs, automatic, request) = prepared(&mut host);
        let revision = host.revision();
        let human = reviewer.approve(&mut host, revision, &request).unwrap();
        host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
        let before = host.inspect();
        host.store.fail_once(stage);
        check_failure(host.publish(host.revision(), 1).unwrap_err(), stage);
        assert_eq!(host.inspect(), before);
        let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
        let replaced = stage == JournalIo::DirectorySync;
        assert_eq!(disk.executions, u64::from(replaced));
        assert_eq!(disk.control.ledger.charged, 16);
        assert_eq!(disk.payload.as_slice(), if replaced { b"visible".as_slice() } else { b"initial".as_slice() });
        drop(host);
        let (mut recovered, _) = FileOversight::open(root.store(), profile()).unwrap();
        recovered.observe_time(recovered.revision(), ElapsedTick(2)).unwrap();
        let outcomes = recovered.reconcile_pending(recovered.revision()).unwrap();
        if replaced {
            assert_eq!(outcomes[&1], Ok(Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 })));
        } else {
            assert_eq!(outcomes[&1], Ok(Reconciliation::AwaitingResolution));
        }
        assert_eq!(recovered.inspect().control.ledger.charged, 16);
        assert_eq!(recovered.inspect().executions, u64::from(replaced));
        assert!(recovered.publish(recovered.revision(), 1).is_err());
    }
}

#[test]
fn framed_one_key_events_are_refused_and_illegal_original_histories_cannot_fabricate_effects() {
    let p = profile();
    let path = Path::new("/operator-owned/observed-publication");
    for event in [BaseEvent::Authorize(1, snapshot()), BaseEvent::Dispatch(1, snapshot())] {
        assert!(journal::encode(&p, path, &[Event::Core(event)]).is_err());
    }
    let illegal = vec![Event::Core(BaseEvent::Time(ElapsedTick(1))), Event::Core(BaseEvent::Publish(1))];
    let encoded = journal::encode(&p, path, &illegal).unwrap();
    let decoded = journal::decode(&p, path, &encoded).unwrap();
    assert!(Machine::replay(&p, &decoded).is_err());
    let valid = vec![Event::Core(BaseEvent::Time(ElapsedTick(1))), Event::Core(BaseEvent::Sweep)];
    let encoded = journal::encode(&p, path, &valid).unwrap();
    for end in 0..encoded.len() { assert!(journal::decode(&p, path, &encoded[..end]).is_err()); }
    let decoded = journal::decode(&p, path, &encoded).unwrap();
    let state = Machine::replay(&p, &decoded).unwrap().snapshot(decoded.len());
    assert_eq!(state.control.ledger.available, 100);
    assert_eq!(state.executions, 0);
}
