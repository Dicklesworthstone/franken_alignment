//! Numerical inference, socket helpers, original permits and endpoint recovery.
#![cfg(unix)]
#[path = "support/supervised_driver.rs"]
#[allow(dead_code)]
mod driver_support;
#[path = "support/decoder_control.rs"]
#[allow(dead_code)]
mod control;
use driver_support::{Rig, snapshot};
use control::numerical::compute;
use fa_reference::action::consequence::delivery::{PublicationEndpoint, EndpointStatus};
use fa_reference::action::consequence::delivery::filesystem::FilePublicationLimits;
use fa_reference::action::consequence::oversight::DispatchKeys;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use fa_reference::action::consequence::oversight::supervised::{DriverError, DriverEvent};
use fa_reference::action::ElapsedTick;
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

#[test]
fn actual_decoder_and_socket_congress_feed_both_original_driver_publication_modes() {
    for two_key in [false, true] {
        let (mut rig, reviewer) = Rig::new(two_key); let mut source = control::run();
        rig.driver.supervisor_mut().broker_mut().enable_decoder_monitoring(source.observation(), DecoderBindingLimits::default()).unwrap();
        source.advance(0, 1, compute()).unwrap();
        let ticket = rig.accept(1); rig.start(1, 1); rig.finish_review([Verdict::Allow; 2]);
        assert_eq!(rig.driver.endpoint().execution_count(), 0);
        let human = reviewer.as_ref().map(|reviewer| {
            assert!(matches!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap(), DriverEvent::AwaitingHuman { .. }));
            let request = rig.driver.request_human_approval(101, rig.inputs.as_ref(), ElapsedTick(40)).unwrap();
            reviewer.approve(&request, ElapsedTick(1)).unwrap()
        });
        assert!(matches!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), human.as_ref()).unwrap(), DriverEvent::PublicationResolved { .. }));
        assert_eq!(rig.driver.endpoint().execution_count(), 1);
        assert_eq!(rig.driver.endpoint().payload(), b"publish");
        assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
        assert_eq!(rig.driver.supervisor().broker().dispatched_decoder_evidence(1).unwrap().unwrap().tokens(), &[1]);
        assert_eq!(rig.driver.supervisor().broker().inspect().ledger.charged, 16);
    }
}

#[test]
fn decoder_loss_after_socket_review_blocks_driver_authorization_not_just_a_direct_api() {
    let (mut rig, _) = Rig::new(false); let mut source = control::run();
    rig.driver.supervisor_mut().broker_mut().enable_decoder_monitoring(source.observation(), DecoderBindingLimits::default()).unwrap();
    source.advance(0, 1, compute()).unwrap();
    let ticket = rig.accept(1); rig.start(1, 1); rig.finish_review([Verdict::Allow; 2]);
    drop(source);
    assert_eq!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap_err(), DriverError::Control(Error::Incomplete));
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.reserved, 0);
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.charged, 0);
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Pending { .. }));
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-decoder-admission-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap(); Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("decoder admission fixture cleanup: {error}"); }
    }
}

#[test]
fn file_publication_with_a_lost_ack_survives_decoder_loss_and_fenced_endpoint_reopen() {
    let directory = Directory::new();
    let (endpoint, recovery) = PublicationEndpoint::create_file_publication(directory.0.join("endpoint"),
        driver_support::target(), b"old".to_vec(), 200, 16,
        FilePublicationLimits { mutations: 128, bytes: 1_048_576 }).unwrap();
    let (mut rig, _) = Rig::with_endpoint(endpoint, false); let mut source = control::run();
    rig.driver.supervisor_mut().broker_mut().enable_decoder_monitoring(source.observation(), DecoderBindingLimits::default()).unwrap();
    source.advance(0, 1, compute()).unwrap();
    let ticket = rig.accept(1); rig.start(1, 1); rig.finish_review([Verdict::Allow; 2]);
    let permit = rig.driver.supervisor_mut().authorize_request(1, rig.inputs.as_ref(), &snapshot()).unwrap();
    let envelope = rig.driver.supervisor_mut().dispatch_request(1, DispatchKeys::single(&permit), rig.inputs.as_ref(), &snapshot()).unwrap();
    let _lost_ack = rig.driver.endpoint_mut().deliver(&envelope).unwrap();
    rig.driver.supervisor_mut().broker_mut().acknowledgment_lost(1).unwrap();
    rig.driver.supervisor_mut().synchronize().unwrap();
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Unknown { .. }));
    drop(source); rig.clients.clear(); rig.inputs = None;
    rig.driver.supervisor_mut().broker_mut().inputs_unavailable(1, 1).unwrap();
    let (offline, endpoint) = rig.driver.detach_endpoint(); drop(endpoint);
    let endpoint = recovery.reopen().unwrap();
    let mut driver = offline.reconnect(endpoint, ElapsedTick(1)).unwrap();
    let results = driver.reconcile_pending(ElapsedTick(1)).unwrap();
    assert!(matches!(results[&1], Ok(EndpointStatus::Resolved(_))));
    assert_eq!(driver.endpoint().execution_count(), 1);
    assert_eq!(driver.endpoint().payload(), b"publish");
    assert_eq!(driver.supervisor().broker().inspect().ledger.charged, 16);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let evidence = driver.supervisor().broker().dispatched_decoder_evidence(1).unwrap().unwrap();
    assert_eq!(evidence.tokens(), &[1]);
    let visible = PublicationEndpoint::read_file_publication(recovery.directory()).unwrap();
    assert_eq!(visible.execution_count, 1);
}
