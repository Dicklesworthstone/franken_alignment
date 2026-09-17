//! Original canonical replacement barriers, not hardware power-loss evidence.
use super::*;
use crate::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalIo, JournalLimits};
use crate::action::consequence::delivery::persistent::observed::mediation::FileMediationUpdate;
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::mediation::{Channel, Completeness, CutCheck, Edge, Enforcer, GraphSpec, Node, NodeKind, MAX_CHECK_EDGE_VISITS};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::full_input::InputProfileBinding;
use crate::perimeter::*;
use crate::reducer::Caps;
use super::super::FileRecoveryFloor;
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
        let root = std::env::temp_dir().join(format!("fa-mediated-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&root).unwrap(); Self(root)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("mediated cleanup: {error}"); } }
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
    FileOversight::create_mediated_guarded(root.store(), profile(), &expected().oversight.guards,
        None, graph(1), None, None).unwrap()
}
fn certify(host: &mut FileOversight, role: &FileMediationObserver) {
    let proposal = host.mediation_snapshot().unwrap().graph.propose_cut(&[2]).unwrap();
    let rev = host.revision(); let epoch = host.inspect().control.ledger.epoch;
    assert!(matches!(role.certify(host, rev, epoch, &proposal, MAX_CHECK_EDGE_VISITS).unwrap().unwrap(), CutCheck::Verified(_)));
}
fn failure(error: JournalError, barrier: JournalIo) {
    let JournalError::Io(failure) = error else { panic!("expected original Store failure"); };
    assert_eq!(failure.operation, barrier);
    assert_eq!(failure.replacement_may_be_visible, matches!(barrier, JournalIo::Rename | JournalIo::DirectorySync));
}

#[test]
fn first_replacement_never_returns_partial_configuration_or_roles() {
    for barrier in BARRIERS {
        let root = Directory::new(); let e = expected();
        let prepared = PreparedGuardedBootstrap::prepare(profile(), &e.oversight.guards, None).unwrap().mediated(graph(1)).unwrap();
        let store = storage::Store::create(&root.store()).unwrap(); store.fail_once(barrier);
        failure(prepared.publish(store).unwrap_err(), barrier);
        assert_eq!(root.store().join(storage::CANONICAL).exists(), barrier == JournalIo::DirectorySync);
        if barrier == JournalIo::DirectorySync {
            let disk = FileOversight::read_mediation(root.store(), &profile(), &e).unwrap();
            assert_eq!(disk.journal.revision, 2); assert!(disk.topology.available); assert!(disk.topology.accepted.is_none());
            let (mut h, roles) = FileOversight::open_mediated_guarded(root.store(), profile(), &e).unwrap();
            assert_eq!(h.revision(), 3); assert!(!h.mediation_snapshot().unwrap().available);
            h.observe_time(h.revision(), ElapsedTick(1)).unwrap();
            let update = FileMediationUpdate { operation: 1, expected_generation: 1,
                expected_authority_epoch: h.inspect().control.ledger.epoch, next: Some(graph(2)) };
            let rev = h.revision(); roles.topology_observer.update(&mut h, rev, &update).unwrap();
            certify(&mut h, &roles.topology_observer);
        } else { assert!(FileOversight::open_mediated_guarded(root.store(), profile(), &e).is_err()); }
    }
}

#[test]
fn failed_recovery_returns_no_role_and_reader_reports_the_actual_visibility() {
    for barrier in BARRIERS {
        let root = Directory::new(); let (mut host, old) = start(&root);
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); certify(&mut host, &old.topology_observer);
        let revision = host.revision(); drop(host);
        let store = storage::Store::open(&root.store()).unwrap(); store.fail_once(barrier);
        failure(FileOversight::open_mediated_store(store, profile(), &expected()).unwrap_err(), barrier);
        let mut e = expected(); e.topology.available = barrier != JournalIo::DirectorySync;
        let disk = FileOversight::read_mediation(root.store(), &profile(), &e).unwrap();
        assert_eq!(disk.journal.revision, revision + u64::from(barrier == JournalIo::DirectorySync));
        assert_eq!(disk.topology.accepted.is_some(), barrier != JournalIo::DirectorySync);
        let (mut h, fresh) = FileOversight::open_mediated_guarded(root.store(), profile(), &e).unwrap();
        assert!(!h.mediation_snapshot().unwrap().available);
        let update = FileMediationUpdate { operation: 1, expected_generation: 1,
            expected_authority_epoch: h.inspect().control.ledger.epoch, next: Some(graph(2)) };
        let rev = h.revision();
        assert_eq!(old.topology_observer.update(&mut h, rev, &update), Err(JournalError::Contract(Error::Binding)));
        fresh.topology_observer.update(&mut h, rev, &update).unwrap(); certify(&mut h, &fresh.topology_observer);
    }
}

