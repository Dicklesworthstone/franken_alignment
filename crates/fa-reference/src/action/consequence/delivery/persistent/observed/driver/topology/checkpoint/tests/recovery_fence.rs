//! Retained source expenditure survives only the exact native recovery fence.
use super::*;
use crate::action::consequence::delivery::persistent::observed::driver::topology::checkpoint::TopologyCheckpointResume;
use crate::action::consequence::delivery::persistent::observed::guarded::mediated::FileMediatedRecovery;
use crate::action::consequence::delivery::persistent::observed::{BaseEvent, Event, journal};
use crate::action::consequence::mediation::CutCheck;

fn active() -> (Directory, FileOversight, FileDriverTopology, TopologyCheckpoint) {
    let (root, mut host, mut source, text) = fixture();
    refresh(&mut source, &mut host);
    let saved = source.checkpoint(&host, text.as_bytes(), INVENTORY).unwrap();
    assert!(saved.available());
    assert_eq!(saved.work().checks, 1);
    (root, host, source, saved)
}

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
    let bytes = archive(after);
    assert_eq!(
        &TopologyCheckpoint::read_trusted(&mut Cursor::new(&bytes), bytes.len()).unwrap(),
        after
    );
}

#[test]
fn recovered_fence_checkpoint_allows_corrected_restart_after_source_rebind_refuses() {
    let (root, host, source, original) = active();
    drop(source);
    drop(host);
    let (host, roles) = ready(&root, &original).finish().unwrap();
    assert!(host.credential_status().unwrap().is_none());
    let fenced = original.after_recovery_fence(&host).unwrap();
    same_source(&original, &fenced);
    assert_eq!(fenced.history_anchor(), &host.history_anchor().unwrap());
    assert_eq!(fenced.history_anchor().revision(), original.history_anchor().revision() + 1);
    assert!(!fenced.available());
    assert!(host.mediation_snapshot().unwrap().accepted.is_none());
    assert_eq!(original.after_recovery_fence(&host).unwrap(), fenced);

    let (port, mut driver) = host.into_supervised_driver();
    drop(port);
    let before = fs::read(root.0.join("store/delivery.bin")).unwrap();
    let revision = driver.supervisor().host().unwrap().revision();
    // The requested replacement has not advanced its generation. It refuses
    // before source installation; the new Fence nevertheless remains real.
    assert!(driver.resume_topology_checkpoint(
        revision,
        original.last_operation() + 1,
        roles.topology_observer,
        &fenced,
        TopologyCheckpointResume {
            topology_path: root.0.join("topology.json"),
            inventory_path: root.0.join("inventory.json"),
            next: original.expected().clone(),
        },
    ).is_err());
    assert!(driver.topology().is_none());
    assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), before);
    drop(driver);

    // Retain/import the actual fence, then use the original guarded recovery
    // with unavailable topology. Old pre-fence availability is never relaxed.
    let retained = archive(&fenced);
    let fenced = TopologyCheckpoint::read_trusted(&mut Cursor::new(&retained), retained.len()).unwrap();
    let (mut host, roles) = ready(&root, &fenced).finish().unwrap();
    assert_eq!(original.after_recovery_fence(&host), Err(JournalError::Contract(Error::Stale)));
    let second = fenced.after_recovery_fence(&host).unwrap();
    same_source(&original, &second);
    assert_eq!(second.history_anchor().revision(), fenced.history_anchor().revision() + 1);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    fs::write(root.0.join("topology.json"), document(2)).unwrap();
    let (port, mut driver) = host.into_supervised_driver();
    drop(port);
    let revision = driver.supervisor().host().unwrap().revision();
    let report = driver.resume_topology_checkpoint(
        revision,
        second.last_operation() + 1,
        roles.topology_observer,
        &second,
        TopologyCheckpointResume {
            topology_path: root.0.join("topology.json"),
            inventory_path: root.0.join("inventory.json"),
            next: load(document(2).as_bytes(), INVENTORY).unwrap(),
        },
    ).unwrap();
    assert!(report.refusal.is_none());
    assert!(matches!(report.checked, Some(Ok(CutCheck::Verified(_)))));
    let source = driver.topology().unwrap().source();
    assert_eq!(source.work().checks, original.work().checks + 1);
    assert_eq!(source.last_operation(), original.last_operation() + 2);
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn recovered_fence_checkpoint_uses_acknowledged_memory_without_source_or_journal_reads() {
    let (root, host, source, original) = active();
    drop(source);
    drop(host);
    let (host, _roles) = ready(&root, &original).finish().unwrap();
    let snapshot = host.inspect();
    let history = host.history_anchor().unwrap();
    for name in ["topology.json", "inventory.json", "store/delivery.bin"] {
        fs::remove_file(root.0.join(name)).unwrap();
    }
    let after = original.after_recovery_fence(&host).unwrap();
    same_source(&original, &after);
    assert_eq!(after.history_anchor(), &history);
    assert_eq!(after.minimum().control_sequence, snapshot.control.sequence);
    assert_eq!(after.minimum().authority_epoch, snapshot.control.ledger.epoch);
    assert_eq!(host.inspect(), snapshot);
    assert!(!after.available());
}

