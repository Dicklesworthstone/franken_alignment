//! Actual native credential transitions preserve the original source-work cut.
use super::*;
use crate::action::consequence::delivery::credential_broker::{
    BrokerRouteBinding, CredentialRevocationRequest, CredentialRotationRequest,
};
use crate::action::consequence::delivery::persistent::observed::{
    BaseEvent, Event, journal,
    credential::{FILE_OVERSIGHT_CREDENTIAL_PROFILE, FileCredentialPolicy},
    guarded::{FileCredentialEpoch, FileCredentialRegistration,
        mediated::FileMediatedRecovery},
};
use crate::perimeter_inventory::LoadedPerimeterInventory;

fn active() -> (Directory, FileOversight, FileDriverTopology, TopologyCheckpoint) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let root = Directory(std::env::temp_dir().join(format!(
        "fa-credential-checkpoint-{}-{stamp}-{}",
        std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))));
    fs::create_dir(&root.0).unwrap();
    let topology = document(1);
    let inventory_text = String::from_utf8(INVENTORY.to_vec()).unwrap()
        .replace("reference-publication", FILE_OVERSIGHT_CREDENTIAL_PROFILE);
    fs::write(root.0.join("topology.json"), &topology).unwrap();
    fs::write(root.0.join("inventory.json"), &inventory_text).unwrap();
    let inventory = LoadedPerimeterInventory::from_json_bytes(inventory_text.as_bytes()).unwrap();
    let loaded = load(topology.as_bytes(), inventory_text.as_bytes()).unwrap();
    let binding = BrokerRouteBinding { family: "file-publication".into(), route: "publication".into() };
    let mut guards = guards();
    guards.credential = Some(FileCredentialPolicy::from_inventory(&inventory, &binding,
        profile().delivery.scope, profile().delivery.target).unwrap());
    let (mut host, roles) = FileOversight::create_mediated_guarded_reserved(
        root.0.join("store"), profile(), &guards,
        Some(FileCredentialRegistration { inventory: &inventory, binding: &binding }),
        FileMediatedBootstrap { topology: loaded.graph().clone(),
            reserve: RecoveryReserve::terminal(), publication: None }, None, None).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let source = roles.topology_observer.bind_topology_source(&host, host.revision(),
        root.0.join("topology.json"), root.0.join("inventory.json"), loaded).unwrap();
    let mut source = FileDriverTopology::new(source);
    refresh(&mut source, &mut host);
    let saved = source.checkpoint(&host, topology.as_bytes(), inventory_text.as_bytes()).unwrap();
    assert!(saved.available());
    assert_eq!(saved.work().checks, 1);
    (root, host, source, saved)
}

fn ready(root: &Directory, saved: &TopologyCheckpoint,
    generation: u64, revoked: bool) -> FileMediatedRecovery
{
    let inventory = LoadedPerimeterInventory::from_json_bytes(saved.inventory_bytes()).unwrap();
    let binding = BrokerRouteBinding { family: "file-publication".into(), route: "publication".into() };
    let mut guards = guards();
    guards.credential = Some(FileCredentialPolicy::from_inventory(&inventory, &binding,
        profile().delivery.scope, profile().delivery.target).unwrap());
    let expected = FileMediatedRequirements {
        oversight: FileRecoveryRequirements { guards,
            effective_policy: profile().delivery.policy,
            credential_epoch: Some(FileCredentialEpoch { generation, revoked }),
            minimum: saved.minimum() },
        topology: FileTopologyRequirement {
            initial: saved.expected().graph().clone(),
            current: saved.expected().graph().clone(), available: saved.available() },
        prediction: None, evaluation: None,
    };
    let mut run = FileOversight::begin_open_mediated_guarded_anchored(root.0.join("store"),
        profile(), &expected, saved.history_anchor()).unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal()).unwrap();
    while run.progress().status == FileGuardedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1).unwrap();
    }
    assert_eq!(run.progress().status, FileGuardedRecoveryStatus::Ready);
    run
}

fn same_source(before: &TopologyCheckpoint, after: &TopologyCheckpoint) {
    assert_eq!(after.topology_bytes(), before.topology_bytes());
    assert_eq!(after.inventory_bytes(), before.inventory_bytes());
    assert_eq!(after.expected(), before.expected());
    assert_eq!(after.work(), before.work());
    assert_eq!(after.last_operation(), before.last_operation());
    let bytes = archive(after);
    assert_eq!(&TopologyCheckpoint::read_trusted(
        &mut Cursor::new(&bytes), bytes.len()).unwrap(), after);
}

