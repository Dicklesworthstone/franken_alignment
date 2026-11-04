//! Native guarded terminal receipts retain source accounting at their exact cut.
use super::*;
use crate::action::consequence::delivery::StopRequest;
use crate::action::consequence::delivery::persistent::observed::{BaseEvent, Event, journal};
use crate::action::consequence::delivery::persistent::observed::guarded::mediated::FileMediatedRecovery;

fn ready(root: &Directory, saved: &TopologyCheckpoint) -> FileMediatedRecovery {
    let expected = FileMediatedRequirements {
        oversight: FileRecoveryRequirements { guards: guards(),
            effective_policy: profile().delivery.policy, credential_epoch: None,
            minimum: saved.minimum() },
        topology: FileTopologyRequirement { initial: load(document(1).as_bytes(), INVENTORY).unwrap().graph().clone(),
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

fn active() -> (Directory, FileOversight, FileDriverTopology, TopologyCheckpoint) {
    let (root, mut host, mut source, document) = fixture();
    refresh(&mut source, &mut host);
    let saved = source.checkpoint(&host, document.as_bytes(), INVENTORY).unwrap();
    assert!(saved.available());
    assert_eq!(saved.work().checks, 1);
    (root, host, source, saved)
}

fn same_source(before: &TopologyCheckpoint, after: &TopologyCheckpoint) {
    assert_eq!(after.topology_bytes(), before.topology_bytes());
    assert_eq!(after.inventory_bytes(), before.inventory_bytes());
    assert_eq!(after.expected(), before.expected());
    assert_eq!(after.work(), before.work());
    assert_eq!(after.last_operation(), before.last_operation());
    let bytes = archive(after);
    assert_eq!(&TopologyCheckpoint::read_trusted(&mut Cursor::new(&bytes), bytes.len()).unwrap(), after);
}

#[test]
fn terminal_checkpoint_admission_and_idempotent_drain_use_exact_native_acknowledgment() {
    let (root, host, source, original) = active();
    let identity = fs::canonicalize(root.0.join("store")).unwrap();
    drop(source); drop(host);
    let run = ready(&root, &original);
    let request = run.stop_request(900).unwrap();
    let admitted = run.finish_admission_stop(request).unwrap();
    assert_eq!(admitted.appended_events(), 2);
    let stopped = original.after_admission_stop(&profile(), &identity, &admitted).unwrap();
    same_source(&original, &stopped);
    assert!(!stopped.available());
    assert_eq!(stopped.history_anchor(), admitted.history_anchor());
    assert_eq!(stopped.minimum().journal_revision, admitted.snapshot().revision);
    assert_eq!(stopped.minimum().control_sequence, admitted.snapshot().control.sequence);
    assert_eq!(stopped.minimum().authority_epoch, admitted.snapshot().control.ledger.epoch);
    let drained = ready(&root, &stopped).finish_stopped_drain(request,
        || panic!("no outstanding effects: exact drain retry needs no clock")).unwrap();
    let final_checkpoint = stopped.after_stopped(&profile(), &identity, &drained).unwrap();
    assert_eq!(final_checkpoint, stopped);
    assert_eq!(original.after_stopped(&profile(), &identity, &drained).unwrap(), stopped);
    let repeated = ready(&root, &stopped).finish_admission_stop(request).unwrap();
    assert_eq!(repeated.appended_events(), 0);
    assert_eq!(stopped.after_admission_stop(&profile(), &identity, &repeated).unwrap(), stopped);

    // Captured native evidence remains usable after live storage disappears.
    // A reopen or source read would fail instead of returning this exact cut.
    for name in ["topology.json", "inventory.json", "store/delivery.bin"] {
        fs::remove_file(root.0.join(name)).unwrap();
    }
    assert_eq!(original.after_admission_stop(&profile(), &identity, &admitted).unwrap(), stopped);
    assert_eq!(stopped.after_stopped(&profile(), &identity, &drained).unwrap(), stopped);
}

#[test]
fn terminal_checkpoint_combined_stop_preserves_original_work_and_updated_native_floors() {
    let (root, host, source, saved) = active();
    let identity = fs::canonicalize(root.0.join("store")).unwrap();
    drop(source); drop(host);
    let run = ready(&root, &saved);
    let request = run.stop_request(901).unwrap();
    let history = run.try_finish_stopped_with_anchor(request, || Ok(ElapsedTick(2))).unwrap();
    assert_eq!(history.history_anchor().revision(), saved.history_anchor().revision() + 3);
    let successor = saved.after_stopped(&profile(), &identity, &history).unwrap();
    same_source(&saved, &successor);
    assert!(!successor.available());
    assert_eq!(successor.history_anchor(), history.history_anchor());
    assert_eq!(successor.minimum().authority_epoch, history.stopped().snapshot().control.ledger.epoch);
    let before = fs::read(identity.join("delivery.bin")).unwrap();
    let retry = ready(&root, &successor).try_finish_stopped_with_anchor(request,
        || panic!("completed retry cannot consult time")).unwrap();
    assert_eq!(successor.after_stopped(&profile(), &identity, &retry).unwrap(), successor);
    assert_eq!(fs::read(identity.join("delivery.bin")).unwrap(), before);
}

#[test]
fn terminal_checkpoint_rejects_unretained_source_work_and_ordinary_successors() {
    for later_source in [false, true] {
        let (root, mut host, mut source, old) = active();
        if later_source { refresh(&mut source, &mut host); }
        else { host.observe_time(host.revision(), ElapsedTick(2)).unwrap(); }
        let latest = source.checkpoint(&host, document(1).as_bytes(), INVENTORY).unwrap();
        let identity = fs::canonicalize(root.0.join("store")).unwrap();
        drop(source); drop(host);
        let run = ready(&root, &latest);
        let request = run.stop_request(902).unwrap();
        let stopped = run.finish_admission_stop(request).unwrap();
        let bytes = fs::read(identity.join("delivery.bin")).unwrap();
        assert_eq!(old.after_admission_stop(&profile(), &identity, &stopped),
            Err(JournalError::Contract(Error::Stale)));
        let successor = latest.after_admission_stop(&profile(), &identity, &stopped).unwrap();
        same_source(&latest, &successor);
        assert_eq!(successor.work().checks, if later_source { 2 } else { 1 });
        assert_eq!(fs::read(identity.join("delivery.bin")).unwrap(), bytes);
    }
}

#[test]
fn terminal_checkpoint_binds_profile_identity_graph_availability_and_monotonic_floors() {
    let (root, host, source, saved) = active();
    let (foreign_root, foreign_host, foreign_source, foreign) = active();
    let identity = fs::canonicalize(root.0.join("store")).unwrap();
    drop(source); drop(host); drop(foreign_source); drop(foreign_host);
    let run = ready(&root, &saved);
    let request = run.stop_request(903).unwrap();
    let stopped = run.finish_admission_stop(request).unwrap();
    let before = fs::read(identity.join("delivery.bin")).unwrap();
    let mut wrong_profile = profile(); wrong_profile.delivery.total += 1;
    assert!(saved.after_admission_stop(&wrong_profile, &identity, &stopped).is_err());
    assert!(saved.after_admission_stop(&profile(), &foreign_root.0.join("store"), &stopped).is_err());
    assert!(foreign.after_admission_stop(&profile(), &identity, &stopped).is_err());
    for change in ["graph", "availability", "control", "epoch", "equal-counter-fork"] {
        let mut changed = saved.clone();
        match change {
            "graph" => changed.expected = load(document(2).as_bytes(), INVENTORY).unwrap(),
            "availability" => changed.available = false,
            "control" => changed.control_sequence = u64::MAX,
            "epoch" => changed.authority_epoch = u64::MAX,
            "equal-counter-fork" => {
                let mut events = changed.history.decoded_events(&profile(), &identity).unwrap();
                let event = events.iter_mut().find(|event| matches!(event,
                    Event::Core(BaseEvent::Time(_)))).expect("original time observation");
                let Event::Core(BaseEvent::Time(tick)) = event else { unreachable!() };
                *tick = ElapsedTick(2);
                changed.history = crate::action::consequence::delivery::persistent::observed::guarded::anchored::FileHistoryAnchor::from_acknowledged(
                    events.len(), journal::encode(&profile(), &identity, &events).unwrap());
                assert_eq!(changed.minimum(), saved.minimum(), "counters cannot distinguish this fork");
            }
            _ => unreachable!(),
        }
        assert!(changed.after_admission_stop(&profile(), &identity, &stopped).is_err(), "{change}");
    }
    assert!(saved.after_admission_stop(&profile(), &identity, &stopped).is_ok());
    assert_eq!(fs::read(identity.join("delivery.bin")).unwrap(), before);
}

#[test]
fn terminal_checkpoint_stop_only_preserves_historical_availability_until_native_fence() {
    let (root, mut host, source, saved) = active();
    let identity = fs::canonicalize(root.0.join("store")).unwrap();
    let state = host.inspect();
    let request = StopRequest { operation: 904, expected_control_sequence: state.control.sequence,
        expected_authority_epoch: state.control.ledger.epoch };
    host.request_stop(host.revision(), request).unwrap();
    assert!(host.mediation_snapshot().unwrap().available);
    drop(source); drop(host);
    let admitted = ready(&root, &saved).finish_admission_stop(request).unwrap();
    assert_eq!(admitted.appended_events(), 0);
    assert!(!admitted.progress().endpoint_fenced, "Stop invalidates the dispatcher fence even without effects");
    assert!(!admitted.progress().drained(), "acknowledging the current fence remains mandatory");
    let stopped = saved.after_admission_stop(&profile(), &identity, &admitted).unwrap();
    assert!(stopped.available(), "Stop alone is not native topology withdrawal");
    same_source(&saved, &stopped);
    let clocks = std::cell::Cell::new(0);
    let drained = ready(&root, &stopped).try_finish_stopped_with_anchor(request,
        || { clocks.set(clocks.get() + 1); Ok(ElapsedTick(2)) }).unwrap();
    assert_eq!(clocks.get(), 1);
    assert_eq!(drained.history_anchor().revision(), stopped.history_anchor().revision() + 3);
    assert!(drained.stopped().progress().drained());
    let fenced = stopped.after_stopped(&profile(), &identity, &drained).unwrap();
    assert!(!fenced.available(), "the actual recovery Fence withdraws the topology");
    same_source(&saved, &fenced);
}
