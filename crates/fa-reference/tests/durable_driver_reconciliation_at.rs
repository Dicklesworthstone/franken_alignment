//! Exercise the actual request registry and supervised query-only recovery path.
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits, JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileOversightProfile};
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase};
use fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition;
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

const REQUEST: u64 = 17;
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-driver-clocked-reconcile-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("driver reconciliation cleanup: {error}"); }
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
fn requested(root: &Directory, p: FileOversightProfile, publish: bool) -> FileOversight {
    let (mut host, reviewer) = FileOversight::create(root.store(), p.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let status = host.submit_request(host.revision(), REQUEST, ActionSpec { version: VERSION, scope: p.delivery.scope,
        target: Some(host.inspect().target), payload: b"visible".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 }, snapshot()).unwrap();
    let attempt = match status.disposition {
        FileRequestDisposition::Admitted { attempt, .. } => attempt,
        FileRequestDisposition::NotAdmitted(error) => panic!("fixture request refused: {error:?}"),
    };
    let action = host.request_action(REQUEST).unwrap().clone();
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
    host.record_inputs(host.revision(), attempt, 0, inputs.clone()).unwrap();
    host.begin_review(host.revision(), attempt, 101, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8),
    }, snapshot()).unwrap();
    let digest = commitment(101, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap();
    host.commit_review(host.revision(), 101, "reviewer", digest).unwrap();
    host.open_reveals(host.revision(), 101).unwrap();
    host.reveal_review(host.revision(), 101, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), 101, Some(&inputs), snapshot()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), attempt, &inputs, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), 1001, attempt, &inputs, ElapsedTick(10)).unwrap();
    let revision = host.revision();
    let human = reviewer.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    if publish {
        assert_eq!(host.publish(host.revision(), attempt), Ok(EndpointOutcome::Executed { resulting_version: 2 }));
    }
    host
}

#[test]
fn driver_preflights_the_whole_clocked_query_before_spending_the_last_slot() {
    let baseline = Directory::new();
    let count = requested(&baseline, profile(), true).revision() as usize;
    for spare in [1, 2] {
        let root = Directory::new();
        let mut p = profile();
        p.delivery.limits.events = count + spare;
        let host = requested(&root, p.clone(), true);
        let before = host.inspect();
        let (_port, mut driver) = host.into_supervised_driver();
        driver.resume_reconciliation(REQUEST).unwrap();
        let mut clocks = 0;
        let result = driver.step_with_evidence(|| { clocks += 1; ElapsedTick(2) },
            |_, _| panic!("query-only recovery called the evidence provider"), None);
        assert_eq!(clocks, 1);
        if spare == 1 {
            assert!(matches!(result, Err(JournalError::Contract(Error::Limit))));
            assert_eq!(driver.phase(), FileDriverPhase::AwaitingReconciliation { request: REQUEST });
            assert_eq!(driver.supervisor().host().unwrap().inspect(), before);
            assert!(driver.supervisor().host().unwrap().storage_failure().is_none());
        } else {
            assert!(matches!(result, Ok(FileDriverEvent::Reconciled { request: REQUEST,
                outcome: Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }) })));
            assert_eq!(driver.phase(), FileDriverPhase::Idle);
            assert_eq!(driver.supervisor().host().unwrap().revision(), before.revision + 2);
        }
        let after = driver.supervisor().host().unwrap().inspect();
        assert_eq!(after.executions, 1);
        assert_eq!(after.control.ledger.charged, 16);
        assert!(driver.helpers_reaped());
        assert_eq!(FileOversight::read_publication(root.store(), &p).unwrap(), after);
    }
}