fn rotation(operation: u64, generation: u64) -> CredentialRotationRequest {
    CredentialRotationRequest { operation, expected_generation: generation,
        next_generation: generation + 1 }
}

#[test]
fn credential_checkpoint_tracks_fence_rotation_revocation_and_old_exact_receipts() {
    let (root, host, source, original) = active();
    drop(source); drop(host);
    // Maintenance and capture consume retained identities, not today's source.
    fs::remove_file(root.0.join("topology.json")).unwrap();
    fs::remove_file(root.0.join("inventory.json")).unwrap();
    let (mut host, _roles) = ready(&root, &original, 1, false).finish().unwrap();
    let fenced = original.after_credential_lifecycle(&host).unwrap();
    same_source(&original, &fenced);
    assert_eq!(fenced.history_anchor().revision(), original.history_anchor().revision() + 1);
    assert!(!fenced.available());
    assert!(host.mediation_snapshot().unwrap().accepted.is_none());
    let ledger = host.inspect().control.ledger;
    let first = rotation(501, 1);
    let receipt = host.rotate_credential_guard(host.revision(), first).unwrap();
    let rotated = original.after_credential_lifecycle(&host).unwrap();
    same_source(&fenced, &rotated);
    assert_eq!(rotated.history_anchor(), &host.history_anchor().unwrap());
    assert_eq!(rotated.minimum().journal_revision, host.revision());
    assert_eq!(rotated.minimum().control_sequence, host.inspect().control.sequence);
    assert_eq!(rotated.minimum().authority_epoch, host.inspect().control.ledger.epoch);
    assert!(!rotated.available());
    let revision = host.revision();
    assert_eq!(host.rotate_credential_guard(0, first).unwrap(), receipt);
    assert_eq!(host.revision(), revision);
    assert_eq!(original.after_credential_lifecycle(&host).unwrap(), rotated);

    host.rotate_credential_guard(host.revision(), rotation(502, 2)).unwrap();
    assert_eq!(host.rotate_credential_guard(0, first).unwrap().generation, 2);
    assert_eq!(host.credential_status().unwrap().unwrap().generation, 3);
    let revoked = host.revoke_credential_guard(host.revision(),
        CredentialRevocationRequest { operation: 503, expected_generation: 3 }).unwrap();
    assert!(revoked.revoked);
    let latest = original.after_credential_lifecycle(&host).unwrap();
    same_source(&original, &latest);
    assert_eq!(latest.history_anchor(), &host.history_anchor().unwrap());
    assert_eq!(host.inspect().control.ledger, ledger);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.credential_status().unwrap().unwrap().retained_changes, 3);
    assert_eq!(rotated.after_credential_lifecycle(&host).unwrap(), latest);
    let bytes = fs::read(root.0.join("store/delivery.bin")).unwrap();
    assert_eq!(original.after_credential_lifecycle(&host).unwrap(), latest);
    assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), bytes);
    drop(host);
    let (host, _roles) = ready(&root, &latest, 3, true).finish().unwrap();
    let recovered = latest.after_credential_lifecycle(&host).unwrap();
    same_source(&latest, &recovered);
    assert!(!recovered.available());
    assert!(host.credential_status().unwrap().unwrap().revoked);
}

#[test]
fn credential_checkpoint_refuses_source_and_ordinary_work_until_the_actual_cut_is_retained() {
    for later_source in [false, true] {
        let (root, mut host, mut source, old) = active();
        if later_source { refresh(&mut source, &mut host); }
        else { host.observe_time(host.revision(), ElapsedTick(2)).unwrap(); }
        let latest = source.checkpoint(&host, old.topology_bytes(), old.inventory_bytes()).unwrap();
        drop(source); drop(host);
        let (mut host, _roles) = ready(&root, &latest, 1, false).finish().unwrap();
        host.rotate_credential_guard(host.revision(), rotation(510, 1)).unwrap();
        let bytes = fs::read(root.0.join("store/delivery.bin")).unwrap();
        let revision = host.revision();
        assert_eq!(old.after_credential_lifecycle(&host),
            Err(JournalError::Contract(Error::Stale)));
        let current = latest.after_credential_lifecycle(&host).unwrap();
        same_source(&latest, &current);
        assert_eq!(current.work().checks, if later_source { 2 } else { 1 });
        assert_eq!(host.revision(), revision);
        assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), bytes);
    }
}

