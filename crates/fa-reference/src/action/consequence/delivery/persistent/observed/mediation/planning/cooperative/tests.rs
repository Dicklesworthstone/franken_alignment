//! Actual owner, original independent checker, two keys and canonical file.
//! Manual congress ballots are controls, not evidence of helper accuracy.
use super::*;
use crate::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::{EndpointOutcome, StopRequest};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, FilePermit,
    JournalIo, JournalLimits, Reconciliation};
use crate::action::consequence::delivery::persistent::observed::{FileHumanPermit,
    FileHumanReviewer, FileOversightProfile, Machine, journal};
use crate::action::consequence::delivery::persistent::observed::mediation::FileMediationSnapshot;
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::mediation::{AuthorityGraph, Channel, Completeness, CutCheck,
    Edge, Enforcer, GraphSpec, Node, NodeKind};
use crate::action::consequence::mediation::planning::{CutPlanningLimits, CutPlanningStatus};
use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract,
    ReviewWindow, action_frame, human::HumanReviewPolicy};
use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use crate::perimeter::{BypassDisposition, CredentialExposure, CredentialHolder, EffectFamilyRecord,
    Mediation, PerimeterScope, RouteRecord, ThreatClass, TrustDomain};
use crate::reducer::Caps;
use crate::round::{Verdict, commitment};
use crate::Snapshot;
use std::{collections::BTreeMap, fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-yielding-mediation-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap(); Self(root)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
    fn bytes(&self) -> Vec<u8> { fs::read(self.store().join("delivery.bin")).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("cooperative mediation cleanup: {error}"); }
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
fn topology(generation: u64, bypass: bool) -> AuthorityGraph {
    let p = profile();
    let mut edges = vec![(1, 2), (2, 3), (3, 4)];
    if bypass { edges.push((1, 4)); }
    AuthorityGraph::new(GraphSpec {
        generation, inventory_generation: generation, scope: p.delivery.scope, target: p.delivery.target,
        family: EffectFamilyRecord {
            scope: PerimeterScope { tenant: 1, principal: 2, purpose: 1 }, family: "publication".into(),
            trust_domains: vec![TrustDomain::Actor, TrustDomain::ObservationAndAnalysis,
                TrustDomain::Enforcement, TrustDomain::GovernanceAndInvestigation],
            credentials: vec![CredentialExposure { credential: "file-owner".into(), holder: CredentialHolder::Broker }],
            routes: vec![RouteRecord { route: "publication".into(), threat: Some(ThreatClass::DirectCredentialOrEgress),
                mediation: Mediation::BrokeredEffects, bypass: BypassDisposition::Blocked }],
            residual_nonclaims: vec!["Declared topology, not operating-system containment".into()],
        },
        completeness: Completeness::DeclaredComplete { inventory_generation: generation },
        nodes: vec![Node { id: 1, kind: NodeKind::Actor }, Node { id: 2, kind: NodeKind::Enforcer },
            Node { id: 3, kind: NodeKind::Enforcer }, Node { id: 4, kind: NodeKind::Sink }],
        edges: edges.into_iter().enumerate().map(|(i, (from, to))| Edge { id: i as u64 + 1,
            from, to, channel: Channel::Dispatch, route: "publication".into(), provenance: i as u64 + 100 }).collect(),
        enforcers: [2, 3].into_iter().map(|node| Enforcer { node, adapter: 10,
            contract_version: 1, generation: 1, provenance: node + 100 }).collect(),
    }).unwrap()
}
fn prices() -> [EnforcerCost; 2] { [EnforcerCost { node: 2, units: 9 }, EnforcerCost { node: 3, units: 2 }] }
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) } }
fn create(root: &Directory) -> (FileOversight, FileHumanReviewer, FileMediationObserver) {
    let (mut host, human) = FileOversight::create(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let observer = host.enable_mediation(host.revision(), topology(1, false)).unwrap();
    (host, human, observer)
}
fn begin(host: &FileOversight, role: &FileMediationObserver) -> FileMediationPlanning {
    role.begin_plan_and_certify(host, host.revision(), host.inspect().control.ledger.epoch,
        &prices(), MediationPlanningBudget::default()).unwrap()
}
fn drive(run: &mut FileMediationPlanning, host: &FileOversight, quantum: usize) {
    for _ in 0..100_000 {
        let before = run.progress();
        let budget = quantum.max(before.solver.minimum_edge_visits);
        let after = run.advance(host, host.revision(), budget).unwrap();
        assert!(after.solver.work.edge_visits - before.solver.work.edge_visits <= budget);
        if after.solver.status == CutPlanningStatus::Ready { return; }
        assert!(after.solver.work.edge_visits > before.solver.work.edge_visits);
    }
    panic!("small original graph did not finish");
}
fn certify(host: &mut FileOversight, role: &FileMediationObserver) -> MediationPlanReceipt {
    let mut run = begin(host, role); drive(&mut run, host, 2);
    let revision = host.revision(); run.certify(role, host, revision).unwrap()
}
fn update(host: &FileOversight, operation: u64, next: Option<AuthorityGraph>) -> FileMediationUpdate {
    FileMediationUpdate { operation, expected_generation: host.mediation_snapshot().unwrap().graph.spec().generation,
        expected_authority_epoch: host.inspect().control.ledger.epoch, next }
}
fn canonical(host: &FileOversight) -> FileMediationSnapshot {
    let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    let events = journal::decode(&host.profile, host.store.identity(), &bytes).unwrap();
    Machine::replay(&host.profile, &events).unwrap().mediation_snapshot(events.len() as u64).unwrap()
}
fn spec(host: &FileOversight) -> ActionSpec {
    ActionSpec { version: VERSION, scope: profile().delivery.scope, target: Some(host.inspect().target),
        payload: b"visible".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 }
}
struct Ready { action: FrozenAction, inputs: CommitteeInput, automatic: FilePermit, human: FileHumanPermit }
fn prepare(host: &mut FileOversight, reviewer: &FileHumanReviewer, attempt: u64) -> Ready {
    let action = host.propose(host.revision(), attempt, spec(host), snapshot()).unwrap();
    let contracts = profile().committee; let helper = &contracts.members()["reviewer"];
    let epoch = action.spec().policy_epoch;
    let mut bytes = action_frame(&action); let boundary = bytes.len();
    bytes.extend_from_slice(helper.question()); let end = bytes.len();
    let actual = ActualHelperInput::new(bytes, helper.profile_at(epoch), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
    ], Vec::new()).unwrap();
    let manifest = EvidenceViewManifest::new(actual, AuthorizationProjection {
        projection_id: 7, policy_epoch: epoch, projected_originals: Vec::new(),
    }, Vec::new()).unwrap();
    let inputs = CommitteeInput::capture(&action, &contracts, BTreeMap::from([("reviewer".to_owned(), manifest)])).unwrap();
    host.record_inputs(host.revision(), attempt, 0, inputs.clone()).unwrap();
    let round = attempt + 100;
    host.begin_review(host.revision(), attempt, round, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8),
    }, snapshot()).unwrap();
    host.commit_review(host.revision(), round, "reviewer", commitment(round, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap()).unwrap();
    host.open_reveals(host.revision(), round).unwrap();
    host.reveal_review(host.revision(), round, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), round, Some(&inputs), snapshot()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), attempt, &inputs, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), attempt + 1000, attempt, &inputs, ElapsedTick(10)).unwrap();
    let revision = host.revision(); let human = reviewer.approve(host, revision, &request).unwrap();
    Ready { action, inputs, automatic, human }
}
fn dispatch(host: &mut FileOversight, ready: &Ready) {
    host.dispatch(host.revision(), &ready.automatic, &ready.human, &ready.action, &ready.inputs, snapshot()).unwrap();
}

