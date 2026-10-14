//! Actual gateway, socket protocol, original reviews, both keys and sink.
//! Fixed manual helper responses are controls, not learned detection evidence.
use super::*;
use super::super::{FileDriverEvent, FileDriverLaunch, FileDriverPhase};
use crate::action::{ActionState, ElapsedTick, Purpose, ResolvedTarget, Scope};
use crate::action::consequence::Consequence;
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::EndpointOutcome;
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits};
use crate::action::consequence::delivery::persistent::observed::{FileHumanPermit, FileHumanReviewer, FileOversightProfile};
use crate::action::consequence::delivery::persistent::observed::mediation::source::{
    TopologySourceRefusal, TopologySourceWork, TOPOLOGY_REFRESH_READ_BYTES};
use crate::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use crate::action::consequence::delivery::persistent::requests::actor::FileActorPort;
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, ReviewWindow, action_frame};
use crate::action::consequence::oversight::actor::ActorProposal;
use crate::action::consequence::oversight::helper_client::{ClientPhase, HelperClient};
use crate::action::consequence::oversight::helper_workers::HelperLimits;
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::action::consequence::oversight::supervised::DriverEvidence;
use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use crate::reducer::Caps;
use crate::round::Verdict;
use crate::Snapshot;
use std::{collections::BTreeMap, fs, os::unix::net::UnixStream, path::PathBuf,
    sync::atomic::{AtomicU64, Ordering}};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-driver-topology-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
    fn topology(&self) -> PathBuf { self.0.join("topology.json") }
    fn inventory(&self) -> PathBuf { self.0.join("inventory.json") }
    fn bytes(&self) -> Vec<u8> { fs::read(self.store().join("delivery.bin")).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("driver topology cleanup: {error}"); }
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

const INVENTORY: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/authority_topology_inventory.json"));
fn document(generation: u64, checks: u64, read_bytes: u64, mode: u8, budget: usize) -> String {
    let extra = if mode == 1 {
        r#",{"id":3,"from":1,"to":4,"channel":"dispatch","route":"publication","provenance":103}"#
    } else { "" };
    let second = if mode == 2 { "" } else {
        r#",{"id":2,"from":2,"to":4,"channel":"dispatch","route":"publication","provenance":102}"#
    };
    format!(r#"{{"schema":"fa.authority-topology/1","generation":{generation},"inventory_generation":{generation},
        "complete_inventory_generation":{generation},
        "scope":{{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5,"purpose":"effect"}},
        "target":{{"adapter":10,"object":11,"contract_version":1,"expected_version":1,"generation":1}},
        "inventory_scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"file-publication",
        "nodes":[{{"id":1,"kind":"actor"}},{{"id":2,"kind":"enforcer"}},{{"id":4,"kind":"sink"}}],
        "edges":[{{"id":1,"from":1,"to":2,"channel":"dispatch","route":"publication","provenance":101}}{second}{extra}],
        "enforcers":[{{"node":2,"adapter":10,"contract_version":1,"generation":1,"provenance":102}}],
        "gates":[2],"check_edge_budget":{budget},"source_limits":{{"checks":{checks},"read_bytes":{read_bytes}}}}}"#)
}
fn text(generation: u64) -> String {
    document(generation, 32, 32 * TOPOLOGY_REFRESH_READ_BYTES, 0,
        crate::action::consequence::mediation::MAX_CHECK_EDGE_VISITS)
}
fn loaded(document: &str) -> LoadedAuthorityTopology {
    LoadedAuthorityTopology::from_json_bytes(document.as_bytes(), INVENTORY.as_bytes()).unwrap()
}

