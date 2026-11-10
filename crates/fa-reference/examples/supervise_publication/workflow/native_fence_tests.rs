//! Original native Fence cleanup without an extra control socket or source read.
use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileFenceBasis, FileFenceError, FileFenceRequest, FileSupervisorFenceGrant,
    FileSupervisorFencePort, MAX_FENCE_REPLAY_EVENTS,
};
use std::cell::Cell;

fn ready(root: &Directory) -> (
    Config, FileHumanReviewer, FileSupervisedDriver,
    FileSupervisorFencePort, FileSupervisorFenceGrant, FileFenceRequest,
) {
    let config = configured(root, false);
    let (mut host, reviewer) = FileOversight::create(&config.store, config.profile.clone()).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    let (_actor, driver) = host.into_supervised_driver();
    let (port, grant) = driver.supervisor().fence_port(MAX_FENCE_REPLAY_EVENTS).unwrap();
    let state = driver.supervisor().host().unwrap().inspect();
    let request = FileFenceRequest::new(71, FileFenceBasis {
        scope: config.profile.delivery.scope,
        journal_revision: state.revision,
        control_sequence: state.control.sequence,
        authority_epoch: state.control.ledger.epoch,
    }).unwrap();
    (config, reviewer, driver, port, grant, request)
}

#[test]
fn refused_fence_does_not_close_control_or_sample_cleanup_time() {
    let root = Directory::new();
    let (config, reviewer, mut driver, port, grant, request) = ready(&root);
    let before = driver.supervisor().host().unwrap().inspect();
    let mut expected = request.expected();
    expected.journal_revision += 1;
    assert_eq!(port.fence(FileFenceRequest::new(request.operation(), expected).unwrap()),
        Err(FileFenceError::StalePredecessor));
    let mut control = Control::new(&config, 1, None).unwrap();
    assert!(!control.checkpoint(&mut driver, &reviewer, &deadline(),
        &mut || panic!("a refused Fence cannot authorize cleanup")).unwrap());
    assert!(!control.native_cleanup_started());
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(driver.supervisor().host().unwrap().inspect(), before);
}

#[test]
fn acknowledged_fence_drains_once_and_shared_grace_preserves_historical_receipt() {
    let root = Directory::new();
    let (config, reviewer, mut driver, port, _grant, request) = ready(&root);
    let native = port.fence(request).unwrap();
    let Knowledge::Known { value: receipt, .. } = &native else { panic!("native Stop"); };
    assert!(!receipt.progress().endpoint_fenced);
    let stopped_revision = receipt.after().journal_revision;
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let mut control = Control::new(&config, 1, None).unwrap();
    let mut shared = control.share();
    let clocks = Cell::new(0);
    assert!(control.checkpoint(&mut driver, &reviewer, &deadline(), &mut || {
        clocks.set(clocks.get() + 1);
        ElapsedTick(1000)
    }).unwrap());
    assert_eq!(clocks.get(), 1);
    let after = driver.supervisor().host().unwrap().inspect();
    assert_eq!(after.revision, stopped_revision + 1);
    assert_eq!(after.stop.as_ref().unwrap(), receipt.stop());
    assert!(driver.supervisor().host().unwrap().stop_progress().unwrap().drained());
    assert!(shared.checkpoint(&mut driver, &reviewer, &deadline(),
        &mut || panic!("shared terminal grace repeated native cleanup")).unwrap());
    assert_eq!(driver.supervisor().host().unwrap().inspect(), after);
    assert_eq!(port.fence(request).unwrap(), native);
    assert!(!receipt.progress().endpoint_fenced);
}

#[test]
fn terminal_or_failed_ingress_cannot_skip_cleanup_after_native_fence() {
    for fail_transport in [false, true] {
        let root = Directory::new();
        let (config, reviewer, mut driver, port, _grant, request) = ready(&root);
        let mut control = Control::new(&config, 1, None).unwrap();
        let mut ingress = |_: &mut FileSupervisedDriver| {
            assert!(matches!(port.fence(request), Ok(Knowledge::Known { .. })));
            if fail_transport { Err("transport failed after native Fence".into()) } else { Ok(true) }
        };
        let mut live = crate::workflow::LiveControl {
            stop: control.share(),
            ingress: &mut ingress,
        };
        let result = live.checkpoint(&mut driver, &reviewer, &deadline(), &mut || ElapsedTick(1000));
        if fail_transport {
            assert_eq!(result, Err("transport failed after native Fence".into()));
        } else {
            assert_eq!(result, Ok(true));
        }
        assert!(driver.supervisor().host().unwrap().stop_progress().unwrap().drained());
        assert!(control.native_cleanup_started());
        let after = driver.supervisor().host().unwrap().inspect();
        assert!(control.native_stop(&mut driver,
            &mut || panic!("ingress cleanup lost original shared custody")).unwrap());
        assert_eq!(driver.supervisor().host().unwrap().inspect(), after);
    }
}

#[test]
fn failed_or_unwinding_native_cleanup_cannot_retry_from_reply_grace() {
    for unwind in [false, true] {
        let root = Directory::new();
        let (config, _reviewer, mut driver, port, _grant, request) = ready(&root);
        assert!(matches!(port.fence(request), Ok(Knowledge::Known { .. })));
        let before = driver.supervisor().host().unwrap().inspect();
        let mut control = Control::new(&config, 1, None).unwrap();
        let mut shared = control.share();
        if unwind {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                control.native_stop(&mut driver, &mut || panic!("fallible trusted clock"))
            }));
            assert!(result.is_err());
        } else {
            assert!(control.native_stop(&mut driver, &mut || ElapsedTick(999)).is_err());
        }
        assert!(control.native_cleanup_started());
        let result = shared.native_stop(&mut driver,
            &mut || panic!("terminal reply must not retry an unacknowledged drain"));
        assert!(result.is_err());
        assert_eq!(driver.supervisor().host().unwrap().inspect(), before);
        assert!(!driver.supervisor().host().unwrap().stop_progress().unwrap().drained());
    }
}
