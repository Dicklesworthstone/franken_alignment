//! Real first-image storage and original reducers; no deployment qualification.
use super::*;
use super::super::{FileMediatedRequirements, FileTopologyRequirement, journal};
use crate::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalIo, JournalLimits};
use crate::action::consequence::delivery::persistent::observed::guarded::{FileRecoveryFloor, FileRecoveryRequirements};
use crate::action::consequence::delivery::persistent::observed::guarded::recovery::FileGuardedRecoveryStatus;
use crate::action::consequence::delivery::persistent::observed::source::FileSourcePolicy;
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::mediation::{CutCheck, MAX_CHECK_EDGE_VISITS};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::action::consequence::oversight::policy_state::{StateFreshness, StateLimits, StateSource};
use crate::full_input::InputProfileBinding;
use crate::perimeter_inventory::topology::LoadedAuthorityTopology;
use crate::reducer::Caps;
use crate::witness::refinement::RefinementBudget;
use std::{collections::BTreeMap, fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-reserved-topology-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("reserved topology cleanup: {error}"); }
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
            policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".into(),
                MemberPolicy { cohort: "one".into(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
                narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("reviewer".into(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"mediated-fixture".to_vec(),
                tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn graph() -> AuthorityGraph {
    let inventory = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/authority_topology_inventory.json"));
    let topology = br#"{"schema":"fa.authority-topology/1","generation":1,"inventory_generation":1,
      "complete_inventory_generation":1,
      "scope":{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5,"purpose":"effect"},
      "target":{"adapter":10,"object":11,"contract_version":1,"expected_version":1,"generation":1},
      "inventory_scope":{"tenant":1,"principal":2,"purpose":1},"family":"file-publication",
      "nodes":[{"id":1,"kind":"actor"},{"id":2,"kind":"enforcer"},{"id":4,"kind":"sink"}],
      "edges":[{"id":1,"from":1,"to":2,"channel":"dispatch","route":"publication","provenance":101},
               {"id":2,"from":2,"to":4,"channel":"dispatch","route":"publication","provenance":102}],
      "enforcers":[{"node":2,"adapter":10,"contract_version":1,"generation":1,"provenance":102}],
      "gates":[2],"check_edge_budget":1000,"source_limits":{"checks":32,"read_bytes":134217728}}"#;
    LoadedAuthorityTopology::from_json_bytes(topology, inventory).unwrap().graph().clone()
}
fn guards(source: bool) -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None,
        source: source.then(|| FileSourcePolicy {
            source: StateSource { scope: profile().delivery.scope, source: 51, generation: 1 },
            limits: StateLimits { events: 32, retained_bytes: 4096 },
            freshness: StateFreshness::new(50).unwrap(),
        }), identity: None, campaigns: None, credential: None }
}
fn bootstrap(witnesses: bool) -> FileMediatedBootstrap {
    FileMediatedBootstrap { topology: graph(), reserve: RecoveryReserve::terminal(),
        publication: witnesses.then_some(PublicationLimits { bindings: 4,
            validation: RefinementBudget { steps: 1000, value_bytes: 4096 } }) }
}
fn expected(source: bool) -> FileMediatedRequirements {
    FileMediatedRequirements { oversight: FileRecoveryRequirements {
        guards: guards(source), effective_policy: profile().delivery.policy, credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: 0, control_sequence: 0, authority_epoch: 0 },
    }, topology: FileTopologyRequirement { initial: graph(), current: graph(), available: true },
        prediction: None, evaluation: None }
}
fn check_image(host: &FileOversight, source: bool, witnesses: bool) {
    assert_eq!(host.journal_capacity().unwrap().reserve(), Some(RecoveryReserve::terminal()));
    assert_eq!(host.file_source_status().map(|state| state.policy), guards(source).source);
    assert_eq!(host.publication_validation_profile().unwrap(), bootstrap(witnesses).publication);
    assert!(host.publication_guard_required());
    let topology = host.mediation_snapshot().unwrap();
    assert_eq!(topology.graph, graph()); assert!(topology.available); assert!(topology.accepted.is_none());
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.available, profile().delivery.total);
    assert_eq!(host.revision(), if source { 4 } else { 3 });
}

#[test]
fn native_first_image_contains_reserve_source_topology_and_exact_witness_selection() {
    for source in [false, true] { for witnesses in [false, true] {
        let root = Directory::new();
        let (mut host, roles) = FileOversight::create_mediated_guarded_reserved(root.store(), profile(),
            &guards(source), None, bootstrap(witnesses), None, None).unwrap();
        check_image(&host, source, witnesses);
        let bytes = fs::read(root.store().join(storage::CANONICAL)).unwrap();
        let events = journal::decode(&profile(), host.store.identity(), &bytes).unwrap();
        assert_eq!(events.len(), host.revision() as usize);
        assert!(matches!(events[1], super::super::super::super::Event::Core(
            super::super::super::super::BaseEvent::ReserveRecovery(_))));
        assert!(roles.consistency_observer.is_none()); assert!(roles.evaluator.is_none());
        // Bootstrap itself was not a cut. The separately held native observer
        // can positively certify after a real current clock observation.
        let revision = host.revision(); host.observe_time(revision, ElapsedTick(1)).unwrap();
        let proposal = graph().propose_cut(&[2]).unwrap();
        let revision = host.revision(); let epoch = host.inspect().control.ledger.epoch;
        assert!(matches!(roles.topology_observer.certify(&mut host, revision, epoch,
            &proposal, MAX_CHECK_EDGE_VISITS).unwrap().unwrap(), CutCheck::Verified(_)));
        assert!(host.mediation_snapshot().unwrap().accepted.is_some());
        assert_eq!(host.inspect().executions, 0);
    } }
}