fn evidence(inputs: &CommitteeInput) -> DriverEvidence {
    DriverEvidence { snapshot: snapshot(), inputs: Some(inputs.clone()) }
}
struct Case {
    driver: FileSupervisedDriver,
    port: FileActorPort<FileOversight>,
    human: FileHumanReviewer,
    root: Directory,
}
struct Reviewed { request: u64, attempt: u64, inputs: CommitteeInput, human: FileHumanPermit }
impl Case {
    fn unselected(checks: u64) -> (Self, FileDriverTopology) {
        let root = Directory::new();
        let document = document(1, checks, checks * TOPOLOGY_REFRESH_READ_BYTES, 0,
            crate::action::consequence::mediation::MAX_CHECK_EDGE_VISITS);
        fs::write(root.topology(), &document).unwrap(); fs::write(root.inventory(), INVENTORY).unwrap();
        let expected = loaded(&document);
        let (mut host, human) = FileOversight::create(root.store(), profile()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let observer = host.enable_mediation(host.revision(), expected.graph().clone()).unwrap();
        let source = observer.bind_topology_source(&host, host.revision(), root.topology(),
            root.inventory(), expected).unwrap();
        let (port, driver) = host.into_supervised_driver();
        (Self { driver, port, human, root }, FileDriverTopology::new(source))
    }
    fn new(checks: u64) -> Self {
        let (mut case, selected) = Self::unselected(checks);
        case.driver.require_topology(case.revision(), selected).unwrap();
        let report = case.driver.refresh_topology(case.revision()).unwrap();
        assert!(report.refusal.is_none()); assert_eq!(report.work.checks, 1);
        case
    }
    fn revision(&self) -> u64 { self.driver.supervisor().host().unwrap().revision() }
    fn work(&self) -> TopologySourceWork { self.driver.topology().unwrap().source().work() }
    fn last(&self) -> &DriverTopologyObservation { self.driver.topology().unwrap().last_observation().unwrap() }
    fn review(&mut self, request: u64) -> Reviewed {
        let work = self.work();
        let (target, epoch) = {
            let host = self.driver.supervisor().host().unwrap();
            (host.inspect().target, host.inspect().control.ledger.epoch)
        };
        let revision = self.revision();
        self.driver.supervisor_mut().set_snapshot(revision, Some(snapshot())).unwrap();
        let _ticket = self.port.submit(request, &ActorProposal { target, payload: b"visible".to_vec(),
            expected_policy_epoch: epoch, deadline: ElapsedTick(100), units: 16 }).unwrap();
        let (attempt, inputs) = {
            let host = self.driver.supervisor().host().unwrap();
            let (attempt, _) = super::super::admitted(host.request_status(request).unwrap()).unwrap();
            let action = host.request_action(request).unwrap();
            let contracts = &host.profile.committee; let helper = &contracts.members()["reviewer"];
            let mut bytes = action_frame(action); let boundary = bytes.len();
            bytes.extend_from_slice(helper.question()); let end = bytes.len();
            let actual = ActualHelperInput::new(bytes, helper.profile_at(epoch), vec![
                SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
                SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
            ], Vec::new()).unwrap();
            let view = EvidenceViewManifest::new(actual, AuthorizationProjection {
                projection_id: 7, policy_epoch: epoch, projected_originals: Vec::new(),
            }, Vec::new()).unwrap();
            (attempt, CommitteeInput::capture(action, contracts,
                BTreeMap::from([("reviewer".to_owned(), view)])).unwrap())
        };
        let (server, peer) = UnixStream::pair().unwrap();
        let mut client = HelperClient::from_unix(peer, profile().committee.members()["reviewer"].profile_at(epoch)).unwrap();
        self.driver.start_review(FileDriverLaunch { request, round: request + 100,
            evidence_root: [9; 32], window: ReviewWindow { commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) },
            expected_input_revision: 0, inputs: inputs.clone(),
            workers: BTreeMap::from([("reviewer".to_owned(), server)]), limits: HelperLimits::default(),
        }, snapshot(), || ElapsedTick(1)).unwrap();
        let mut completed = false;
        for _ in 0..64 {
            let event = self.driver.step_with_evidence(|| ElapsedTick(1), |_, _| Ok(evidence(&inputs)), None).unwrap();
            match event {
                FileDriverEvent::ReviewApplied { receipt, .. } => {
                    assert_eq!(receipt.policy.control.decision.consequence, Consequence::Continue);
                    completed = true; break;
                }
                FileDriverEvent::Workers { .. } => {
                    client.step().unwrap();
                    if client.phase() == ClientPhase::NeedsInference { client.respond(Verdict::Allow, b"salt").unwrap(); }
                }
                other => panic!("original socket review: {other:?}"),
            }
        }
        assert!(completed);
        let approval = self.driver.request_human_approval(request + 1000, &inputs,
            ElapsedTick(40), ElapsedTick(1)).unwrap();
        let human = {
            let mut host = self.driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
            self.human.approve(&mut host, revision, &approval).unwrap()
        };
        assert_eq!(self.work(), work, "review and human request cannot spend first-publication source work");
        Reviewed { request, attempt, inputs, human }
    }
    fn dispatch(&mut self, reviewed: &Reviewed) {
        let before = self.work();
        assert!(matches!(self.driver.step_with_evidence(|| ElapsedTick(1),
            |_, _| Ok(evidence(&reviewed.inputs)), Some(&reviewed.human)).unwrap(),
            FileDriverEvent::Dispatched { request, attempt }
                if request == reviewed.request && attempt == reviewed.attempt));
        assert_eq!(self.work(), before);
    }
    fn reconcile(&mut self, reviewed: &Reviewed) {
        let before = self.work(); let observation = self.last().clone();
        assert!(matches!(self.driver.step_with_evidence(|| ElapsedTick(1),
            |_, _| panic!("receipt reconciliation must not read evidence"), None).unwrap(),
            FileDriverEvent::Reconciled { request, .. } if request == reviewed.request));
        assert_eq!(self.work(), before); assert_eq!(self.last(), &observation);
        assert_eq!(self.driver.phase(), FileDriverPhase::Idle);
    }
}

