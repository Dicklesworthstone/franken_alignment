//! Original ordinary recovery seals repeatable source custody without live reads.
use super::*;
use crate::action::consequence::delivery::persistent::observed::guarded::mediated::FileMediatedRecovery;
use std::cell::Cell;

fn ready(root: &Directory, saved: &TopologyCheckpoint) -> FileMediatedRecovery {
    let expected = FileMediatedRequirements {
        oversight: FileRecoveryRequirements {
            guards: guards(),
            effective_policy: profile().delivery.policy,
            credential_epoch: None,
            minimum: saved.minimum(),
        },
        topology: FileTopologyRequirement {
            initial: load(document(1).as_bytes(), INVENTORY)
                .unwrap()
                .graph()
                .clone(),
            current: saved.expected().graph().clone(),
            available: saved.available(),
        },
        prediction: None,
        evaluation: None,
    };
    let mut run = FileOversight::begin_open_mediated_guarded_anchored(
        root.0.join("store"),
        profile(),
        &expected,
        saved.history_anchor(),
    )
    .unwrap()
    .require_recovery_reserve(RecoveryReserve::terminal())
    .unwrap();
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
    assert!(
        !after.available(),
        "only the original Fence withdraws availability"
    );
    let bytes = archive(after);
    assert_eq!(
        &TopologyCheckpoint::read_trusted(&mut Cursor::new(&bytes), bytes.len()).unwrap(),
        after
    );
}

#[test]
fn reconciled_checkpoint_preserves_zero_and_spent_work_and_repeats_without_source_access() {
    for checks in [0, 1] {
        let (root, mut host, mut source, document) = fixture();
        for _ in 0..checks {
            refresh(&mut source, &mut host);
        }
        let saved = source
            .checkpoint(&host, document.as_bytes(), INVENTORY)
            .unwrap();
        drop(source);
        drop(host);
        fs::remove_file(root.0.join("topology.json")).unwrap();
        fs::remove_file(root.0.join("inventory.json")).unwrap();
        let clocks = Cell::new(0);
        let (recovered, successor) = ready(&root, &saved)
            .finish_reconciled_with_checkpoint(&saved, || {
                clocks.set(clocks.get() + 1);
                ElapsedTick(2)
            })
            .unwrap();
        assert_eq!(clocks.get(), 1);
        same_source(&saved, &successor);
        assert_eq!(successor.work().checks, checks);
        assert_eq!(
            successor.history_anchor(),
            &recovered.owner.history_anchor().unwrap()
        );
        assert_eq!(
            successor.history_anchor().revision(),
            saved.history_anchor().revision() + 3
        );
        let snapshot = recovered.owner.inspect();
        assert_eq!(
            successor.minimum().control_sequence,
            snapshot.control.sequence
        );
        assert_eq!(
            successor.minimum().authority_epoch,
            snapshot.control.ledger.epoch
        );
        assert_eq!(snapshot.executions, 0);
        assert!(recovered.outcomes.is_empty());
        assert!(!recovered.owner.mediation_snapshot().unwrap().available);
        assert!(
            recovered
                .owner
                .mediation_snapshot()
                .unwrap()
                .accepted
                .is_none()
        );
        drop(recovered);
        let (repeated, next) = ready(&root, &successor)
            .finish_reconciled_with_checkpoint(&successor, || ElapsedTick(3))
            .unwrap();
        same_source(&successor, &next);
        assert_eq!(
            next.history_anchor().revision(),
            successor.history_anchor().revision() + 3
        );
        assert_eq!(
            next.history_anchor(),
            &repeated.owner.history_anchor().unwrap()
        );
    }
}

#[test]
fn reconciled_checkpoint_refuses_stale_ordinary_or_source_cuts_before_clock_and_cleanup() {
    for source_work in [false, true] {
        let (root, mut host, mut source, document) = fixture();
        let old = source
            .checkpoint(&host, document.as_bytes(), INVENTORY)
            .unwrap();
        if source_work {
            refresh(&mut source, &mut host);
        } else {
            host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        }
        let latest = source
            .checkpoint(&host, document.as_bytes(), INVENTORY)
            .unwrap();
        drop(source);
        drop(host);
        fs::write(root.0.join("store/delivery.pending"), b"keep staging").unwrap();
        let before = fs::read(root.0.join("store/delivery.bin")).unwrap();
        assert!(matches!(
            ready(&root, &latest).finish_reconciled_with_checkpoint(&old, || panic!(
                "stale source custody cannot reach the clock"
            )),
            Err(JournalError::Contract(Error::Stale))
        ));
        assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), before);
        assert_eq!(
            fs::read(root.0.join("store/delivery.pending")).unwrap(),
            b"keep staging"
        );
        let (_, successor) = ready(&root, &latest)
            .finish_reconciled_with_checkpoint(&latest, || ElapsedTick(3))
            .unwrap();
        same_source(&latest, &successor);
        assert_eq!(successor.work().checks, u64::from(source_work));
    }
}

#[test]
fn reconciled_checkpoint_binds_owner_graph_availability_and_native_floors_before_mutation() {
    let (root, host, source, document) = fixture();
    let saved = source
        .checkpoint(&host, document.as_bytes(), INVENTORY)
        .unwrap();
    let (_other_root, other, other_source, other_document) = fixture();
    let foreign = other_source
        .checkpoint(&other, other_document.as_bytes(), INVENTORY)
        .unwrap();
    drop(source);
    drop(host);
    drop(other_source);
    drop(other);
    let before = fs::read(root.0.join("store/delivery.bin")).unwrap();
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
        assert!(
            ready(&root, &saved)
                .finish_reconciled_with_checkpoint(&bad, || panic!(
                    "mismatching source custody cannot reach the clock"
                ))
                .is_err(),
            "{mismatch}"
        );
        assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), before);
    }
    let (_, successor) = ready(&root, &saved)
        .finish_reconciled_with_checkpoint(&saved, || ElapsedTick(2))
        .unwrap();
    same_source(&saved, &successor);
}

#[test]
fn reconciled_checkpoint_clock_refusal_and_canonical_change_return_no_acknowledgment() {
    let (root, host, source, document) = fixture();
    let saved = source
        .checkpoint(&host, document.as_bytes(), INVENTORY)
        .unwrap();
    drop(source);
    drop(host);
    fs::write(root.0.join("store/delivery.pending"), b"keep staging").unwrap();
    let before = fs::read(root.0.join("store/delivery.bin")).unwrap();
    assert!(matches!(
        ready(&root, &saved).finish_reconciled_with_checkpoint(&saved, || ElapsedTick(0)),
        Err(JournalError::Contract(Error::Stale))
    ));
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ready(&root, &saved)
                .finish_reconciled_with_checkpoint(&saved, || panic!("trusted clock unavailable"))
        }))
        .is_err()
    );
    assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), before);
    assert_eq!(
        fs::read(root.0.join("store/delivery.pending")).unwrap(),
        b"keep staging"
    );
    assert!(matches!(
        ready(&root, &saved).finish_reconciled_with_checkpoint(&saved, || {
            fs::write(root.0.join("store/delivery.bin"), b"competing image").unwrap();
            ElapsedTick(2)
        }),
        Err(JournalError::Contract(Error::Binding))
    ));
    assert_eq!(
        fs::read(root.0.join("store/delivery.bin")).unwrap(),
        b"competing image"
    );
    assert_eq!(
        fs::read(root.0.join("store/delivery.pending")).unwrap(),
        b"keep staging"
    );
}
