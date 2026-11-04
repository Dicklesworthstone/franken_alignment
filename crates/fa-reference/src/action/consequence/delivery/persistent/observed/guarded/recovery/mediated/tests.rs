//! Actual canonical journals, original transition replay and replacement faults.
//! These controls do not establish hardware durability or a replay latency bound.
use super::*;
use super::super::{BaseEvent, Event};
use super::super::super::{FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements};
use super::super::super::mediated::FileTopologyRequirement;
use crate::Error;
use crate::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalIo, JournalLimits};
use crate::action::consequence::delivery::persistent::observed::mediation::{
    FileMediationObserver, FileMediationUpdate, MediationEvent};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::mediation::{AuthorityGraph, Channel, Completeness,
    CutCheck, Edge, Enforcer, GraphSpec, Node, NodeKind, MAX_CHECK_EDGE_VISITS};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::full_input::InputProfileBinding;
use crate::perimeter::*;
use crate::reducer::Caps;
use std::{collections::BTreeMap, fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

static NEXT: AtomicU64 = AtomicU64::new(0);
const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
    JournalIo::Rename, JournalIo::DirectorySync];
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-mediated-replay-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap(); Self(root)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
    fn bytes(&self) -> Vec<u8> { fs::read(self.store().join(storage::CANONICAL)).unwrap() }
    fn pending(&self) -> PathBuf { self.store().join("delivery.pending") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("mediated replay cleanup: {error}"); }
    }
}
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1,
                tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart },
                vec![1], vec![2], vec![3], 1).unwrap(),
            suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".into(),
                MemberPolicy { cohort: "one".into(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
                narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("reviewer".into(), HelperContract::new(InputProfileBinding {
            profile_id: 1, profile_bytes: b"mediated-fixture".to_vec(), tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1,
        }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn graph(generation: u64) -> AuthorityGraph {
    let p = profile().delivery;
    AuthorityGraph::new(GraphSpec { generation, inventory_generation: generation, scope: p.scope, target: p.target,
        family: EffectFamilyRecord { scope: PerimeterScope { tenant: 1, principal: 2, purpose: 1 },
            family: "publication".into(), trust_domains: vec![TrustDomain::Actor, TrustDomain::ObservationAndAnalysis,
                TrustDomain::Enforcement, TrustDomain::GovernanceAndInvestigation],
            credentials: vec![CredentialExposure { credential: "owner".into(), holder: CredentialHolder::Broker }],
            routes: vec![RouteRecord { route: "publication".into(), threat: Some(ThreatClass::DirectCredentialOrEgress),
                mediation: Mediation::BrokeredEffects, bypass: BypassDisposition::Blocked }],
            residual_nonclaims: vec!["Declared topology only".into()] },
        completeness: Completeness::DeclaredComplete { inventory_generation: generation },
        nodes: vec![Node { id: 1, kind: NodeKind::Actor }, Node { id: 2, kind: NodeKind::Enforcer }, Node { id: 3, kind: NodeKind::Sink }],
        edges: vec![Edge { id: 1, from: 1, to: 2, channel: Channel::Dispatch, route: "publication".into(), provenance: 1 },
            Edge { id: 2, from: 2, to: 3, channel: Channel::Dispatch, route: "publication".into(), provenance: 2 }],
        enforcers: vec![Enforcer { node: 2, adapter: p.target.adapter, contract_version: 1, generation: 1, provenance: 9 }],
    }).unwrap()
}
fn expected() -> FileMediatedRequirements {
    FileMediatedRequirements { oversight: FileRecoveryRequirements {
        guards: FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None, identity: None,
            campaigns: None, credential: None }, effective_policy: profile().delivery.policy, credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: 0, control_sequence: 0, authority_epoch: 0 },
    }, topology: FileTopologyRequirement { initial: graph(1), current: graph(1), available: true },
        prediction: None, evaluation: None }
}
fn start(root: &Directory) -> (FileOversight, FileMediatedRoles) {
    let (mut host, roles) = FileOversight::create_mediated_guarded(root.store(), profile(),
        &expected().oversight.guards, None, graph(1), None, None).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    certify(&mut host, &roles.topology_observer);
    (host, roles)
}
fn certify(host: &mut FileOversight, role: &FileMediationObserver) {
    let proposal = host.mediation_snapshot().unwrap().graph.propose_cut(&[2]).unwrap();
    let rev = host.revision(); let epoch = host.inspect().control.ledger.epoch;
    assert!(matches!(role.certify(host, rev, epoch, &proposal, MAX_CHECK_EDGE_VISITS).unwrap().unwrap(), CutCheck::Verified(_)));
}
fn ready(run: &mut FileMediatedRecovery, quantum: usize) {
    while run.progress().status == FileGuardedRecoveryStatus::Replaying {
        let before = run.progress();
        let after = run.advance(before.replayed_events, quantum).unwrap();
        assert_eq!(after.replayed_events - before.replayed_events,
            quantum.min(before.total_events - before.replayed_events));
        assert_eq!(after.attempted_events, after.replayed_events);
    }
    assert_eq!(run.progress().status, FileGuardedRecoveryStatus::Ready);
}

#[test]
fn bounded_polls_keep_the_writer_and_finish_one_fence_with_fresh_observer_custody() {
    for quantum in [1, 3, MAX_JOURNAL_EVENTS] {
        let root = Directory::new(); let (host, old) = start(&root);
        let revision = host.revision(); let anchor = host.history_anchor().unwrap(); drop(host);
        let before = root.bytes();
        let mut run = FileOversight::begin_open_mediated_guarded_anchored(root.store(), profile(), &expected(), &anchor).unwrap();
        assert_eq!(run.progress().attempted_events, 0);
        assert!(FileOversight::begin_open_mediated_guarded(root.store(), profile(), &expected()).is_err());
        assert_eq!(run.advance(1, 1), Err(JournalError::Contract(Error::Stale)));
        assert_eq!(run.advance(0, 0), Err(JournalError::Contract(Error::InvalidInput)));
        assert_eq!(run.advance(0, MAX_JOURNAL_EVENTS + 1), Err(JournalError::Contract(Error::Limit)));
        assert_eq!(run.progress().attempted_events, 0);
        ready(&mut run, quantum);
        let done = run.progress(); assert_eq!(run.advance(done.replayed_events, 1).unwrap(), done);
        assert_eq!(root.bytes(), before, "Ready grants no owner and spends no fence");
        let (mut host, roles) = run.finish().unwrap();
        assert_eq!(host.revision(), revision + 1);
        assert!(!host.mediation_snapshot().unwrap().available);
        assert!(host.mediation_snapshot().unwrap().accepted.is_none());
        assert!(roles.consistency_observer.is_none()); assert!(roles.evaluator.is_none());
        let update = FileMediationUpdate { operation: 1, expected_generation: 1,
            expected_authority_epoch: host.inspect().control.ledger.epoch, next: Some(graph(2)) };
        let revision = host.revision();
        assert_eq!(old.topology_observer.update(&mut host, revision, &update), Err(JournalError::Contract(Error::Binding)));
        roles.topology_observer.update(&mut host, revision, &update).unwrap();
        certify(&mut host, &roles.topology_observer);
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.available, 100);
    }
}

#[test]
fn cancellation_and_early_finish_preserve_canonical_and_staging_bytes() {
    for complete in [false, true] {
        let root = Directory::new(); let (host, _) = start(&root); drop(host);
        let before = root.bytes(); fs::write(root.pending(), b"preserve").unwrap();
        let mut run = FileOversight::begin_open_mediated_guarded(root.store(), profile(), &expected()).unwrap();
        if complete { ready(&mut run, 1); } else { run.advance(0, 1).unwrap(); }
        assert_eq!(run.cancel().status, FileGuardedRecoveryStatus::Cancelled);
        assert_eq!(root.bytes(), before); assert_eq!(fs::read(root.pending()).unwrap(), b"preserve");
        let run = FileOversight::begin_open_mediated_guarded(root.store(), profile(), &expected()).unwrap();
        assert!(matches!(run.finish(), Err(JournalError::Contract(Error::Incomplete))));
        assert_eq!(root.bytes(), before); assert_eq!(fs::read(root.pending()).unwrap(), b"preserve");
    }
}

#[test]
fn initial_and_current_topologies_are_pinned_before_replay_and_availability_before_ready() {
    for mismatch in 0..4 {
        let root = Directory::new(); let (host, _) = start(&root); drop(host);
        let before = root.bytes(); fs::write(root.pending(), b"preserve").unwrap();
        let mut e = expected();
        match mismatch {
            0 => e.topology.initial = graph(2),
            1 => e.topology.current = graph(2),
            2 => e.topology.available = false,
            _ => e.oversight.minimum.journal_revision = u64::MAX,
        }
        if mismatch < 2 {
            assert!(matches!(FileOversight::begin_open_mediated_guarded(root.store(), profile(), &e),
                Err(JournalError::Contract(Error::Binding))));
        } else {
            let mut run = FileOversight::begin_open_mediated_guarded(root.store(), profile(), &e).unwrap();
            assert!(run.advance(0, MAX_JOURNAL_EVENTS).is_err());
            assert!(matches!(run.progress().status, FileGuardedRecoveryStatus::Failed(_)));
            assert!(run.finish().is_err());
        }
        assert_eq!(root.bytes(), before); assert_eq!(fs::read(root.pending()).unwrap(), b"preserve");
    }
}

#[test]
fn anchored_history_rejects_equal_length_forks_before_any_replay_or_cleanup() {
    let root = Directory::new(); let (host, _) = start(&root);
    let anchor = host.history_anchor().unwrap(); let mut events = host.events.clone();
    let time = events.iter_mut().find(|event| matches!(event, Event::Core(BaseEvent::Time(_)))).unwrap();
    *time = Event::Core(BaseEvent::Time(ElapsedTick(2)));
    let fork = journal::encode(&profile(), host.store.identity(), &events).unwrap(); drop(host);
    fs::write(root.store().join(storage::CANONICAL), &fork).unwrap(); fs::write(root.pending(), b"preserve").unwrap();
    assert!(matches!(FileOversight::begin_open_mediated_guarded_anchored(root.store(), profile(), &expected(), &anchor),
        Err(JournalError::Contract(Error::Binding))));
    assert_eq!(root.bytes(), fork); assert_eq!(fs::read(root.pending()).unwrap(), b"preserve");
}

#[test]
fn a_valid_anchor_does_not_skip_invalid_suffix_transitions_or_clear_the_failure_latch() {
    let root = Directory::new(); let (host, _) = start(&root);
    let anchor = host.history_anchor().unwrap(); let mut events = host.events.clone();
    events.push(Event::Mediation(MediationEvent::Update(FileMediationUpdate { operation: 99,
        expected_generation: 500, expected_authority_epoch: host.inspect().control.ledger.epoch, next: None })));
    let bad = journal::encode(&profile(), host.store.identity(), &events).unwrap(); drop(host);
    fs::write(root.store().join(storage::CANONICAL), &bad).unwrap(); fs::write(root.pending(), b"preserve").unwrap();
    let mut run = FileOversight::begin_open_mediated_guarded_anchored(root.store(), profile(), &expected(), &anchor).unwrap();
    assert!(run.advance(0, MAX_JOURNAL_EVENTS).is_err());
    let stopped = run.progress();
    let FileGuardedRecoveryStatus::Failed(error) = stopped.status else { panic!("latch original transition failure"); };
    assert_eq!(stopped.attempted_events, stopped.replayed_events + 1);
    assert_eq!(run.advance(stopped.replayed_events, 1), Err(JournalError::Contract(error)));
    assert_eq!(run.progress(), stopped);
    assert!(matches!(run.finish(), Err(JournalError::Contract(actual)) if actual == error));
    assert_eq!(root.bytes(), bad); assert_eq!(fs::read(root.pending()).unwrap(), b"preserve");
}

#[test]
fn interrupted_original_transition_cannot_resume_or_issue_any_roles() {
    let root = Directory::new(); let (host, _) = start(&root); drop(host); let before = root.bytes();
    let mut run = FileOversight::begin_open_mediated_guarded(root.store(), profile(), &expected()).unwrap();
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run.inner.advance_checked_with(0, 1, |machine, event| {
            let _ = machine.apply(event)?;
            panic!("interrupt after original transition");
        }, |_| Ok(()))
    }));
    assert!(unwind.is_err());
    assert_eq!(run.progress().status, FileGuardedRecoveryStatus::Interrupted);
    assert_eq!(run.progress().attempted_events, 1); assert_eq!(run.progress().replayed_events, 0);
    assert_eq!(run.advance(0, 1), Err(JournalError::Unavailable));
    assert!(matches!(run.finish(), Err(JournalError::Unavailable)));
    assert_eq!(root.bytes(), before);
}