#[test]
fn yielding_planner_preserves_time_growth_and_reaches_original_two_key_publication() {
    for quantum in [1, 7, usize::MAX] {
        let root = Directory::new(); let (mut host, human, observer) = create(&root);
        let original = host.mediation_snapshot().unwrap().graph.plan_minimum_cut(&prices(), CutPlanningLimits::default()).unwrap();
        let mut run = begin(&host, &observer); let first = run.progress();
        assert_eq!(first.solver.work.edge_visits, 0);
        let bytes = root.bytes(); run.advance(&host, host.revision(), 1).unwrap();
        assert_eq!(root.bytes(), bytes); assert!(host.mediation_snapshot().unwrap().accepted.is_none());
        let work = run.progress().solver.work;
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        assert_eq!(host.inspect().control.sequence, first.control_sequence);
        assert_eq!(run.progress().solver.work, work);
        let bytes = root.bytes(); drive(&mut run, &host, quantum);
        assert_eq!(root.bytes(), bytes); assert!(host.mediation_snapshot().unwrap().accepted.is_none());
        let revision = host.revision(); let receipt = run.certify(&observer, &mut host, revision).unwrap();
        assert_eq!(receipt.plan, original); assert_eq!(receipt.journal_revision, revision + 1);
        let CutCheck::Verified(cut) = receipt.checked.unwrap() else { panic!("independent check required"); };
        let ready = prepare(&mut host, &human, 1); dispatch(&mut host, &ready);
        assert_eq!(host.delivery_mediation(1).unwrap(), Some(&cut));
        let publication = host.publish_checked(host.revision(), 1, Some(&ready.inputs), snapshot(), ElapsedTick(3)).unwrap();
        assert_eq!(publication.outcome, EndpointOutcome::Executed { resulting_version: 2 });
        assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(publication.outcome));
        assert_eq!(host.inspect().payload, b"visible"); assert_eq!(host.inspect().executions, 1);
        assert_eq!(canonical(&host).accepted, Some(cut));
    }
}

