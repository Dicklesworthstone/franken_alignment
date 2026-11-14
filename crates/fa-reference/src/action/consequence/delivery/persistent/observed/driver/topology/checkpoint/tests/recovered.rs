//! Only original, unbound observer custody may carry one recovery Fence.
use super::*;
use crate::action::consequence::delivery::persistent::observed::{
    guarded::mediated::FileMediatedRoles,
    mediation::FileMediationUpdate,
};

fn reopen(root: &Directory, saved: &TopologyCheckpoint) -> (FileOversight, FileMediatedRoles) {
    let expected = FileMediatedRequirements {
        oversight: FileRecoveryRequirements {
            guards: guards(),
            effective_policy: profile().delivery.policy,
            credential_epoch: None,
            minimum: saved.minimum(),
        },
        topology: FileTopologyRequirement {
            initial: load(document(1).as_bytes(), INVENTORY).unwrap().graph().clone(),
            current: saved.expected().graph().clone(),
            available: saved.available(),
        },
        prediction: None,
        evaluation: None,
    };
    let mut run = FileOversight::begin_open_mediated_guarded_anchored(
        root.0.join("store"), profile(), &expected, saved.history_anchor(),
    ).unwrap().require_recovery_reserve(RecoveryReserve::terminal()).unwrap();
    while run.progress().status == FileGuardedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1).unwrap();
    }
    run.finish().unwrap()
}

fn same_source(before: &TopologyCheckpoint, after: &TopologyCheckpoint) {
    assert_eq!(after.topology_bytes(), before.topology_bytes());
    assert_eq!(after.inventory_bytes(), before.inventory_bytes());
    assert_eq!(after.expected(), before.expected());
    assert_eq!(after.work(), before.work());
    assert_eq!(after.last_operation(), before.last_operation());
    assert!(!after.available());
    let bytes = archive(after);
    assert_eq!(
        TopologyCheckpoint::read_trusted(&mut Cursor::new(&bytes), bytes.len()).unwrap(),
        *after,
    );
}

#[test]
fn unbound_recovered_observer_preserves_exact_source_work_without_live_files_or_extra_events() {
    for checks in [0, 1] {
        let (root, mut host, mut source, document) = fixture();
        for _ in 0..checks { refresh(&mut source, &mut host); }
        let saved = source.checkpoint(&host, document.as_bytes(), INVENTORY).unwrap();
        drop(source);
        drop(host);
        fs::remove_file(root.0.join("topology.json")).unwrap();
        fs::remove_file(root.0.join("inventory.json")).unwrap();
        let (host, roles) = reopen(&root, &saved);
        let before = fs::read(root.0.join("store/delivery.bin")).unwrap();
        let state = host.inspect();
        let successor = roles.topology_observer
            .recovered_topology_checkpoint(&host, &saved).unwrap();
        same_source(&saved, &successor);
        assert_eq!(successor.history_anchor(), &host.history_anchor().unwrap());
        assert_eq!(successor.history_anchor().revision(), saved.history_anchor().revision() + 1);
        assert_eq!(successor.minimum().control_sequence, state.control.sequence);
        assert_eq!(successor.minimum().authority_epoch, state.control.ledger.epoch);
        assert_eq!(successor.work().checks, checks);
        assert_eq!(host.inspect(), state);
        assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), before);
        assert_eq!(roles.topology_observer
            .recovered_topology_checkpoint(&host, &saved).unwrap(), successor);
        assert!(roles.topology_observer
            .recovered_topology_checkpoint(&host, &successor).is_err(),
            "an equal cut is not a newly acknowledged recovery Fence");
        let previous_observer = roles.topology_observer;
        drop(host);
        let (host, roles) = reopen(&root, &successor);
        assert_eq!(previous_observer.recovered_topology_checkpoint(&host, &successor),
            Err(JournalError::Contract(Error::Binding)),
            "an old observer cannot bind a newly reopened allocation with the same path");
        let repeated = roles.topology_observer
            .recovered_topology_checkpoint(&host, &successor).unwrap();
        same_source(&successor, &repeated);
        assert_eq!(repeated.history_anchor().revision(), successor.history_anchor().revision() + 1);
        assert!(roles.topology_observer
            .recovered_topology_checkpoint(&host, &saved).is_err(),
            "two recovery Fences cannot explain one exact source successor");
    }
}

