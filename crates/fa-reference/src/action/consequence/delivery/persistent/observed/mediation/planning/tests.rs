use super::*;
use super::super::{FileMediationSnapshot, FileMediationUpdate};
use super::super::super::{FileHumanPermit, FileHumanReviewer, FileOversightProfile, FilePermit,
    JournalIo, Machine, ReviewWindow, journal};
use super::super::super::super::{FileDeliveryProfile, JournalLimits, Reconciliation};
use crate::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::EndpointOutcome;
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::mediation::{AuthorityGraph, Channel, Completeness, Edge, Enforcer, Node, NodeKind};
use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, action_frame};
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
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-planned-mediation-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&root).unwrap(); Self(root)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("planned mediation cleanup: {error}"); }
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
fn topology(generation: u64, parallel: bool, bypass: bool) -> AuthorityGraph {
    let p = profile();
    let basic = super::super::tests::graph(p.delivery.scope, p.delivery.target, generation, false);
    let mut spec = basic.spec().clone();
    spec.nodes = vec![Node { id: 1, kind: NodeKind::Actor }, Node { id: 2, kind: NodeKind::Enforcer },
        Node { id: 3, kind: NodeKind::Enforcer }, Node { id: 4, kind: NodeKind::Sink }];
    let mut edges = if parallel { vec![(1, 2), (2, 4), (1, 3), (3, 4)] }
        else { vec![(1, 2), (2, 3), (3, 4)] };
    if bypass { edges.push((1, 4)); }
    spec.edges = edges.into_iter().enumerate().map(|(i, (from, to))| Edge {
        id: i as u64 + 1, from, to, channel: Channel::Dispatch,
        route: "publication".into(), provenance: i as u64 + 100,
    }).collect();
    spec.enforcers.push(Enforcer { node: 3, ..spec.enforcers[0] });
    AuthorityGraph::new(spec).unwrap()
}
fn prices() -> [EnforcerCost; 2] { [EnforcerCost { node: 2, units: 9 }, EnforcerCost { node: 3, units: 2 }] }
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) } }
fn create(root: &Directory, graph: AuthorityGraph) -> (FileOversight, FileHumanReviewer, FileMediationObserver) {
    let (mut host, human) = FileOversight::create(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let observer = host.enable_mediation(host.revision(), graph).unwrap();
    (host, human, observer)
}
fn plan(host: &mut FileOversight, role: &FileMediationObserver) -> MediationPlanReceipt {
    let revision = host.revision(); let epoch = host.inspect().control.ledger.epoch;
    role.plan_and_certify(host, revision, epoch, &prices(), MediationPlanningBudget::default()).unwrap()
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
    let contracts = profile().committee;
    let helper = &contracts.members()["reviewer"];
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
fn cheapest_independently_checked_cut_is_consumed_by_a_real_two_key_publication() {
    let root = Directory::new(); let (mut host, human, observer) = create(&root, topology(1, false, false));
    assert!(host.propose(host.revision(), 1, spec(&host), snapshot()).is_err());
    let revision = host.revision(); let result = plan(&mut host, &observer);
    assert_eq!(result.journal_revision, revision + 1);
    let CutPlanOutcome::Candidate(candidate) = result.plan.outcome else { panic!("minimum candidate"); };
    assert_eq!(candidate.total_cost, 2); assert_eq!(candidate.proposal.gates, vec![3]);
    let CutCheck::Verified(cut) = result.checked.unwrap() else { panic!("original checker"); };
    assert_eq!(host.mediation_snapshot().unwrap().accepted.as_ref(), Some(&cut));
    let ready = prepare(&mut host, &human, 1); dispatch(&mut host, &ready);
    assert_eq!(host.delivery_mediation(1).unwrap(), Some(&cut));
    let published = host.publish_checked(host.revision(), 1, Some(&ready.inputs), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(published.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(published.outcome));
    assert_eq!(host.inspect().payload, b"visible"); assert_eq!(host.inspect().executions, 1);
    assert_eq!(canonical(&host).accepted, Some(cut));
}

#[test]
fn topology_replanning_cancels_old_approvals_but_admits_fresh_work_under_the_new_cut() {
    let root = Directory::new(); let (mut host, human, observer) = create(&root, topology(1, false, false));
    plan(&mut host, &observer); let old = prepare(&mut host, &human, 1);
    let request = update(&host, 10, Some(topology(2, true, false))); let revision = host.revision();
    let receipt = observer.update_and_plan(&mut host, revision, &request, &prices(), MediationPlanningBudget::default()).unwrap();
    let change = receipt.change.unwrap(); assert_eq!(change.cancelled, vec![1]); assert_eq!(change.refunded_units, 16);
    let CutCheck::Verified(cut) = receipt.certification.unwrap().checked.unwrap() else { panic!("new cut"); };
    assert_eq!(cut.gates(), &[2, 3]); assert_eq!(cut.graph().spec().generation, 2);
    assert_eq!(host.inspect().control.ledger.epoch, 1);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Revoked);
    assert!(host.dispatch(host.revision(), &old.automatic, &old.human, &old.action, &old.inputs, snapshot()).is_err());
    let fresh = prepare(&mut host, &human, 2); dispatch(&mut host, &fresh);
    assert_eq!(host.delivery_mediation(2).unwrap(), Some(&cut));
    assert_eq!(host.publish_checked(host.revision(), 2, Some(&fresh.inputs), snapshot(), ElapsedTick(2)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
}

#[test]
fn newly_observed_bypass_keeps_dispatched_liability_and_native_publication_seals_it() {
    let root = Directory::new(); let (mut host, human, observer) = create(&root, topology(1, false, false));
    plan(&mut host, &observer); let ready = prepare(&mut host, &human, 1); dispatch(&mut host, &ready);
    let retained = host.delivery_mediation(1).unwrap().unwrap().clone();
    let request = update(&host, 20, Some(topology(2, false, true))); let revision = host.revision();
    let report = observer.update_and_plan(&mut host, revision, &request, &prices(), MediationPlanningBudget::default()).unwrap();
    let result = report.certification.unwrap();
    assert!(matches!(result.plan.outcome, CutPlanOutcome::Bypass(_)));
    assert!(matches!(result.checked, Ok(CutCheck::Bypass(_))));
    assert!(host.mediation_snapshot().unwrap().accepted.is_none());
    assert_eq!(host.delivery_mediation(1).unwrap(), Some(&retained));
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().control.ledger.available, 84);
    let sealed = host.publish_checked(host.revision(), 1, Some(&ready.inputs), snapshot(), ElapsedTick(2)).unwrap();
    assert!(matches!(sealed.outcome, EndpointOutcome::NotExecuted { .. }));
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(sealed.outcome));
    assert_eq!(host.inspect().control.ledger.available, 100); assert_eq!(host.inspect().executions, 0);
    assert!(host.propose(host.revision(), 2, spec(&host), snapshot()).is_err());
}

#[test]
fn solver_exhaustion_after_update_cannot_roll_back_the_acknowledged_topology() {
    let root = Directory::new(); let (mut host, _, observer) = create(&root, topology(1, false, false));
    plan(&mut host, &observer);
    let request = update(&host, 30, Some(topology(2, true, false))); let revision = host.revision();
    let budget = MediationPlanningBudget { planning: CutPlanningLimits { edge_visits: 1, ..CutPlanningLimits::default() },
        ..MediationPlanningBudget::default() };
    let receipt = observer.update_and_plan(&mut host, revision, &request, &prices(), budget).unwrap();
    assert!(receipt.change.is_some()); assert_eq!(receipt.certification, Err(JournalError::Contract(Error::Limit)));
    assert_eq!(host.revision(), revision + 1);
    let after = host.mediation_snapshot().unwrap();
    assert!(after.available); assert!(after.accepted.is_none()); assert_eq!(after.graph.spec().generation, 2);
    assert_eq!(canonical(&host), after);
    assert!(host.propose(host.revision(), 1, spec(&host), snapshot()).is_err());
    let epoch = host.inspect().control.ledger.epoch;
    // Retrying the acknowledged UPDATE does not repeat its revocation.
    let receipt = observer.update_and_plan(&mut host, 0, &request, &prices(), MediationPlanningBudget::default()).unwrap();
    assert!(matches!(receipt.certification.unwrap().checked, Ok(CutCheck::Verified(_))));
    assert_eq!(host.inspect().control.ledger.epoch, epoch);
    assert_eq!(host.mediation_snapshot().unwrap().retained_updates, 1);
}

#[test]
fn the_independent_verifier_has_its_own_budget_and_its_refusal_is_durable() {
    let root = Directory::new(); let (mut host, _, observer) = create(&root, topology(1, false, false));
    let revision = host.revision();
    let receipt = observer.plan_and_certify(&mut host, revision, 0, &prices(),
        MediationPlanningBudget { verification_edge_visits: 1, ..MediationPlanningBudget::default() }).unwrap();
    assert!(matches!(receipt.plan.outcome, CutPlanOutcome::Candidate(_)));
    assert_eq!(receipt.checked, Err(Error::Limit));
    assert_eq!(host.mediation_snapshot().unwrap().last_check, Some(Err(Error::Limit)));
    assert!(canonical(&host).accepted.is_none());
    assert!(host.propose(host.revision(), 1, spec(&host), snapshot()).is_err());
    assert!(matches!(plan(&mut host, &observer).checked, Ok(CutCheck::Verified(_))));
}

#[test]
fn foreign_role_stale_predecessors_and_missing_prices_never_replace_a_valid_cut() {
    let root = Directory::new(); let other = Directory::new();
    let (mut host, _, observer) = create(&root, topology(1, false, false));
    let (_other, _, foreign) = create(&other, topology(1, false, false)); plan(&mut host, &observer);
    let before = host.mediation_snapshot().unwrap(); let revision = host.revision();
    assert_eq!(foreign.plan_and_certify(&mut host, revision, 0, &prices(), MediationPlanningBudget::default()), Err(JournalError::Contract(Error::Binding)));
    for (revision, epoch) in [(revision - 1, 0), (revision, 1)] {
        assert_eq!(observer.plan_and_certify(&mut host, revision, epoch, &prices(), MediationPlanningBudget::default()), Err(JournalError::Contract(Error::Stale)));
    }
    assert_eq!(observer.plan_and_certify(&mut host, revision, 0, &[], MediationPlanningBudget::default()), Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.mediation_snapshot().unwrap(), before); assert_eq!(canonical(&host), before);
}

#[test]
fn retry_of_an_older_topology_never_certifies_a_newer_graph_and_withdrawal_stays_closed() {
    let root = Directory::new(); let (mut host, _, observer) = create(&root, topology(1, false, false));
    plan(&mut host, &observer);
    let first = update(&host, 40, Some(topology(2, true, false))); let revision = host.revision();
    observer.update_and_plan(&mut host, revision, &first, &prices(), MediationPlanningBudget::default()).unwrap();
    let second = update(&host, 41, Some(topology(3, false, true))); let revision = host.revision();
    observer.update(&mut host, revision, &second).unwrap(); let before = host.mediation_snapshot().unwrap();
    let retry = observer.update_and_plan(&mut host, 0, &first, &prices(), MediationPlanningBudget::default()).unwrap();
    assert_eq!(retry.certification, Err(JournalError::Contract(Error::Stale)));
    assert_eq!(host.mediation_snapshot().unwrap(), before);
    let withdraw = update(&host, 42, None); let revision = host.revision();
    let result = observer.update_and_plan(&mut host, revision, &withdraw, &prices(), MediationPlanningBudget::default()).unwrap();
    assert_eq!(result.certification, Err(JournalError::Contract(Error::Incomplete)));
    assert!(!host.mediation_snapshot().unwrap().available);
    let revision = host.revision(); let epoch = host.inspect().control.ledger.epoch;
    assert_eq!(observer.plan_and_certify(&mut host, revision, epoch, &prices(), MediationPlanningBudget::default()), Err(JournalError::Contract(Error::Incomplete)));
}

#[test]
fn unknown_inventory_update_and_unreachable_sink_are_never_successful_coverage() {
    let root = Directory::new(); let (mut host, _, observer) = create(&root, topology(1, false, false));
    plan(&mut host, &observer);
    let mut graph = topology(2, false, false).spec().clone(); graph.completeness = Completeness::Unknown;
    let request = update(&host, 50, Some(AuthorityGraph::new(graph).unwrap())); let revision = host.revision();
    let result = observer.update_and_plan(&mut host, revision, &request, &prices(), MediationPlanningBudget::default()).unwrap();
    assert_eq!(result.certification, Err(JournalError::Contract(Error::Incomplete)));
    assert!(canonical(&host).accepted.is_none());
    let mut graph = topology(3, false, false).spec().clone(); graph.nodes.push(Node { id: 5, kind: NodeKind::Sink });
    let request = update(&host, 51, Some(AuthorityGraph::new(graph).unwrap())); let revision = host.revision();
    let result = observer.update_and_plan(&mut host, revision, &request, &prices(), MediationPlanningBudget::default()).unwrap().certification.unwrap();
    assert_eq!(result.plan.outcome, CutPlanOutcome::Unreachable { sinks: vec![5] });
    assert_eq!(result.checked, Ok(CutCheck::Unreachable { sinks: vec![5] }));
    assert!(canonical(&host).accepted.is_none());
}

#[test]
fn each_certification_storage_barrier_exposes_no_candidate_and_recovery_withdraws_old_cuts() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let (mut host, _, observer) = create(&root, topology(1, false, false));
        let before = host.inspect(); let revision = host.revision(); host.store.fail_once(barrier);
        let JournalError::Io(failure) = observer.plan_and_certify(&mut host, revision, 0, &prices(), MediationPlanningBudget::default()).unwrap_err()
            else { panic!("selected certificate write barrier"); };
        assert_eq!(failure.operation, barrier); assert_eq!(host.inspect(), before);
        assert_eq!(host.mediation_snapshot(), Err(JournalError::Unavailable));
        let disk = canonical(&host);
        assert_eq!(disk.accepted.is_some(), barrier == JournalIo::DirectorySync);
        drop(host);
        let (mut recovered, _) = FileOversight::open(root.store(), profile()).unwrap();
        assert!(!recovered.mediation_snapshot().unwrap().available);
        assert!(recovered.mediation_snapshot().unwrap().accepted.is_none());
        let revision = recovered.revision(); let epoch = recovered.inspect().control.ledger.epoch;
        assert_eq!(observer.plan_and_certify(&mut recovered, revision, epoch, &prices(), MediationPlanningBudget::default()), Err(JournalError::Contract(Error::Binding)));
        assert_eq!(recovered.inspect().executions, 0);
    }
}

#[test]
fn update_storage_failure_quarantines_the_owner_without_attempting_certification() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let (mut host, _, observer) = create(&root, topology(1, false, false));
        plan(&mut host, &observer); let before = host.inspect();
        let request = update(&host, 60, Some(topology(2, false, true))); let revision = host.revision();
        host.store.fail_once(barrier);
        let JournalError::Io(failure) = observer.update_and_plan(&mut host, revision, &request, &prices(), MediationPlanningBudget::default()).unwrap_err()
            else { panic!("selected topology write barrier"); };
        assert_eq!(failure.operation, barrier); assert_eq!(host.inspect(), before);
        assert_eq!(host.mediation_snapshot(), Err(JournalError::Unavailable));
        let disk = canonical(&host); let replaced = barrier == JournalIo::DirectorySync;
        assert_eq!(disk.graph.spec().generation, if replaced { 2 } else { 1 });
        assert_eq!(disk.accepted.is_some(), !replaced);
        if replaced { assert!(disk.last_check.is_none()); }
    }
}