#[test]
fn original_socket_review_and_both_keys_publish_two_requests_with_one_retained_source() {
    let mut case = Case::new(8);
    for (request, expected_checks) in [(901, 2), (902, 3)] {
        let reviewed = case.review(request); case.dispatch(&reviewed);
        let event = case.driver.step_with_evidence(|| ElapsedTick(1), |_, _| Ok(evidence(&reviewed.inputs)), None).unwrap();
        assert!(matches!(event, FileDriverEvent::PublicationChecked { publication, source_failure: None, .. }
            if matches!(publication.outcome, EndpointOutcome::Executed { .. })));
        assert_eq!(case.work().checks, expected_checks);
        assert_eq!(case.driver.topology().unwrap().source().last_operation(), expected_checks);
        assert_eq!(case.last().request, Some(request)); assert_eq!(case.last().attempt, Some(reviewed.attempt));
        let DriverTopologyOutcome::Finished(Ok(report)) = &case.last().outcome else { panic!("native source result"); };
        assert!(report.refusal.is_none());
        case.reconcile(&reviewed);
        let host = case.driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().executions, expected_checks - 1);
        assert_eq!(host.inspect().control.ledger.stages[&reviewed.attempt], ActionState::Confirmed);
    }
}

#[test]
fn source_loss_inside_policy_capture_is_seen_before_publication_and_never_refunds_dispatch() {
    for computed_step in [false, true] {
        let mut case = Case::new(8); let reviewed = case.review(901); case.dispatch(&reviewed);
        let path = case.root.topology();
        let mut capture = |_: &crate::action::FrozenAction, _: &CommitteeContract| {
            fs::remove_file(&path).unwrap(); Ok(evidence(&reviewed.inputs))
        };
        let event = if computed_step {
            case.driver.step_with_computed_evidence(|| ElapsedTick(1), &mut capture, None)
        } else { case.driver.step_with_evidence(|| ElapsedTick(1), &mut capture, None) }.unwrap();
        assert!(matches!(event, FileDriverEvent::PublicationChecked { publication, .. }
            if matches!(publication.outcome, EndpointOutcome::NotExecuted { .. })));
        let DriverTopologyOutcome::Finished(Ok(report)) = &case.last().outcome else { panic!("acknowledged withdrawal"); };
        assert!(matches!(report.refusal, Some(TopologySourceRefusal::Input(_))));
        assert!(report.withdrawal.is_some()); assert_eq!(case.work().checks, 2);
        {
            let host = case.driver.supervisor().host().unwrap();
            assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.charged, 16);
            assert!(host.mediation_snapshot().unwrap().accepted.is_none());
        }
        case.reconcile(&reviewed);
        assert_eq!(case.driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
    }
}