#[test]
fn recovered_fence_checkpoint_refuses_stale_source_time_and_changed_topology_cuts() {
    for later in ["source", "time", "topology"] {
        let (root, mut host, mut source, old) = active();
        let generation = match later {
            "source" => {
                refresh(&mut source, &mut host);
                1
            }
            "time" => {
                host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
                1
            }
            "topology" => {
                let revision = host.revision();
                let operation = source.source().last_operation() + 1;
                source.source.rebind(&mut host, revision, operation,
                    load(document(2).as_bytes(), INVENTORY).unwrap()).unwrap();
                2
            }
            _ => unreachable!(),
        };
        let latest = source.checkpoint(&host, document(generation).as_bytes(), INVENTORY).unwrap();
        drop(source);
        drop(host);
        let (host, _roles) = ready(&root, &latest).finish().unwrap();
        let before = fs::read(root.0.join("store/delivery.bin")).unwrap();
        assert_eq!(old.after_recovery_fence(&host), Err(JournalError::Contract(Error::Stale)), "{later}");
        let current = latest.after_recovery_fence(&host).unwrap();
        same_source(&latest, &current);
        assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), before);
    }
}

#[test]
fn recovered_fence_checkpoint_requires_same_owner_complete_prefix_graph_and_floors() {
    let (root, host, source, saved) = active();
    let (_foreign_root, foreign, _foreign_source, foreign_saved) = active();
    assert!(saved.after_recovery_fence(&foreign).is_err());
    // A source capture on a live owner is not itself a recovery Fence.
    assert_eq!(saved.after_recovery_fence(&host), Err(JournalError::Contract(Error::Stale)));
    let identity = fs::canonicalize(root.0.join("store")).unwrap();
    drop(source);
    drop(host);
    let (host, _roles) = ready(&root, &saved).finish().unwrap();
    assert!(foreign_saved.after_recovery_fence(&host).is_err());
    let before = fs::read(identity.join("delivery.bin")).unwrap();
    for change in ["graph", "availability", "control", "epoch", "profile", "equal-counter-fork"] {
        let mut changed = saved.clone();
        match change {
            "graph" => changed.expected = load(document(2).as_bytes(), INVENTORY).unwrap(),
            "availability" => changed.available = false,
            "control" => changed.control_sequence = u64::MAX,
            "epoch" => changed.authority_epoch = u64::MAX,
            "profile" | "equal-counter-fork" => {
                let mut events = saved.history.decoded_events(&profile(), &identity).unwrap();
                let mut selected = profile();
                if change == "profile" {
                    selected.delivery.total += 1;
                } else {
                    let event = events.iter_mut().find(|event|
                        matches!(event, Event::Core(BaseEvent::Time(_)))).unwrap();
                    let Event::Core(BaseEvent::Time(tick)) = event else { unreachable!() };
                    *tick = ElapsedTick(2);
                }
                changed.history = FileHistoryAnchor::from_acknowledged(events.len(),
                    journal::encode(&selected, &identity, &events).unwrap());
                assert_eq!(changed.minimum(), saved.minimum());
            }
            _ => unreachable!(),
        }
        assert!(changed.after_recovery_fence(&host).is_err(), "{change}");
    }
    let successor = saved.after_recovery_fence(&host).unwrap();
    assert_eq!(successor.after_recovery_fence(&host), Err(JournalError::Contract(Error::Stale)));
    same_source(&saved, &successor);
    assert_eq!(fs::read(identity.join("delivery.bin")).unwrap(), before);
}

#[test]
fn recovered_fence_checkpoint_refuses_later_work_and_unacknowledged_storage_candidates() {
    for barrier in [None, Some(JournalIo::Stage), Some(JournalIo::Write),
        Some(JournalIo::FileSync), Some(JournalIo::Rename), Some(JournalIo::DirectorySync)] {
        let (root, host, source, saved) = active();
        drop(source);
        drop(host);
        let (mut host, _roles) = ready(&root, &saved).finish().unwrap();
        let acknowledged = saved.after_recovery_fence(&host).unwrap();
        let retained = archive(&acknowledged);
        if let Some(barrier) = barrier {
            host.store.fail_once(barrier);
        }
        let result = host.observe_time(host.revision(), ElapsedTick(2));
        let refused = saved.after_recovery_fence(&host);
        if barrier.is_some() {
            assert!(result.is_err());
            assert!(host.storage_failure().is_some());
            assert_eq!(refused, Err(JournalError::Unavailable));
        } else {
            result.unwrap();
            assert_eq!(refused, Err(JournalError::Contract(Error::Stale)));
        }
        let restored = TopologyCheckpoint::read_trusted(&mut Cursor::new(&retained), retained.len()).unwrap();
        assert_eq!(restored, acknowledged);
        same_source(&saved, &restored);
    }
}
