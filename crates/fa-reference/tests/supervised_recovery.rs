//! Recovery of the original controller across memory/file endpoint interruption.
//! File tests execute real filesystem operations when run; no run is claimed here.
#![cfg(unix)]
#[path = "support/supervised_driver.rs"]
mod support;

use support::{Rig, proposal, snapshot, target};
use fa_reference::action::consequence::delivery::{EndpointOutcome, EndpointStatus, PublicationEndpoint};
use fa_reference::action::consequence::delivery::filesystem::FilePublicationLimits;
use fa_reference::action::consequence::oversight::DispatchKeys;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use fa_reference::action::consequence::oversight::supervised::{DriverEvent, DriverPhase};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-driver-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("driver test cleanup: {error}"); }
    }
}

#[test]
fn a_reserved_original_permit_survives_reconnect_or_is_cancelled_while_offline() {
    for cancel in [false, true] {
        let (mut rig, _) = Rig::new(false);
        let ticket = rig.accept(42);
        rig.start(42, 11);
        rig.finish_review([Verdict::Allow; 2]);
        rig.driver.supervisor_mut().broker_mut().restart_dispatcher().unwrap();
        assert!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).is_err());
        assert_eq!(rig.driver.supervisor().broker().inspect().ledger.reserved, 16);
        let Rig { port, driver, inputs, clients } = rig;
        drop(clients);
        let (offline, endpoint) = driver.detach_endpoint();
        if cancel { port.cancel(&ticket).unwrap(); }
        let mut driver = offline.reconnect(endpoint, ElapsedTick(2)).unwrap();
        let result = driver.step(ElapsedTick(2), inputs.as_ref(), &snapshot(), None).unwrap();
        if cancel {
            assert!(matches!(result, DriverEvent::Stopped { state: ActionState::Cancelled, .. }));
            assert_eq!(driver.supervisor().broker().inspect().ledger.available, 100);
            assert_eq!(driver.endpoint().execution_count(), 0);
        } else {
            assert!(matches!(result, DriverEvent::PublicationResolved { .. }));
            let ledger = driver.supervisor().broker().inspect().ledger;
            assert_eq!((ledger.available, ledger.reserved, ledger.charged), (84, 0, 16));
            assert_eq!(driver.endpoint().execution_count(), 1);
        }
    }
}

#[test]
fn a_foreign_peer_or_backward_clock_returns_the_original_owners_for_retry() {
    let (rig, _) = Rig::new(false);
    let (other, _) = Rig::new(false);
    let (offline, endpoint) = rig.driver.detach_endpoint();
    let (_other_offline, foreign) = other.driver.detach_endpoint();
    let before = offline.supervisor().broker().inspect();
    let failure = match offline.reconnect(foreign, ElapsedTick(2)) {
        Err(failure) => failure,
        Ok(_) => panic!("foreign brand admitted"),
    };
    assert_eq!(failure.error, Error::Binding);
    assert_eq!(failure.offline.supervisor().broker().inspect(), before);
    assert_eq!(failure.endpoint.execution_count(), 0);
    let failure = match failure.offline.reconnect(endpoint, ElapsedTick(0)) {
        Err(failure) => failure,
        Ok(_) => panic!("clock rollback admitted"),
    };
    assert_eq!(failure.error, Error::Stale);
    assert_eq!(failure.offline.supervisor().broker().inspect(), before);
    let mut driver = failure.offline.reconnect(failure.endpoint, ElapsedTick(2)).unwrap();
    assert!(driver.reconcile_pending(ElapsedTick(2)).unwrap().is_empty());
    assert_eq!(driver.supervisor().broker().inspect().ledger.available, 100);
}