#[test]
fn valid_topology_never_replaces_missing_original_helper_inputs() {
    let mut case = Case::new(8); let reviewed = case.review(901); case.dispatch(&reviewed);
    let event = case.driver.step_with_evidence(|| ElapsedTick(1),
        |_, _| Ok(DriverEvidence { snapshot: snapshot(), inputs: None }), None).unwrap();
    assert!(matches!(event, FileDriverEvent::PublicationChecked { publication, .. }
        if matches!(publication.outcome, EndpointOutcome::NotExecuted { .. })));
    let DriverTopologyOutcome::Finished(Ok(report)) = &case.last().outcome else { panic!("actual check"); };
    assert!(report.refusal.is_none()); assert_eq!(case.work().checks, 2);
    assert_eq!(case.driver.supervisor().host().unwrap().inspect().executions, 0);
    case.reconcile(&reviewed);
}

#[test]
fn completed_endpoint_receipt_skips_missing_files_and_exhausted_source_budget() {
    let mut case = Case::new(1); let reviewed = case.review(901); case.dispatch(&reviewed);
    // Existing trusted-owner API completes the very SAME retained envelope.
    // The next driver poll must classify the original endpoint, not reexecute it.
    {
        let mut host = case.driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
        let publication = host.publish_checked(revision, reviewed.attempt, Some(&reviewed.inputs), snapshot(), ElapsedTick(1)).unwrap();
        assert!(matches!(publication.outcome, EndpointOutcome::Executed { .. }));
    }
    fs::remove_file(case.root.topology()).unwrap(); fs::remove_file(case.root.inventory()).unwrap();
    let prior = case.last().clone();
    let event = case.driver.step_with_evidence(|| ElapsedTick(1), |_, _| panic!("resolved provider"), None).unwrap();
    assert!(matches!(event, FileDriverEvent::PublicationChecked { publication, .. }
        if publication.basis == PublicationBasis::PreviouslyResolved));
    assert_eq!(case.work().checks, 1); assert_eq!(case.last(), &prior);
    case.reconcile(&reviewed);
    assert_eq!(case.driver.supervisor().host().unwrap().inspect().executions, 1);
}

#[test]
fn both_deadline_boundaries_use_original_priority_and_post_source_clock() {
    for before_capture in [false, true] {
        let mut case = Case::new(8); let reviewed = case.review(901); case.dispatch(&reviewed);
        let prior = case.last().clone(); let mut clocks = 0; let mut captures = 0;
        let event = case.driver.step_with_evidence(|| {
            clocks += 1;
            if before_capture || clocks > 1 { ElapsedTick(100) } else { ElapsedTick(1) }
        }, |_, _| { captures += 1; Ok(evidence(&reviewed.inputs)) }, None).unwrap();
        assert!(matches!(event, FileDriverEvent::PublicationChecked { publication, .. }
            if publication.basis == PublicationBasis::DeadlineElapsed
                && matches!(publication.outcome, EndpointOutcome::NotExecuted { .. })));
        assert_eq!(clocks, 2); assert_eq!(captures, usize::from(!before_capture));
        assert_eq!(case.work().checks, if before_capture { 1 } else { 2 });
        if before_capture { assert_eq!(case.last(), &prior); }
        assert_eq!(case.driver.supervisor().host().unwrap().inspect().executions, 0);
    }
}

#[test]
fn required_source_exhaustion_on_a_later_request_preserves_the_first_execution() {
    let mut case = Case::new(2); let first = case.review(901); case.dispatch(&first);
    assert!(matches!(case.driver.step_with_evidence(|| ElapsedTick(1),
        |_, _| Ok(evidence(&first.inputs)), None).unwrap(), FileDriverEvent::PublicationChecked { .. }));
    case.reconcile(&first);
    let second = case.review(902); case.dispatch(&second); let spent = case.work();
    let event = case.driver.step_with_evidence(|| ElapsedTick(1), |_, _| Ok(evidence(&second.inputs)), None).unwrap();
    assert!(matches!(event, FileDriverEvent::PublicationChecked { publication, .. }
        if matches!(publication.outcome, EndpointOutcome::NotExecuted { .. })));
    assert_eq!(case.work(), spent);
    let DriverTopologyOutcome::Finished(Ok(report)) = &case.last().outcome else { panic!("budget withdrawal"); };
    assert_eq!(report.refusal, Some(TopologySourceRefusal::BudgetExhausted));
    assert_eq!(case.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 32);
    case.reconcile(&second);
    let state = case.driver.supervisor().host().unwrap().inspect();
    assert_eq!(state.executions, 1); assert_eq!(state.control.ledger.available, 84);
    assert_eq!(state.control.ledger.stages[&first.attempt], ActionState::Confirmed);
}