#[test]
fn source_and_reserve_survive_original_guarded_recovery_while_coverage_is_withdrawn() {
    let root = Directory::new();
    let (host, old_roles) = FileOversight::create_mediated_guarded_reserved(root.store(), profile(),
        &guards(true), None, bootstrap(false), None, None).unwrap();
    let before = host.revision(); drop(host);
    let mut recovery = FileOversight::begin_open_mediated_guarded(root.store(), profile(), &expected(true))
        .unwrap().require_recovery_reserve(RecoveryReserve::terminal()).unwrap();
    while recovery.progress().status == FileGuardedRecoveryStatus::Replaying {
        recovery.advance(recovery.progress().replayed_events, 1).unwrap();
    }
    let (mut recovered, fresh_roles) = recovery.finish().unwrap();
    assert_eq!(recovered.revision(), before + 1);
    assert_eq!(recovered.journal_capacity().unwrap().reserve(), Some(RecoveryReserve::terminal()));
    assert_eq!(recovered.file_source_status().unwrap().policy, guards(true).source.unwrap());
    assert!(!recovered.mediation_snapshot().unwrap().available);
    assert!(recovered.mediation_snapshot().unwrap().accepted.is_none());
    let proposal = graph().propose_cut(&[2]).unwrap();
    let revision = recovered.revision(); let epoch = recovered.inspect().control.ledger.epoch;
    assert!(old_roles.topology_observer.certify(&mut recovered, revision, epoch,
        &proposal, MAX_CHECK_EDGE_VISITS).is_err());
    // Fresh custody cannot silently re-certify a graph withdrawn by recovery.
    let revision = recovered.revision();
    assert!(!matches!(fresh_roles.topology_observer.certify(&mut recovered, revision, epoch,
        &proposal, MAX_CHECK_EDGE_VISITS), Ok(Ok(CutCheck::Verified(_)))));
}

#[test]
fn all_original_persistence_barriers_return_no_partial_configuration() {
    for witnesses in [false, true] {
        for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
            JournalIo::Rename, JournalIo::DirectorySync] {
            let root = Directory::new();
            let prepared = prepare(profile(), &guards(true), None, bootstrap(witnesses), None, None).unwrap();
            let store = storage::Store::create(&root.store()).unwrap(); store.fail_once(barrier);
            let Err(JournalError::Io(failure)) = prepared.publish(store) else { panic!("expected original Store failure"); };
            assert_eq!(failure.operation, barrier);
            assert_eq!(failure.replacement_may_be_visible, matches!(barrier, JournalIo::Rename | JournalIo::DirectorySync));
            let canonical = root.store().join(storage::CANONICAL);
            assert_eq!(canonical.exists(), barrier == JournalIo::DirectorySync);
            if canonical.exists() {
                // A lost final acknowledgment can leave ONLY the whole image.
                let store = storage::Store::open(&root.store()).unwrap();
                let bytes = store.read(profile().delivery.limits.bytes).unwrap();
                let events = journal::decode(&profile(), store.identity(), &bytes).unwrap();
                let machine = super::super::super::super::Machine::replay(&profile(), &events).unwrap();
                let (host, _) = FileOversight::owner(profile(), store, events, machine);
                check_image(&host, true, witnesses);
            }
        }
    }
}

#[test]
fn invalid_context_reserve_witness_and_capacity_refuse_before_store_creation() {
    for mode in 0..4 {
        let root = Directory::new(); let mut profile = profile();
        let guards = guards(true); let mut selected = bootstrap(true);
        match mode {
            0 => selected.reserve.events = 0,
            1 => selected.publication.as_mut().unwrap().bindings = 0,
            2 => profile.delivery.scope.run += 1,
            _ => profile.delivery.limits.events = 1,
        }
        assert!(FileOversight::create_mediated_guarded_reserved(root.store(), profile, &guards,
            None, selected, None, None).is_err());
        assert!(!root.store().exists());
    }
}

#[test]
fn invalid_native_preparation_never_replaces_an_existing_store() {
    let root = Directory::new();
    let (host, _) = FileOversight::create_mediated_guarded_reserved(root.store(), profile(),
        &guards(true), None, bootstrap(false), None, None).unwrap();
    let bytes = fs::read(root.store().join(storage::CANONICAL)).unwrap();
    drop(host);
    let mut invalid = bootstrap(false); invalid.reserve.bytes = 0;
    assert!(FileOversight::create_mediated_guarded_reserved(root.store(), profile(),
        &guards(true), None, invalid, None, None).is_err());
    assert_eq!(fs::read(root.store().join(storage::CANONICAL)).unwrap(), bytes);
    assert!(FileOversight::create_mediated_guarded_reserved(root.store(), profile(),
        &guards(true), None, bootstrap(false), None, None).is_err());
    assert_eq!(fs::read(root.store().join(storage::CANONICAL)).unwrap(), bytes);
}
