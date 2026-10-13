//! Unwinds through the real actor gateway, socket congress and durable driver.
//! Native-child tests establish direct-child custody, not descendant containment.
use super::*;
use super::super::{FileHumanReviewer, FileOversightProfile};
use super::super::super::{FileDeliveryProfile, JournalLimits};
use super::super::super::requests::actor::FileActorTicket;
use crate::action::{Purpose, ResolvedTarget, Scope};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{HelperContract, action_frame};
use crate::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge};
use crate::action::consequence::oversight::helper_client::{ClientPhase, HelperClient};
use crate::action::consequence::oversight::helper_processes::HelperProgram;
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use crate::reducer::Caps;
use crate::round::Verdict;
use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind, panic_any};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const REQUEST: u64 = 901;
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-driver-unwind-{}-{time}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("driver unwind cleanup: {error}"); }
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
            InputProfileBinding { profile_id: 1, profile_bytes: b"driver-unwind-v1".to_vec(),
                tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec(),
        ).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) } }
fn evidence(inputs: &CommitteeInput) -> DriverEvidence {
    DriverEvidence { snapshot: snapshot(), inputs: Some(inputs.clone()) }
}
struct Session {
    driver: FileSupervisedDriver,
    port: FileActorPort<FileOversight>,
    ticket: FileActorTicket<FileOversight>,
    reviewer: FileHumanReviewer,
    inputs: CommitteeInput,
}
impl Session {
    fn admit(root: &Directory) -> Self {
        let (mut host, reviewer) = FileOversight::create(root.store(), profile()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let (port, mut driver) = host.into_supervised_driver();
        let (revision, target, epoch) = {
            let host = driver.supervisor().host().unwrap();
            (host.revision(), host.inspect().target, host.inspect().control.ledger.epoch)
        };
        driver.supervisor_mut().set_snapshot(revision, Some(snapshot())).unwrap();
        let ticket = port.submit(REQUEST, &ActorProposal { target, payload: b"visible".to_vec(),
            expected_policy_epoch: epoch, deadline: ElapsedTick(100), units: 16 }).unwrap();
        let inputs = {
            let host = driver.supervisor().host().unwrap();
            let action = host.request_action(REQUEST).unwrap();
            let contracts = &host.profile.committee;
            let helper = &contracts.members()["reviewer"];
            let mut bytes = action_frame(action);
            let boundary = bytes.len(); bytes.extend_from_slice(helper.question()); let end = bytes.len();
            let actual = ActualHelperInput::new(bytes, helper.profile_at(epoch), vec![
                SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
                SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
            ], Vec::new()).unwrap();
            let view = EvidenceViewManifest::new(actual, AuthorizationProjection { projection_id: 7,
                policy_epoch: epoch, projected_originals: Vec::new() }, Vec::new()).unwrap();
            CommitteeInput::capture(action, contracts, BTreeMap::from([("reviewer".to_owned(), view)])).unwrap()
        };
        Self { driver, port, ticket, reviewer, inputs }
    }
    fn launch<W>(&self, worker: W) -> FileDriverLaunch<W> {
        FileDriverLaunch { request: REQUEST, round: 101, evidence_root: [9; 32],
            window: ReviewWindow { commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) },
            expected_input_revision: 0, inputs: self.inputs.clone(),
            workers: BTreeMap::from([("reviewer".to_owned(), worker)]), limits: HelperLimits::default() }
    }
    fn start_socket(&mut self) -> HelperClient<UnixStream> {
        let (server, peer) = UnixStream::pair().unwrap();
        let client = HelperClient::from_unix(peer, profile().committee.members()["reviewer"].profile_at(0)).unwrap();
        self.driver.start_review(self.launch(server), snapshot(), || ElapsedTick(1)).unwrap();
        client
    }
    fn step(&mut self, human: Option<&FileHumanPermit>) -> FileDriverEvent {
        let inputs = &self.inputs;
        self.driver.step_with_evidence(|| ElapsedTick(1), |_, _| Ok(evidence(inputs)), human).unwrap()
    }
    fn complete(&mut self, client: &mut HelperClient<UnixStream>) {
        for _ in 0..64 {
            match self.step(None) {
                FileDriverEvent::ReviewApplied { receipt, .. } => {
                    assert_eq!(receipt.policy.control.decision.consequence, Consequence::Continue);
                    assert_eq!(self.driver.phase(), FileDriverPhase::AwaitingDispatch { request: REQUEST });
                    return;
                }
                FileDriverEvent::Workers { .. } => advance_client(client),
                event => panic!("unexpected review result: {event:?}"),
            }
        }
        panic!("bounded original socket review did not finish");
    }
    fn approve(&mut self) -> FileHumanPermit {
        let request = self.driver.request_human_approval(1001, &self.inputs, ElapsedTick(40), ElapsedTick(1)).unwrap();
        let mut host = self.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        self.reviewer.approve(&mut host, revision, &request).unwrap()
    }
    fn canonical(&self) -> Vec<u8> {
        let host = self.driver.supervisor().host().unwrap();
        host.store.read(host.profile.delivery.limits.bytes).unwrap()
    }
}
fn advance_client(client: &mut HelperClient<UnixStream>) {
    client.step().unwrap();
    if client.phase() == ClientPhase::NeedsInference { client.respond(Verdict::Allow, b"salt").unwrap(); }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cut { Clock(usize), Evidence(usize) }
fn assert_payload(payload: Box<dyn Any + Send>, cut: Cut) {
    assert_eq!(payload.downcast_ref::<Cut>(), Some(&cut), "original panic payload must survive cleanup");
}
fn assert_idle_without_callbacks(driver: &mut FileSupervisedDriver) {
    assert_eq!(driver.phase(), FileDriverPhase::Idle);
    assert_eq!(driver.next_review_deadline(), None);
    assert!(matches!(driver.step_with_evidence(|| panic!("idle clock"),
        |_, _| panic!("idle provider"), None).unwrap(), FileDriverEvent::Idle));
}

#[test]
fn review_entry_clock_unwind_closes_sockets_without_inventing_durable_progress() {
    let root = Directory::new(); let mut session = Session::admit(&root);
    let mut client = session.start_socket();
    let before = session.canonical();
    let payload = catch_unwind(AssertUnwindSafe(|| session.driver.step_with_evidence(
        || panic_any(Cut::Clock(1)), |_, _| panic!("provider must not run"), None))).unwrap_err();
    assert_payload(payload, Cut::Clock(1));
    assert_idle_without_callbacks(&mut session.driver);
    assert_eq!(session.canonical(), before);
    assert!(client.step().is_err(), "the failed review must not retain a live socket");
    assert!(matches!(session.port.poll(&session.ticket), Knowledge::Pending { request: REQUEST }));
    let host = session.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert!(host.storage_failure().is_none());
}

#[test]
fn evidence_unwind_after_real_reveals_never_leaves_a_reusable_review() {
    let root = Directory::new(); let mut session = Session::admit(&root);
    let mut client = session.start_socket();
    let mut failed = false;
    for _ in 0..64 {
        let result = catch_unwind(AssertUnwindSafe(|| session.driver.step_with_evidence(
            || ElapsedTick(1), |_, _| panic_any(Cut::Evidence(1)), None)));
        match result {
            Err(payload) => { assert_payload(payload, Cut::Evidence(1)); failed = true; break; }
            Ok(Ok(FileDriverEvent::Workers { .. })) => advance_client(&mut client),
            other => panic!("unexpected pre-capture result: {other:?}"),
        }
    }
    assert!(failed, "the original completed socket round must reach the provider");
    assert_idle_without_callbacks(&mut session.driver);
    {
        let host = session.driver.supervisor().host().unwrap();
        let (attempt, state) = admitted(host.request_status(REQUEST).unwrap()).unwrap();
        assert_eq!(state, ActionState::Reviewing);
        assert!(!host.inspect().control.decisions.contains_key(&attempt));
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.available, 100);
    }
    drop(client); drop(session);
    let (mut reopened, _) = FileOversight::open(root.store(), profile()).unwrap();
    reopened.observe_time(reopened.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(admitted(reopened.request_status(REQUEST).unwrap()).unwrap().1, ActionState::Cancelled);
    assert_eq!(reopened.inspect().executions, 0);
}

#[test]
fn ready_callback_unwinds_preserve_reservations_until_original_explicit_cancellation() {
    for cut in [Cut::Clock(1), Cut::Clock(2), Cut::Clock(3), Cut::Evidence(1), Cut::Evidence(2)] {
        let root = Directory::new(); let mut session = Session::admit(&root);
        let mut client = session.start_socket(); session.complete(&mut client);
        let human = session.approve();
        let inputs = session.inputs.clone(); let mut clocks = 0; let mut captures = 0;
        let payload = catch_unwind(AssertUnwindSafe(|| session.driver.step_with_evidence(|| {
            clocks += 1; if cut == Cut::Clock(clocks) { panic_any(cut); } ElapsedTick(1)
        }, |_, _| {
            captures += 1; if cut == Cut::Evidence(captures) { panic_any(cut); } Ok(evidence(&inputs))
        }, Some(&human)))).unwrap_err();
        assert_payload(payload, cut);
        assert_idle_without_callbacks(&mut session.driver);
        let reserved = matches!(cut, Cut::Clock(3) | Cut::Evidence(2));
        {
            let mut host = session.driver.supervisor_mut().host_mut().unwrap();
            assert_eq!(admitted(host.request_status(REQUEST).unwrap()).unwrap().1,
                if reserved { ActionState::Authorized } else { ActionState::Reviewing });
            assert_eq!(host.inspect().control.ledger.available, if reserved { 84 } else { 100 });
            assert_eq!(host.inspect().executions, 0);
            assert!(host.storage_failure().is_none());
            // Only the existing explicit cancellation may release the held units.
            let revision = host.revision(); host.cancel_request(revision, REQUEST).unwrap();
            assert_eq!(host.inspect().control.ledger.available, 100);
        }
        assert!(matches!(session.port.poll(&session.ticket), Knowledge::Known {
            value: ActorOutcome::CancelledBeforeDispatch, .. }));
    }
}

#[test]
fn prepublication_and_reconciliation_unwinds_preserve_query_only_custody() {
    for execute in [false, true] {
        let root = Directory::new(); let mut session = Session::admit(&root);
        let mut client = session.start_socket(); session.complete(&mut client);
        let human = session.approve();
        assert!(matches!(session.step(Some(&human)), FileDriverEvent::Dispatched { .. }));
        if execute { assert_executed(session.step(None)); }
        let before = session.canonical();
        let payload = catch_unwind(AssertUnwindSafe(|| session.driver.step_with_evidence(
            || panic_any(Cut::Clock(1)), |_, _| panic!("no evidence before a failed clock"), None))).unwrap_err();
        assert_payload(payload, Cut::Clock(1));
        assert_eq!(session.driver.phase(), FileDriverPhase::AwaitingReconciliation { request: REQUEST });
        assert_eq!(session.canonical(), before);
        assert_eq!(session.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
        let event = session.driver.step_with_evidence(|| ElapsedTick(1),
            |_, _| panic!("query-only recovery must not call the provider"), None).unwrap();
        assert!(matches!(event, FileDriverEvent::Reconciled { request: REQUEST, .. }));
        assert_eq!(session.driver.supervisor().host().unwrap().inspect().executions, if execute { 1 } else { 0 });
        if execute {
            assert!(matches!(session.port.poll(&session.ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
        }
        assert_idle_without_callbacks(&mut session.driver);
    }
}
fn assert_executed(event: FileDriverEvent) {
    let outcome = match event {
        FileDriverEvent::Published { outcome, .. } => outcome,
        FileDriverEvent::PublicationChecked { publication, .. } => publication.outcome,
        other => panic!("expected original publication: {other:?}"),
    };
    assert_eq!(outcome, EndpointOutcome::Executed { resulting_version: 2 });
}

#[test]
fn unchanged_callbacks_still_complete_the_original_two_key_publication() {
    let root = Directory::new(); let mut session = Session::admit(&root);
    let mut client = session.start_socket(); session.complete(&mut client);
    assert!(matches!(session.step(None), FileDriverEvent::AwaitingHuman { request: REQUEST }));
    assert_eq!(session.driver.supervisor().host().unwrap().inspect().executions, 0);
    let human = session.approve();
    assert!(matches!(session.step(Some(&human)), FileDriverEvent::Dispatched { request: REQUEST, .. }));
    assert_executed(session.step(None));
    assert!(matches!(session.step(None), FileDriverEvent::Reconciled {
        outcome: Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }), .. }));
    assert!(matches!(session.port.poll(&session.ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert_eq!(session.driver.supervisor().host().unwrap().inspect().executions, 1);
    assert_eq!(session.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
}

#[test]
fn entry_clock_unwind_retains_the_original_native_child_through_release_and_reaping() {
    let root = Directory::new(); let mut session = Session::admit(&root);
    let program = HelperProgram::new(PathBuf::from("/bin/sleep"), root.0.clone(),
        vec!["30".into()], BTreeMap::new()).unwrap();
    session.driver.start_process_review(session.launch(program), snapshot(), || ElapsedTick(1)).unwrap();
    let original = session.driver.helper_processes()["reviewer"];
    assert!(!original.stop_requested); assert!(original.exit.is_none());
    let before = session.canonical();
    let payload = catch_unwind(AssertUnwindSafe(|| session.driver.step_with_evidence(
        || panic_any(Cut::Clock(1)), |_, _| panic!("no native provider"), None))).unwrap_err();
    assert_payload(payload, Cut::Clock(1));
    assert_eq!(session.canonical(), before);
    assert_idle_without_callbacks(&mut session.driver);
    let retained = session.driver.helper_processes()["reviewer"];
    assert_eq!(retained.pid, original.pid); assert!(retained.stop_requested);
    let released = session.driver.release();
    let mut children = released.children.expect("original direct-child ownership must survive release");
    assert_eq!(children.statuses()["reviewer"].pid, original.pid);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !children.all_reaped() && std::time::Instant::now() < deadline {
        children.reap(); std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(children.all_reaped(), "original stopped child must be reaped, not forgotten");
    assert!(children.statuses()["reviewer"].exit.is_some());
    assert_eq!(released.supervisor.host().unwrap().inspect().executions, 0);
}