#[test]
fn setup_refusals_return_original_custody_without_io_or_budget_reset() {
    let (mut case, selected) = Case::unselected(8); let before = case.root.bytes(); let revision = case.revision();
    let error = case.driver.require_topology(revision - 1, selected).unwrap_err();
    assert_eq!(error.error, JournalError::Contract(Error::Stale));
    assert_eq!(error.topology.source().work(), TopologySourceWork::default());
    assert_eq!(case.root.bytes(), before); assert!(case.driver.topology().is_none());
    let (mut other, foreign) = Case::unselected(8);
    let error = other.driver.require_topology(other.revision(), error.topology).unwrap_err();
    assert_eq!(error.error, JournalError::Contract(Error::Binding));
    case.driver.require_topology(revision, error.topology).unwrap();
    let error = case.driver.require_topology(revision, foreign).unwrap_err();
    assert_eq!(error.error, JournalError::Contract(Error::Duplicate));
    other.driver.require_topology(other.revision(), error.topology).unwrap();
    assert_eq!(case.root.bytes(), before);
    assert!(case.driver.refresh_topology(case.revision()).unwrap().refusal.is_none());
    assert!(other.driver.refresh_topology(other.revision()).unwrap().refusal.is_none());
}

#[test]
fn release_handoff_keeps_required_source_work_and_does_not_resurrect_a_job() {
    let mut case = Case::new(8); let reviewed = case.review(901); case.dispatch(&reviewed);
    let work = case.work(); let last = case.last().clone(); let bytes = case.root.bytes();
    let released = case.driver.release();
    assert_eq!(released.topology.as_ref().unwrap().source().work(), work);
    case.driver = released.into_driver().unwrap();
    assert_eq!(case.work(), work); assert_eq!(case.last(), &last);
    assert_eq!(case.driver.phase(), FileDriverPhase::Idle);
    assert!(matches!(case.driver.step_with_evidence(|| panic!("no old-job clock"),
        |_, _| panic!("no old-job provider"), None).unwrap(), FileDriverEvent::Idle));
    assert_eq!(case.root.bytes(), bytes);
    let host = case.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 16); assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.stages[&reviewed.attempt], ActionState::Dispatching);
}

#[test]
fn failed_handoff_returns_the_original_source_for_corrected_same_owner_transfer() {
    let mut case = Case::new(8); let (other, unused_source) = Case::unselected(8);
    let work = case.work(); let last = case.last().clone(); let bytes = case.root.bytes();
    let mut released = case.driver.release(); let foreign = other.driver.release();
    let original = std::mem::replace(&mut released.supervisor, foreign.supervisor);
    let error = released.into_driver().unwrap_err();
    assert_eq!(error.error, JournalError::Contract(Error::Binding));
    assert_eq!(error.release.topology.as_ref().unwrap().source().work(), work);
    let mut released = error.release; released.supervisor = original;
    case.driver = released.into_driver().unwrap();
    assert_eq!(case.work(), work); assert_eq!(case.last(), &last); assert_eq!(case.root.bytes(), bytes);
    drop(unused_source);
    let reviewed = case.review(901); case.dispatch(&reviewed);
    fs::remove_file(case.root.inventory()).unwrap();
    let event = case.driver.step_with_evidence(|| ElapsedTick(1), |_, _| Ok(evidence(&reviewed.inputs)), None).unwrap();
    assert!(matches!(event, FileDriverEvent::PublicationChecked { publication, .. }
        if matches!(publication.outcome, EndpointOutcome::NotExecuted { .. })));
    assert_eq!(case.work().checks, 2);
}