#[test]
fn cancellation_and_incomplete_certification_leave_existing_cut_and_bytes_unchanged() {
    let root = Directory::new(); let (mut host, _, observer) = create(&root);
    certify(&mut host, &observer); let before = host.mediation_snapshot().unwrap(); let bytes = root.bytes();
    let steps = before.graph.plan_minimum_cut(&prices(), CutPlanningLimits::default()).unwrap().work.edge_visits;
    for polls in 0..=steps {
        let mut run = begin(&host, &observer);
        for _ in 0..polls { run.advance(&host, host.revision(), 2).unwrap(); }
        let progress = run.progress();
        match polls % 3 {
            0 => assert_eq!(run.cancel(), progress.solver.work),
            1 => drop(run),
            _ if progress.solver.status != CutPlanningStatus::Ready => {
                let revision = host.revision();
                assert_eq!(run.certify(&observer, &mut host, revision), Err(JournalError::Contract(Error::Incomplete)));
            }
            _ => { let _ = run.cancel(); }
        }
        assert_eq!(host.mediation_snapshot().unwrap(), before); assert_eq!(root.bytes(), bytes);
    }
}

#[test]
fn foreign_custody_and_wrong_poll_predecessors_spend_nothing_but_correct_calls_work() {
    let root = Directory::new(); let foreign_root = Directory::new();
    let (mut host, _, observer) = create(&root); let (foreign_host, _, foreign) = create(&foreign_root);
    let mut run = begin(&host, &observer); let before = run.progress(); let bytes = root.bytes();
    assert_eq!(run.advance(&host, host.revision() - 1, 10), Err(JournalError::Contract(Error::Stale)));
    assert_eq!(run.advance(&foreign_host, foreign_host.revision(), 10), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(run.progress(), before); assert_eq!(root.bytes(), bytes);
    drive(&mut run, &host, 2); let revision = host.revision();
    assert_eq!(run.certify(&foreign, &mut host, revision), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(root.bytes(), bytes); assert!(host.mediation_snapshot().unwrap().accepted.is_none());
    assert!(matches!(certify(&mut host, &observer).checked, Ok(CutCheck::Verified(_))));
}

#[test]
fn stop_between_partial_or_ready_polls_is_not_delayed_and_unknown_effects_stay_charged() {
    for ready_first in [false, true] {
        let root = Directory::new(); let (mut host, reviewer, observer) = create(&root);
        certify(&mut host, &observer);
        let ready = prepare(&mut host, &reviewer, 1); dispatch(&mut host, &ready);
        let mut run = begin(&host, &observer);
        let mut unpolled_after_stop = begin(&host, &observer);
        if ready_first { drive(&mut run, &host, 2); drive(&mut unpolled_after_stop, &host, 2); }
        else { run.advance(&host, host.revision(), 2).unwrap(); }
        let control = host.inspect().control;
        let request = StopRequest { operation: 801, expected_control_sequence: control.sequence,
            expected_authority_epoch: control.ledger.epoch };
        let receipt = host.request_stop(host.revision(), request).unwrap();
        assert_eq!(receipt.request(), request); assert!(receipt.cancelled().is_empty());
        assert_eq!(host.stop_progress().unwrap().unresolved, vec![1]);
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(host.inspect().executions, 0);
        let stopped = root.bytes(); let work = run.progress().solver.work;
        assert_eq!(run.advance(&host, host.revision(), usize::MAX), Err(JournalError::Contract(Error::Stale)));
        assert_eq!(run.progress().solver.work, work);
        assert_eq!(run.progress().invalidated, Some(JournalError::Contract(Error::Stale)));
        assert_eq!(run.advance(&host, host.revision(), 0), Err(JournalError::Contract(Error::Stale)));
        let revision = host.revision();
        assert_eq!(unpolled_after_stop.certify(&observer, &mut host, revision), Err(JournalError::Contract(Error::Stale)));
        assert_eq!(root.bytes(), stopped); assert_eq!(host.inspect().control.ledger.charged, 16);
        assert!(host.propose(host.revision(), 2, spec(&host), snapshot()).is_err());
    }
}

#[test]
fn topology_change_invalidates_old_tasks_and_old_keys_while_new_task_can_publish() {
    let root = Directory::new(); let (mut host, reviewer, observer) = create(&root);
    certify(&mut host, &observer); let old = prepare(&mut host, &reviewer, 1);
    let mut outdated = begin(&host, &observer); drive(&mut outdated, &host, 2);
    let request = update(&host, 20, Some(topology(2, false))); let revision = host.revision();
    let next = observer.update_and_begin_plan(&mut host, revision, &request, &prices(), MediationPlanningBudget::default()).unwrap();
    assert_eq!(next.change.unwrap().cancelled, vec![1]);
    assert!(host.mediation_snapshot().unwrap().accepted.is_none());
    let changed = root.bytes();
    assert_eq!(outdated.advance(&host, host.revision(), usize::MAX), Err(JournalError::Contract(Error::Stale)));
    assert_eq!(root.bytes(), changed); assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    assert!(host.dispatch(host.revision(), &old.automatic, &old.human, &old.action, &old.inputs, snapshot()).is_err());
    let mut next = next.planning.unwrap(); drive(&mut next, &host, 2);
    let revision = host.revision(); let result = next.certify(&observer, &mut host, revision).unwrap();
    let CutCheck::Verified(cut) = result.checked.unwrap() else { panic!("original current checker"); };
    assert_eq!(cut.graph().spec().generation, 2);
    let fresh = prepare(&mut host, &reviewer, 2); dispatch(&mut host, &fresh);
    assert_eq!(host.publish_checked(host.revision(), 2, Some(&fresh.inputs), snapshot(), ElapsedTick(2)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
}

#[test]
fn failed_admission_or_cancelled_new_plan_cannot_undo_an_acknowledged_revocation() {
    let root = Directory::new(); let (mut host, _, observer) = create(&root);
    certify(&mut host, &observer);
    let request = update(&host, 30, Some(topology(2, false))); let revision = host.revision();
    let result = observer.update_and_begin_plan(&mut host, revision, &request, &[], MediationPlanningBudget::default()).unwrap();
    assert_eq!(result.planning.unwrap_err(), JournalError::Contract(Error::Incomplete));
    assert_eq!(host.revision(), revision + 1); assert_eq!(host.inspect().control.ledger.epoch, 1);
    assert!(host.mediation_snapshot().unwrap().accepted.is_none()); let changed = root.bytes();
    let retry = observer.update_and_begin_plan(&mut host, 0, &request, &prices(), MediationPlanningBudget::default()).unwrap();
    assert_eq!(retry.change, result.change); let _ = retry.planning.unwrap().cancel();
    assert_eq!(root.bytes(), changed); assert!(canonical(&host).accepted.is_none());
    assert!(matches!(certify(&mut host, &observer).checked, Ok(CutCheck::Verified(_))));
    let withdraw = update(&host, 31, None); let revision = host.revision();
    let result = observer.update_and_begin_plan(&mut host, revision, &withdraw, &prices(), MediationPlanningBudget::default()).unwrap();
    assert_eq!(result.planning.unwrap_err(), JournalError::Contract(Error::Incomplete));
    assert!(!host.mediation_snapshot().unwrap().available); let bytes = root.bytes();
    let retry = observer.update_and_begin_plan(&mut host, 0, &request, &prices(), MediationPlanningBudget::default()).unwrap();
    assert!(retry.planning.is_err()); assert_eq!(root.bytes(), bytes);
}

#[test]
fn solver_exhaustion_preserves_original_cut_and_verifier_exhaustion_records_no_new_certificate() {
    let root = Directory::new(); let (mut host, _, observer) = create(&root);
    certify(&mut host, &observer); let before = host.mediation_snapshot().unwrap(); let bytes = root.bytes();
    let mut budget = MediationPlanningBudget::default(); budget.planning.edge_visits = 1;
    let mut run = observer.begin_plan_and_certify(&host, host.revision(), host.inspect().control.ledger.epoch,
        &prices(), budget).unwrap();
    assert_eq!(run.advance(&host, host.revision(), usize::MAX), Err(JournalError::Contract(Error::Limit)));
    assert_eq!(run.progress().solver.work.edge_visits, 1);
    assert_eq!(run.advance(&host, host.revision(), usize::MAX), Err(JournalError::Contract(Error::Limit)));
    assert_eq!(root.bytes(), bytes); assert_eq!(host.mediation_snapshot().unwrap(), before);
    let budget = MediationPlanningBudget { verification_edge_visits: 0, ..MediationPlanningBudget::default() };
    let mut run = observer.begin_plan_and_certify(&host, host.revision(), host.inspect().control.ledger.epoch,
        &prices(), budget).unwrap(); drive(&mut run, &host, 2);
    let revision = host.revision(); let result = run.certify(&observer, &mut host, revision).unwrap();
    assert_eq!(result.checked, Err(Error::Limit)); assert_eq!(result.journal_revision, revision + 1);
    assert_eq!(host.mediation_snapshot().unwrap().accepted, before.accepted);
    assert_eq!(host.mediation_snapshot().unwrap().last_check, Some(Err(Error::Limit)));
    let request = update(&host, 40, Some(topology(2, true))); let revision = host.revision();
    let mut run = observer.update_and_begin_plan(&mut host, revision, &request, &prices(), MediationPlanningBudget::default())
        .unwrap().planning.unwrap(); drive(&mut run, &host, 1);
    let revision = host.revision(); let result = run.certify(&observer, &mut host, revision).unwrap();
    assert!(matches!(result.plan.outcome, CutPlanOutcome::Bypass(_)));
    assert!(matches!(result.checked, Ok(CutCheck::Bypass(_))));
    assert!(canonical(&host).accepted.is_none());
}

#[test]
fn each_original_certification_barrier_returns_no_receipt_and_other_jobs_cannot_use_faulted_owner() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let (mut host, _, observer) = create(&root);
        let mut run = begin(&host, &observer); let mut other = begin(&host, &observer);
        drive(&mut run, &host, 2); let before = host.inspect(); let revision = host.revision();
        host.store.fail_once(barrier);
        let JournalError::Io(failure) = run.certify(&observer, &mut host, revision).unwrap_err()
            else { panic!("original selected persistence barrier"); };
        assert_eq!(failure.operation, barrier); assert_eq!(host.inspect(), before);
        let work = other.progress().solver.work;
        assert_eq!(other.advance(&host, revision, 100), Err(JournalError::Unavailable));
        assert_eq!(other.progress().solver.work, work);
        assert_eq!(host.mediation_snapshot(), Err(JournalError::Unavailable));
        assert_eq!(canonical(&host).accepted.is_some(), barrier == JournalIo::DirectorySync);
        drop(host);
        let (recovered, _) = FileOversight::open(root.store(), profile()).unwrap();
        assert!(recovered.mediation_snapshot().unwrap().accepted.is_none());
        assert_eq!(other.advance(&recovered, recovered.revision(), 100), Err(JournalError::Contract(Error::Binding)));
    }
}

#[test]
fn a_task_keeps_neither_the_file_lock_nor_observer_custody_after_owner_drop() {
    let root = Directory::new(); let (host, _, observer) = create(&root);
    let mut run = begin(&host, &observer); run.advance(&host, host.revision(), 1).unwrap();
    let work = run.progress().solver.work; drop(host);
    let (mut recovered, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(run.advance(&recovered, recovered.revision(), 100), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(run.progress().solver.work, work); let bytes = root.bytes(); let revision = recovered.revision();
    assert_eq!(run.certify(&observer, &mut recovered, revision), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(root.bytes(), bytes); assert!(recovered.mediation_snapshot().unwrap().accepted.is_none());
}