#[test]
fn recovery_source_successor_refuses_source_time_and_topology_work_beyond_the_saved_cut() {
    for source_work in [false, true] {
        let (root, mut host, mut source, document) = fixture();
        let old = source.checkpoint(&host, document.as_bytes(), INVENTORY).unwrap();
        if source_work {
            refresh(&mut source, &mut host);
        } else {
            host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        }
        let latest = source.checkpoint(&host, document.as_bytes(), INVENTORY).unwrap();
        drop(source);
        drop(host);
        let (mut host, roles) = reopen(&root, &latest);
        let canonical = fs::read(root.0.join("store/delivery.bin")).unwrap();
        assert!(roles.topology_observer.recovered_topology_checkpoint(&host, &old).is_err());
        assert!(roles.topology_observer.recovered_topology_checkpoint(&host, &latest).is_ok());
        assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), canonical);
        if source_work {
            let update = FileMediationUpdate {
                operation: latest.last_operation() + 1,
                expected_generation: 1,
                expected_authority_epoch: host.inspect().control.ledger.epoch,
                next: Some(load(super::document(2).as_bytes(), INVENTORY).unwrap().graph().clone()),
            };
            let revision = host.revision();
            roles.topology_observer.update(&mut host, revision, &update).unwrap();
        } else {
            host.observe_time(host.revision(), ElapsedTick(3)).unwrap();
        }
        let canonical = fs::read(root.0.join("store/delivery.bin")).unwrap();
        assert!(roles.topology_observer.recovered_topology_checkpoint(&host, &latest).is_err());
        assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), canonical);
    }
}

#[test]
fn recovery_source_successor_binds_original_issuer_prefix_graph_work_and_floors() {
    let (root, host, source, document) = fixture();
    let saved = source.checkpoint(&host, document.as_bytes(), INVENTORY).unwrap();
    drop(source);
    drop(host);
    let (host, roles) = reopen(&root, &saved);
    let (_other_root, other, other_source, other_document) = fixture();
    let foreign = other_source.checkpoint(&other, other_document.as_bytes(), INVENTORY).unwrap();
    assert!(roles.topology_observer.recovered_topology_checkpoint(&other, &saved).is_err());
    let canonical = fs::read(root.0.join("store/delivery.bin")).unwrap();
    for mismatch in ["owner", "graph", "availability", "control", "epoch", "work"] {
        let mut bad = saved.clone();
        match mismatch {
            "owner" => bad = foreign.clone(),
            "graph" => bad.expected = load(super::document(2).as_bytes(), INVENTORY).unwrap(),
            "availability" => bad.available = !bad.available,
            "control" => bad.control_sequence = u64::MAX,
            "epoch" => bad.authority_epoch = u64::MAX,
            "work" => bad.work.reserved_read_bytes = 1,
            _ => unreachable!(),
        }
        assert!(roles.topology_observer.recovered_topology_checkpoint(&host, &bad).is_err(),
            "{mismatch}");
        assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), canonical);
    }
    assert!(roles.topology_observer.recovered_topology_checkpoint(&host, &saved).is_ok());
}

#[test]
fn recovery_source_successor_cannot_capture_a_poisoned_owner_after_failed_source_binding() {
    let (root, host, source, document) = fixture();
    let saved = source.checkpoint(&host, document.as_bytes(), INVENTORY).unwrap();
    drop(source);
    drop(host);
    let (mut host, roles) = reopen(&root, &saved);
    let successor = roles.topology_observer.recovered_topology_checkpoint(&host, &saved).unwrap();
    let canonical = fs::read(root.0.join("store/delivery.bin")).unwrap();
    host.store.fail_once(JournalIo::DirectorySync);
    let update = FileMediationUpdate {
        operation: saved.last_operation() + 1,
        expected_generation: 1,
        expected_authority_epoch: host.inspect().control.ledger.epoch,
        next: Some(load(super::document(2).as_bytes(), INVENTORY).unwrap().graph().clone()),
    };
    let revision = host.revision();
    assert!(roles.topology_observer.update(&mut host, revision, &update).is_err());
    assert!(host.storage_failure().is_some());
    assert_eq!(roles.topology_observer.recovered_topology_checkpoint(&host, &saved),
        Err(JournalError::Unavailable));
    assert!(host.history_anchor_after(successor.history_anchor()).is_err());
    assert_ne!(fs::read(root.0.join("store/delivery.bin")).unwrap(), canonical,
        "the failed acknowledgment may have already exposed its replacement");
}