#[test]
fn loss_handoff_and_explicit_newer_rebind_require_fresh_review_without_budget_refill() {
    let mut case = Case::new(32); let old = case.review(901); case.dispatch(&old);
    fs::remove_file(case.root.topology()).unwrap();
    assert!(matches!(case.driver.step_with_evidence(|| ElapsedTick(1), |_, _| Ok(evidence(&old.inputs)), None).unwrap(),
        FileDriverEvent::PublicationChecked { publication, .. }
            if matches!(publication.outcome, EndpointOutcome::NotExecuted { .. })));
    case.reconcile(&old); let spent = case.work();
    case.driver = case.driver.release().into_driver().unwrap();
    fs::write(case.root.topology(), text(1)).unwrap();
    let unchanged = case.driver.refresh_topology(case.revision()).unwrap();
    assert_eq!(unchanged.refusal, Some(TopologySourceRefusal::CoverageUnavailable));
    assert_eq!(case.work(), spent);
    let before = case.root.bytes();
    assert_eq!(case.driver.rebind_topology(case.revision(), 100, loaded(&text(1))),
        Err(JournalError::Contract(Error::Stale)));
    let wrong_limits = document(2, 33, 33 * TOPOLOGY_REFRESH_READ_BYTES, 0,
        crate::action::consequence::mediation::MAX_CHECK_EDGE_VISITS);
    assert_eq!(case.driver.rebind_topology(case.revision(), 100, loaded(&wrong_limits)),
        Err(JournalError::Contract(Error::Binding)));
    assert_eq!(case.root.bytes(), before); assert_eq!(case.work(), spent);
    let replacement = text(2); fs::write(case.root.topology(), &replacement).unwrap();
    case.driver.rebind_topology(case.revision(), 100, loaded(&replacement)).unwrap();
    assert_eq!(case.work(), spent);
    assert!(case.driver.supervisor().host().unwrap().mediation_snapshot().unwrap().accepted.is_none());
    assert!(case.driver.refresh_topology(case.revision()).unwrap().refusal.is_none());
    let fresh = case.review(902); case.dispatch(&fresh);
    assert!(matches!(case.driver.step_with_evidence(|| ElapsedTick(1), |_, _| Ok(evidence(&fresh.inputs)), None).unwrap(),
        FileDriverEvent::PublicationChecked { publication, .. }
            if matches!(publication.outcome, EndpointOutcome::Executed { .. })));
    assert_eq!(case.work().checks, spent.checks + 2);
    case.reconcile(&fresh);
    let host = case.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.stages[&old.attempt], ActionState::ConfirmedNotExecuted);
    assert_eq!(host.inspect().control.ledger.stages[&fresh.attempt], ActionState::Confirmed);
}

#[test]
fn operation_id_overflow_quarantines_without_wrapping_or_refilling_the_source() {
    let (mut case, mut selected) = Case::unselected(8);
    {
        let mut host = case.driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
        selected.source.refresh(&mut host, revision, u64::MAX).unwrap();
    }
    let spent = selected.source().work();
    case.driver.require_topology(case.revision(), selected).unwrap(); let bytes = case.root.bytes();
    assert_eq!(case.driver.refresh_topology(case.revision()), Err(JournalError::Contract(Error::Overflow)));
    assert_eq!(case.work(), spent); assert_eq!(case.root.bytes(), bytes);
    assert_eq!(case.driver.topology().unwrap().source().last_operation(), u64::MAX);
    assert_eq!(case.last().outcome, DriverTopologyOutcome::Finished(Err(JournalError::Contract(Error::Overflow))));
    assert!(case.driver.supervisor().host().unwrap().storage_failure().is_some());
    let release = case.driver.release(); let error = release.into_driver().unwrap_err();
    assert_eq!(error.error, JournalError::Unavailable);
    assert_eq!(error.release.topology.as_ref().unwrap().source().work(), spent);
}

#[test]
fn source_persistence_failures_at_every_barrier_stop_before_final_clock_or_publication() {
    use crate::action::consequence::delivery::persistent::JournalIo;
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let mut case = Case::new(8); let reviewed = case.review(901); case.dispatch(&reviewed);
        let before = case.root.bytes();
        case.driver.supervisor_mut().host_mut().unwrap().store.fail_once(barrier);
        let mut clocks = 0;
        let event = case.driver.step_with_evidence(|| { clocks += 1; ElapsedTick(1) },
            |_, _| Ok(evidence(&reviewed.inputs)), None).unwrap();
        assert!(matches!(event, FileDriverEvent::PublicationUnknown { error: JournalError::Io(ref failure), .. }
            if failure.operation == barrier));
        assert_eq!(clocks, 1, "unacknowledged topology cannot reach the final clock");
        assert!(matches!(case.last().outcome, DriverTopologyOutcome::Finished(Err(JournalError::Io(_)))));
        assert_eq!(case.driver.topology().unwrap().source().last_refusal(), Some(&TopologySourceRefusal::CheckNotAcknowledged));
        assert_eq!(case.work().checks, 2);
        {
            let host = case.driver.supervisor().host().unwrap();
            assert!(host.storage_failure().is_some()); assert_eq!(host.inspect().executions, 0);
            assert_eq!(host.inspect().control.ledger.charged, 16);
        }
        if barrier != JournalIo::DirectorySync { assert_eq!(case.root.bytes(), before); }
        let release = case.driver.release(); let failed = release.into_driver().unwrap_err();
        assert_eq!(failed.error, JournalError::Unavailable);
        assert_eq!(failed.release.topology.as_ref().unwrap().source().work().checks, 2);
    }
}