#[test]
fn driver_same_tick_reconciliation_needs_no_second_event_slot() {
    let baseline = Directory::new();
    let count = requested(&baseline, profile(), true).revision() as usize;
    let root = Directory::new();
    let mut p = profile();
    p.delivery.limits.events = count + 1;
    let host = requested(&root, p.clone(), true);
    let before = host.revision();
    let (_port, mut driver) = host.into_supervised_driver();
    driver.resume_reconciliation(REQUEST).unwrap();
    let event = driver.step_with_evidence(|| ElapsedTick(1),
        |_, _| panic!("recovery requested new evidence"), None).unwrap();
    assert!(matches!(event, FileDriverEvent::Reconciled { request: REQUEST, .. }));
    assert_eq!(driver.supervisor().host().unwrap().revision(), before + 1);
    assert_eq!(driver.phase(), FileDriverPhase::Idle);
    assert_eq!(FileOversight::read_publication(root.store(), &p).unwrap(), driver.supervisor().host().unwrap().inspect());
}

#[test]
fn stale_clock_refusal_retains_the_job_and_a_valid_retry_still_reconciles() {
    let root = Directory::new();
    let host = requested(&root, profile(), true);
    let before = host.inspect();
    let (_port, mut driver) = host.into_supervised_driver();
    driver.resume_reconciliation(REQUEST).unwrap();
    let refused = driver.step_with_evidence(|| ElapsedTick(0),
        |_, _| panic!("stale recovery called evidence provider"), None);
    assert!(matches!(refused, Err(JournalError::Contract(Error::Stale))));
    assert_eq!(driver.phase(), FileDriverPhase::AwaitingReconciliation { request: REQUEST });
    assert_eq!(driver.supervisor().host().unwrap().inspect(), before);
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), before);
    let event = driver.step_with_evidence(|| ElapsedTick(2),
        |_, _| panic!("recovery retry called evidence provider"), None).unwrap();
    assert!(matches!(event, FileDriverEvent::Reconciled { request: REQUEST, .. }));
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 1);
}

#[test]
fn reopened_driver_queries_with_fresh_time_without_reviewer_or_helper_inputs() {
    let root = Directory::new();
    drop(requested(&root, profile(), true));
    let (host, _reviewer) = FileOversight::open(root.store(), profile()).unwrap();
    assert!(!host.clock_ready());
    let before = host.revision();
    let (_port, mut driver) = host.into_supervised_driver();
    driver.resume_reconciliation(REQUEST).unwrap();
    let event = driver.step_with_evidence(|| ElapsedTick(1),
        |_, _| panic!("reopened recovery requested unavailable inputs"), None).unwrap();
    assert!(matches!(event, FileDriverEvent::Reconciled { request: REQUEST,
        outcome: Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }) }));
    let after = driver.supervisor().host().unwrap().inspect();
    assert_eq!(after.revision, before + 2);
    assert_eq!(after.executions, 1);
    assert_eq!(after.control.ledger.charged, 16);
    assert_eq!(driver.phase(), FileDriverPhase::Idle);
    assert!(driver.helpers_reaped());
}

#[test]
fn repeated_driver_queries_do_not_publish_or_refund_an_unknown_dispatch() {
    let root = Directory::new();
    let host = requested(&root, profile(), false);
    let (_port, mut driver) = host.into_supervised_driver();
    for tick in [2, 3, 4] {
        driver.resume_reconciliation(REQUEST).unwrap();
        let event = driver.step_with_evidence(|| ElapsedTick(tick),
            |_, _| panic!("unknown-effect recovery requested new authority"), None).unwrap();
        assert!(matches!(event, FileDriverEvent::Reconciled { request: REQUEST, .. }));
        let host = driver.supervisor().host().unwrap();
        let state = host.inspect();
        assert_eq!(state.executions, 0);
        assert_eq!(state.control.ledger.charged, 16);
        assert_eq!(state.control.ledger.available, 84);
        assert!(matches!(host.request_status(REQUEST).unwrap().disposition,
            FileRequestDisposition::Admitted { stage: ActionState::Dispatching | ActionState::Unknown, .. }));
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), state);
    }
}