#[test]
fn credential_checkpoint_binds_original_owner_profile_graph_prefix_and_floors() {
    let (root, host, source, saved) = active();
    let (_foreign_root, foreign_host, foreign_source, foreign) = active();
    drop(foreign_source); drop(foreign_host); drop(source); drop(host);
    let (mut host, _roles) = ready(&root, &saved, 1, false).finish().unwrap();
    host.rotate_credential_guard(host.revision(), rotation(520, 1)).unwrap();
    let bytes = fs::read(root.0.join("store/delivery.bin")).unwrap();
    let identity = fs::canonicalize(root.0.join("store")).unwrap();
    assert!(foreign.after_credential_lifecycle(&host).is_err());
    for change in ["graph", "availability", "control", "epoch", "profile", "equal-counter-fork"] {
        let mut changed = saved.clone();
        match change {
            "graph" => changed.expected = load(document(2).as_bytes(), saved.inventory_bytes()).unwrap(),
            "availability" => changed.available = false,
            "control" => changed.control_sequence = u64::MAX,
            "epoch" => changed.authority_epoch = u64::MAX,
            "profile" | "equal-counter-fork" => {
                let mut events = saved.history.decoded_events(&profile(), &identity).unwrap();
                let mut selected_profile = profile();
                if change == "profile" { selected_profile.delivery.total += 1; }
                else {
                    let event = events.iter_mut().find(|event|
                        matches!(event, Event::Core(BaseEvent::Time(_)))).unwrap();
                    let Event::Core(BaseEvent::Time(tick)) = event else { unreachable!() };
                    *tick = ElapsedTick(2);
                }
                changed.history = FileHistoryAnchor::from_acknowledged(events.len(),
                    journal::encode(&selected_profile, &identity, &events).unwrap());
                assert_eq!(changed.minimum(), saved.minimum());
            }
            _ => unreachable!(),
        }
        assert!(changed.after_credential_lifecycle(&host).is_err(), "{change}");
    }
    let current = saved.after_credential_lifecycle(&host).unwrap();
    same_source(&saved, &current);
    assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), bytes);
}

#[test]
fn credential_checkpoint_never_certifies_an_unacknowledged_storage_candidate() {
    let barriers = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
        JournalIo::Rename, JournalIo::DirectorySync];
    for revoke in [false, true] {
        for barrier in barriers {
            let (root, host, source, saved) = active();
            drop(source); drop(host);
            let (mut host, _roles) = ready(&root, &saved, 1, false).finish().unwrap();
            let fenced = saved.after_credential_lifecycle(&host).unwrap();
            let retained = archive(&fenced);
            host.store.fail_once(barrier);
            let result = if revoke {
                host.revoke_credential_guard(host.revision(),
                    CredentialRevocationRequest { operation: 530, expected_generation: 1 })
            } else { host.rotate_credential_guard(host.revision(), rotation(530, 1)) };
            assert!(result.is_err(), "{barrier:?}");
            assert!(host.storage_failure().is_some());
            assert_eq!(saved.after_credential_lifecycle(&host), Err(JournalError::Unavailable));
            assert_eq!(fenced.after_credential_lifecycle(&host), Err(JournalError::Unavailable));
            let restored = TopologyCheckpoint::read_trusted(
                &mut Cursor::new(&retained), retained.len()).unwrap();
            assert_eq!(restored, fenced);
            same_source(&saved, &restored);
            assert_eq!(restored.history_anchor().revision(), saved.history_anchor().revision() + 1);
        }
    }
}

#[test]
fn credential_checkpoint_does_not_admit_unregistered_guards_or_terminal_events() {
    let (_root, host, source, text) = fixture();
    let unguarded = source.checkpoint(&host, text.as_bytes(), INVENTORY).unwrap();
    assert_eq!(unguarded.after_credential_lifecycle(&host), Err(JournalError::Contract(Error::Missing)));
    let (_root, mut host, _source, saved) = active();
    let state = host.inspect();
    host.request_stop(host.revision(), crate::action::consequence::delivery::StopRequest {
        operation: 540, expected_control_sequence: state.control.sequence,
        expected_authority_epoch: state.control.ledger.epoch,
    }).unwrap();
    assert_eq!(saved.after_credential_lifecycle(&host), Err(JournalError::Contract(Error::Stale)));
}