#[test]
fn file_publication_lost_ack_recovers_after_all_helper_clients_are_gone() {
    let temp = Temp::new();
    let (endpoint, key) = PublicationEndpoint::create_file_publication(
        temp.0.join("endpoint"), target(), b"old".to_vec(), 200, 16,
        FilePublicationLimits { mutations: 128, bytes: 1_048_576 },
    ).unwrap();
    let (mut rig, _) = Rig::with_endpoint(endpoint, false);
    let ticket = rig.accept(42);
    rig.start(42, 11);
    rig.finish_review([Verdict::Allow; 2]);
    // Use the trusted dispatch seam to interrupt between endpoint execution and
    // receipt acceptance; the uninterrupted driver normally does both together.
    let permit = rig.driver.supervisor_mut().authorize_request(42, rig.inputs.as_ref(), &snapshot()).unwrap();
    let message = rig.driver.supervisor_mut().dispatch_request(42, DispatchKeys::single(&permit), rig.inputs.as_ref(), &snapshot()).unwrap();
    let receipt = rig.driver.endpoint_mut().deliver(&message).unwrap();
    rig.driver.supervisor_mut().acknowledgment_lost(42).unwrap();
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Unknown { .. }));
    assert_eq!(PublicationEndpoint::read_file_publication(key.directory()).unwrap().payload, b"publish");
    let attempt = rig.driver.supervisor().attempt(42).unwrap();
    let revision = rig.driver.supervisor().broker().input_revision(attempt).unwrap();
    rig.driver.supervisor_mut().broker_mut().inputs_unavailable(attempt, revision).unwrap();
    let Rig { port, driver, clients, .. } = rig;
    drop(clients);
    let (offline, endpoint) = driver.detach_endpoint();
    drop(endpoint);
    let recovered = key.reopen().unwrap();
    let mut driver = offline.reconnect(recovered, ElapsedTick(2)).unwrap();
    let results = driver.reconcile_pending(ElapsedTick(2)).unwrap();
    assert_eq!(results[&attempt], Ok(EndpointStatus::Resolved(receipt.clone())));
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert!(!driver.accept_receipt(receipt).unwrap());
    assert!(driver.reconcile_pending(ElapsedTick(2)).unwrap().is_empty());
    assert_eq!(driver.endpoint_mut().deliver(&message).unwrap_err(), Error::Stale);
    assert!(matches!(driver.step(ElapsedTick(2), None, &snapshot(), None).unwrap(), DriverEvent::Stopped { state: ActionState::Confirmed, .. }));
    assert_eq!(driver.phase(), DriverPhase::Idle);
    let duplicate = port.submit(42, &proposal()).unwrap();
    assert_eq!(port.poll(&ticket), port.poll(&duplicate));
    assert!(driver.accept_next(&snapshot()).unwrap().is_none());
    assert_eq!(PublicationEndpoint::read_file_publication(key.directory()).unwrap().execution_count, 1);
    assert_eq!(driver.supervisor().broker().inspect().ledger.charged, 16);
}

#[test]
fn file_write_failure_is_unknown_until_recovered_endpoint_evidence_resolves_it() {
    let temp = Temp::new();
    let (endpoint, key) = PublicationEndpoint::create_file_publication(
        temp.0.join("endpoint"), target(), b"old".to_vec(), 200, 16,
        FilePublicationLimits { mutations: 128, bytes: 1_048_576 },
    ).unwrap();
    let (mut rig, _) = Rig::with_endpoint(endpoint, false);
    let ticket = rig.accept(42);
    rig.start(42, 11);
    rig.finish_review([Verdict::Allow; 2]);
    fs::write(key.directory().join("publication.pending"), b"orphaned staging file").unwrap();
    assert!(matches!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap(), DriverEvent::DeliveryUnknown { .. }));
    assert_eq!(rig.driver.phase(), DriverPhase::Idle);
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.charged, 16);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Unknown { .. }));
    rig.port.cancel(&ticket).unwrap();
    rig.driver.supervisor_mut().synchronize().unwrap();
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.charged, 16);
    let attempt = rig.driver.supervisor().attempt(42).unwrap();
    let Rig { port, driver, clients, .. } = rig;
    drop(clients);
    let (offline, poisoned) = driver.detach_endpoint();
    drop(poisoned);
    let recovered = key.reopen().unwrap();
    let mut driver = offline.reconnect(recovered, ElapsedTick(2)).unwrap();
    assert_eq!(driver.reconcile_pending(ElapsedTick(2)).unwrap()[&attempt], Ok(EndpointStatus::AwaitingResolution));
    assert_eq!(driver.supervisor().broker().inspect().ledger.charged, 16);
    let results = driver.reconcile_pending(ElapsedTick(100)).unwrap();
    let Ok(EndpointStatus::Resolved(receipt)) = &results[&attempt] else { panic!("{results:?}"); };
    assert!(matches!(receipt.outcome(), EndpointOutcome::NotExecuted { .. }));
    assert_eq!(driver.supervisor().broker().inspect().ledger.available, 100);
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::ConfirmedNotExecuted, .. }));
    assert!(driver.reconcile_pending(ElapsedTick(100)).unwrap().is_empty());
    let visible = PublicationEndpoint::read_file_publication(key.directory()).unwrap();
    assert_eq!(visible.payload, b"old");
    assert_eq!(visible.execution_count, 0);
}

#[test]
fn a_late_genuine_receipt_updates_the_actor_while_the_driver_is_offline() {
    let (mut rig, _) = Rig::new(false);
    let ticket = rig.accept(42);
    rig.start(42, 11);
    rig.finish_review([Verdict::Allow; 2]);
    let permit = rig.driver.supervisor_mut().authorize_request(42, rig.inputs.as_ref(), &snapshot()).unwrap();
    let message = rig.driver.supervisor_mut().dispatch_request(42, DispatchKeys::single(&permit), rig.inputs.as_ref(), &snapshot()).unwrap();
    let receipt = rig.driver.endpoint_mut().deliver(&message).unwrap();
    let Rig { port, driver, .. } = rig;
    let (mut offline, endpoint) = driver.detach_endpoint();
    assert!(offline.accept_receipt(receipt.clone()).unwrap());
    assert!(!offline.accept_receipt(receipt).unwrap());
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let mut driver = offline.reconnect(endpoint, ElapsedTick(2)).unwrap();
    assert!(driver.reconcile_pending(ElapsedTick(2)).unwrap().is_empty());
    assert_eq!(driver.endpoint().execution_count(), 1);
}