#[test]
fn post_capture_clock_unwind_preserves_actual_source_result_and_retires_the_send_path() {
    for missing in [false, true] {
        let mut case = Case::new(8); let reviewed = case.review(901); case.dispatch(&reviewed);
        if missing { fs::remove_file(case.root.topology()).unwrap(); }
        let mut clocks = 0;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            case.driver.step_with_evidence(|| {
                clocks += 1;
                if clocks == 2 { panic!("after native source acknowledgment"); }
                ElapsedTick(1)
            }, |_, _| Ok(evidence(&reviewed.inputs)), None)
        }));
        assert!(result.is_err()); assert_eq!(clocks, 2); assert_eq!(case.work().checks, 2);
        let DriverTopologyOutcome::Finished(Ok(report)) = &case.last().outcome else { panic!("retain source result"); };
        assert_eq!(report.refusal.is_some(), missing);
        assert_eq!(case.driver.phase(), FileDriverPhase::AwaitingReconciliation { request: reviewed.request });
        let host = case.driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(host.mediation_snapshot().unwrap().available, !missing);
    }
}

#[test]
fn required_credential_is_not_replaced_by_a_successful_topology_check() {
    use crate::action::consequence::delivery::credential_broker::{BrokerCredential, BrokerRouteBinding, ProviderCredential};
    use crate::action::consequence::delivery::persistent::observed::credential::FILE_OVERSIGHT_CREDENTIAL_PROFILE;
    use crate::perimeter_inventory::LoadedPerimeterInventory;
    for supplied in [false, true] {
        let (mut case, selected) = Case::unselected(8);
        // The separately selected original credential contract has its own
        // inventory metadata; no source parser is replaced to provision it.
        let inventory = LoadedPerimeterInventory::from_json_bytes(INVENTORY.replace(
            "reference-publication", FILE_OVERSIGHT_CREDENTIAL_PROFILE).as_bytes()).unwrap();
        let binding = BrokerRouteBinding { family: "file-publication".into(), route: "publication".into() };
        let permit = {
            let mut host = case.driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
            host.enable_credential_guard(revision, &inventory, &binding).unwrap();
            host.bind_credential_pair(host.revision(), &inventory, &binding,
                BrokerCredential::new(vec![7; 16]).unwrap(), ProviderCredential::new(vec![7; 16]).unwrap()).unwrap()
        };
        case.driver.require_topology(case.revision(), selected).unwrap();
        case.driver.refresh_topology(case.revision()).unwrap();
        let reviewed = case.review(901); case.dispatch(&reviewed);
        let event = if supplied {
            case.driver.step_with_evidence_and_credential(|| ElapsedTick(1),
                |_, _| Ok(evidence(&reviewed.inputs)), None, &permit)
        } else { case.driver.step_with_evidence(|| ElapsedTick(1), |_, _| Ok(evidence(&reviewed.inputs)), None) }.unwrap();
        if supplied {
            assert!(matches!(event, FileDriverEvent::PublicationChecked { publication, .. }
                if matches!(publication.outcome, EndpointOutcome::Executed { .. })));
        } else {
            assert!(matches!(event, FileDriverEvent::PublicationUnknown { error: JournalError::Contract(Error::Incomplete), .. }));
        }
        assert_eq!(case.work().checks, 2);
        let DriverTopologyOutcome::Finished(Ok(report)) = &case.last().outcome else { panic!("actual valid topology"); };
        assert!(report.refusal.is_none());
        assert_eq!(case.driver.supervisor().host().unwrap().inspect().executions, u64::from(supplied));
    }
}