#[test]
fn canonical_cut_and_update_results_are_never_confused_with_lost_acknowledgments() {
    for changing in [false, true] { for barrier in BARRIERS {
        let root = Directory::new(); let (mut h, roles) = start(&root);
        h.observe_time(h.revision(), ElapsedTick(1)).unwrap();
        if changing { certify(&mut h, &roles.topology_observer); }
        let before = h.inspect(); let rev = h.revision(); let epoch = before.control.ledger.epoch;
        let proposal = graph(1).propose_cut(&[2]).unwrap();
        let update = FileMediationUpdate { operation: 10, expected_generation: 1, expected_authority_epoch: epoch,
            next: Some(graph(2)) };
        h.store.fail_once(barrier);
        let error = if changing { roles.topology_observer.update(&mut h, rev, &update).unwrap_err() }
            else { roles.topology_observer.certify(&mut h, rev, epoch, &proposal, MAX_CHECK_EDGE_VISITS).unwrap_err() };
        failure(error, barrier); assert_eq!(h.inspect(), before);
        assert_eq!(h.mediation_snapshot(), Err(JournalError::Unavailable));
        let mut e = expected(); let visible = barrier == JournalIo::DirectorySync;
        if changing && visible { e.topology.current = graph(2); }
        let disk = FileOversight::read_mediation(root.store(), &profile(), &e).unwrap();
        assert_eq!(disk.journal.revision, rev + u64::from(visible));
        assert_eq!(disk.topology.accepted.is_some(), if changing { !visible } else { visible });
        assert_eq!(disk.updates.len(), usize::from(changing && visible));
        if changing && visible {
            assert_eq!(disk.updates[0].request, update); assert_eq!(disk.updates[0].journal_revision, rev + 1);
            assert_eq!(disk.updates[0].change.as_ref().unwrap().current, Some(graph(2)));
        }
        drop(h);
        let (mut h, fresh) = FileOversight::open_mediated_guarded(root.store(), profile(), &e).unwrap();
        if changing && visible {
            let rev = h.revision();
            assert_eq!(fresh.topology_observer.update(&mut h, 0, &update).unwrap(), disk.updates[0].change);
            assert_eq!(h.revision(), rev); assert!(!h.mediation_snapshot().unwrap().available);
        }
    } }
}

#[test]
fn duplicate_bootstrap_and_impossible_suffix_refuse_without_cleanup() {
    let initial = Event::Mediation(MediationEvent::Enable(graph(1)));
    assert!(check_topology(std::slice::from_ref(&initial), Some(&graph(1))).is_ok());
    assert_eq!(check_topology(std::slice::from_ref(&initial), None), Err(Error::Binding));
    assert_eq!(check_topology(&[initial.clone(), initial], Some(&graph(1))), Err(Error::Binding));
    let root = Directory::new(); let (mut h, roles) = start(&root);
    h.observe_time(h.revision(), ElapsedTick(1)).unwrap(); certify(&mut h, &roles.topology_observer);
    let e = expected(); let path = h.store.identity().to_path_buf();
    let mut events = h.events.clone();
    events.push(Event::Mediation(MediationEvent::Update(FileMediationUpdate { operation: 99,
        expected_generation: 500, expected_authority_epoch: h.inspect().control.ledger.epoch, next: None })));
    let bad = journal::encode(&profile(), &path, &events).unwrap();
    std::fs::write(path.join(storage::CANONICAL), &bad).unwrap();
    std::fs::write(path.join("delivery.pending"), b"preserve").unwrap();
    assert!(FileOversight::read_mediation(&path, &profile(), &e).is_err());
    assert_eq!(std::fs::read(path.join(storage::CANONICAL)).unwrap(), bad);
    assert_eq!(std::fs::read(path.join("delivery.pending")).unwrap(), b"preserve");
}