#[test]
fn changed_canonical_after_ready_refuses_without_cleanup_or_overwriting_the_new_image() {
    let root = Directory::new(); let (host, _) = start(&root); drop(host);
    let mut run = FileOversight::begin_open_mediated_guarded(root.store(), profile(), &expected()).unwrap(); ready(&mut run, 1);
    fs::write(root.store().join(storage::CANONICAL), b"changed canonical").unwrap();
    fs::write(root.pending(), b"preserve").unwrap();
    assert!(matches!(run.finish(), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(root.bytes(), b"changed canonical"); assert_eq!(fs::read(root.pending()).unwrap(), b"preserve");
}

#[test]
fn every_native_replacement_failure_returns_no_owner_and_preserves_actual_visibility() {
    for barrier in BARRIERS {
        let root = Directory::new(); let (host, _) = start(&root); let revision = host.revision(); drop(host);
        let before = root.bytes();
        let mut run = FileOversight::begin_open_mediated_guarded(root.store(), profile(), &expected()).unwrap(); ready(&mut run, 1);
        run.inner.store.fail_once(barrier);
        assert!(matches!(run.finish(), Err(JournalError::Io(failure)) if failure.operation == barrier));
        let mut e = expected(); e.topology.available = barrier != JournalIo::DirectorySync;
        let disk = FileOversight::read_mediation(root.store(), &profile(), &e).unwrap();
        assert_eq!(disk.journal.revision, revision + u64::from(barrier == JournalIo::DirectorySync));
        assert_eq!(disk.topology.accepted.is_some(), barrier != JournalIo::DirectorySync);
        if barrier != JournalIo::DirectorySync { assert_eq!(root.bytes(), before); }
        let mut retry = FileOversight::begin_open_mediated_guarded(root.store(), profile(), &e).unwrap(); ready(&mut retry, 1);
        let (host, _) = retry.finish().unwrap();
        assert!(!host.mediation_snapshot().unwrap().available); assert_eq!(host.inspect().executions, 0);
    }
}
