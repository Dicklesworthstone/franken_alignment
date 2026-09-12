//! Real endpoint files; no passing-run or power-loss guarantee is implied.
#![cfg(unix)]

#[path = "support/actor_gateway.rs"]
#[allow(dead_code)]
mod support;

use fa_reference::action::consequence::delivery::{PublicationEndpoint, StopRequest};
use fa_reference::action::consequence::delivery::filesystem::FilePublicationLimits;
use fa_reference::action::consequence::oversight::DispatchKeys;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, IntakeLimits, Knowledge};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::Error;
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use support::{attach, proposal, review, snapshot};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let tick = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-stop-{}-{tick}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("stop test cleanup failed: {error}"); }
    }
}

#[test]
fn stop_fence_and_nonexecution_seal_survive_endpoint_reopen_without_a_second_refund() {
    let root = Temp::new();
    let (mut endpoint, key) = PublicationEndpoint::create_file_publication(
        root.0.join("publication"), proposal().target, b"old".to_vec(), 200, 16,
        FilePublicationLimits { mutations: 128, bytes: 1_048_576 },
    ).unwrap();
    let (port, mut supervisor) = attach(&mut endpoint, IntakeLimits::default());
    let ticket = port.submit(1, &proposal()).unwrap();
    supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
    let inputs = review(&mut supervisor, 1, 1);
    let permit = supervisor.authorize_request(1, Some(&inputs), &snapshot()).unwrap();
    let delayed = supervisor.dispatch_request(1, DispatchKeys::single(&permit), Some(&inputs), &snapshot()).unwrap();
    let before = supervisor.broker().inspect();
    let request = StopRequest { operation: 1, expected_control_sequence: before.sequence,
        expected_authority_epoch: before.ledger.epoch };
    let stop = supervisor.request_stop(request).unwrap();
    assert_eq!(stop.refunded_units(), 0);
    assert!(supervisor.progress_stop(&mut endpoint).unwrap().progress.drained());
    assert_eq!(supervisor.broker().inspect().ledger.available, 100);
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::ConfirmedNotExecuted, .. }));
    let revision = key.visible_revision();
    drop(endpoint);
    let mut endpoint = key.reopen().unwrap();
    endpoint.observe_time(ElapsedTick(1)).unwrap();
    assert_eq!(endpoint.deliver(&delayed), Err(Error::Stale));
    let repeated = supervisor.progress_stop(&mut endpoint).unwrap();
    assert!(repeated.progress.drained());
    assert!(repeated.outcomes.is_empty());
    assert_eq!(key.visible_revision(), revision);
    assert_eq!(supervisor.request_stop(request).unwrap(), stop);
    assert_eq!(supervisor.broker().inspect().ledger.available, 100);
    let visible = PublicationEndpoint::read_file_publication(key.directory()).unwrap();
    assert_eq!(visible.execution_count, 0);
    assert_eq!(visible.payload, b"old");
}

#[test]
fn failed_fence_write_keeps_the_stop_and_charge_until_recovered_endpoint_evidence() {
    for executed in [false, true] {
        let root = Temp::new();
        let (mut endpoint, key) = PublicationEndpoint::create_file_publication(
            root.0.join("publication"), proposal().target, b"old".to_vec(), 200, 16,
            FilePublicationLimits { mutations: 128, bytes: 1_048_576 },
        ).unwrap();
        let (port, mut supervisor) = attach(&mut endpoint, IntakeLimits::default());
        let ticket = port.submit(1, &proposal()).unwrap();
        supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
        let inputs = review(&mut supervisor, 1, 1);
        let permit = supervisor.authorize_request(1, Some(&inputs), &snapshot()).unwrap();
        let delayed = supervisor.dispatch_request(1, DispatchKeys::single(&permit), Some(&inputs), &snapshot()).unwrap();
        if executed { let _lost_ack = endpoint.deliver(&delayed).unwrap(); }
        let before = supervisor.broker().inspect();
        let request = StopRequest { operation: 2, expected_control_sequence: before.sequence,
            expected_authority_epoch: before.ledger.epoch };
        let receipt = supervisor.request_stop(request).unwrap();
        fs::write(key.directory().join("publication.pending"), b"occupied staging file").unwrap();
        assert_eq!(supervisor.progress_stop(&mut endpoint), Err(Error::Incomplete));
        let failed = supervisor.stop_progress().unwrap();
        assert!(!failed.endpoint_fenced);
        assert!(!failed.drained());
        assert_eq!(failed.charged_units, 16);
        assert_eq!(supervisor.broker().inspect().ledger.stages[&1], ActionState::Unknown);
        assert!(matches!(port.poll(&ticket), Knowledge::Unknown { .. }));
        assert_eq!(supervisor.request_stop(request).unwrap(), receipt);
        supervisor.broker_mut().inputs_unavailable(1, 1).unwrap();
        drop(endpoint);
        let mut endpoint = key.reopen().unwrap();
        endpoint.observe_time(ElapsedTick(1)).unwrap();
        let recovered = supervisor.progress_stop(&mut endpoint).unwrap();
        assert!(recovered.progress.drained());
        assert_eq!(recovered.progress.charged_units, if executed { 16 } else { 0 });
        assert_eq!(endpoint.execution_count(), if executed { 1 } else { 0 });
        assert_eq!(endpoint.deliver(&delayed), Err(Error::Stale));
        assert!(supervisor.progress_stop(&mut endpoint).unwrap().outcomes.is_empty());
        assert_eq!(supervisor.broker().inspect().ledger.available, if executed { 84 } else { 100 });
    }
}
