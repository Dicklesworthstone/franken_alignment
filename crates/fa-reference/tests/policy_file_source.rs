#![cfg(unix)]

#[allow(dead_code)]
#[path = "support/supervised_driver.rs"]
mod fixture;

use fixture::{Rig, proposal};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use fa_reference::action::consequence::oversight::evidence_source::*;
use fa_reference::action::consequence::oversight::helper_client::ClientProgress;
use fa_reference::action::consequence::oversight::policy_state::{StateLimits, MAX_STATE_VALUE_BYTES};
use fa_reference::action::consequence::oversight::supervised::{DriverEvent, FileReviewLaunch};
use fa_reference::action::consequence::oversight::DispatchKeys;
use fa_reference::action::{ElapsedTick, Purpose, Scope};
use fa_reference::round::Verdict;
use fa_reference::{Error, Snapshot};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("fa-policy-file-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn path(&self) -> PathBuf { self.0.join("state.json") }
    fn publish(&self, generation: u64, snapshot: Snapshot) {
        let evidence = EvidenceSnapshot::new(EvidenceIdentity { source: 9, generation, scope: scope() }, snapshot,
            BTreeMap::from([("alice".to_owned(), b"alice-context".to_vec()), ("bob".to_owned(), b"bob-context".to_vec())])).unwrap();
        let pending = self.0.join("next.json");
        fs::write(&pending, evidence.encode()).unwrap(); fs::rename(pending, self.path()).unwrap();
    }
    fn attach(&self, rig: &mut Rig, limits: StateLimits) -> PolicyFileSource {
        PolicyFileSource::attach(FileEvidenceSource::new(self.path(), 9, scope(), MAX_EVIDENCE_FILE_BYTES).unwrap(),
            rig.driver.supervisor_mut().broker_mut(), 1, limits).unwrap()
    }
}
impl Drop for Directory { fn drop(&mut self) { fs::remove_dir_all(&self.0).unwrap(); } }
fn scope() -> Scope { Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect } }
fn accept(rig: &mut Rig, source: &mut PolicyFileSource, request: u64) {
    rig.port.submit(request, &proposal()).unwrap();
    let data = source.read().unwrap();
    let intake = rig.driver.accept_next(data.snapshot()).unwrap().unwrap();
    assert_eq!(intake.request, request); assert!(intake.result.unwrap().is_some());
}
fn review(rig: &mut Rig, source: &mut PolicyFileSource, request: u64, round: u64) {
    let launch = rig.launch(request, round);
    let data = source.read().unwrap();
    rig.inputs = Some(data.inputs_for(rig.driver.supervisor().action(request).unwrap(), rig.driver.supervisor().broker().contracts()).unwrap());
    rig.driver.start_file_review(source, FileReviewLaunch {
        request, round, window: launch.window, expected_input_revision: launch.expected_input_revision,
        workers: launch.streams, limits: launch.limits,
    }).unwrap();
    for _ in 0..128 {
        for (member, client) in &mut rig.clients {
            if client.step().unwrap() == ClientProgress::NeedsInference {
                assert_eq!(client.input().unwrap().actual_input(), rig.inputs.as_ref().unwrap().views()[member].actual_input());
                client.respond(Verdict::Allow, member.as_bytes()).unwrap();
            }
        }
        match rig.driver.step_from_file(source, || ElapsedTick(1), None).result.unwrap() {
            DriverEvent::Workers { .. } => {},
            DriverEvent::ReviewApplied { .. } => return,
            other => panic!("unexpected {other:?}"),
        }
    }
    panic!("bounded helper exchange did not finish");
}
fn ready(rig: &mut Rig, source: &mut PolicyFileSource) { accept(rig, source, 1); review(rig, source, 1, 1); }
fn conserved(rig: &Rig) {
    let state = rig.driver.supervisor().broker().inspect().ledger;
    assert_eq!(state.available + state.reserved + state.charged, 100);
}

#[test]
fn file_adapter_publishes_a_real_closed_source_consumed_by_dispatch() {
    let directory = Directory::new(); directory.publish(1, fixture::snapshot());
    let (mut rig, _) = Rig::new(false); let mut source = directory.attach(&mut rig, StateLimits::default());
    assert_eq!(rig.driver.supervisor().broker().capture_policy_state(), Err(Error::Incomplete));
    let data = source.read().unwrap();
    let cut = rig.driver.supervisor().broker().capture_policy_state().unwrap();
    assert_eq!(cut.snapshot(), data.snapshot()); assert_eq!(cut.frontier().through, 1);
    ready(&mut rig, &mut source);
    assert!(matches!(rig.driver.step_from_file(&mut source, || ElapsedTick(1), None).result.unwrap(), DriverEvent::PublicationResolved { .. }));
    assert_eq!(rig.driver.endpoint().payload(), b"publish");
    let actual = rig.driver.supervisor().broker().delivery_policy_state(1).unwrap().unwrap();
    assert_eq!(actual, &cut);
    assert_eq!(rig.driver.supervisor().broker().policy_state_status().unwrap().retained_events, 1);
    conserved(&rig);
}

#[test]
fn a_failed_file_read_closes_the_direct_dispatch_path_not_only_the_driver_method() {
    for outage in [false, true] {
        let directory = Directory::new(); directory.publish(1, fixture::snapshot());
        let (mut rig, _) = Rig::new(false); let mut source = directory.attach(&mut rig, StateLimits::default()); ready(&mut rig, &mut source);
        let permit = rig.driver.supervisor_mut().authorize_request(1, rig.inputs.as_ref(), &fixture::snapshot()).unwrap();
        if outage { fs::remove_file(directory.path()).unwrap(); assert!(source.read().is_err()); }
        let dispatched = rig.driver.supervisor_mut().dispatch_request(1, DispatchKeys::single(&permit), rig.inputs.as_ref(), &fixture::snapshot());
        if outage {
            assert!(dispatched.is_err());
            assert_eq!(rig.driver.supervisor().broker().inspect().ledger.reserved, 16);
            assert_eq!(rig.driver.endpoint().execution_count(), 0);
            assert!(rig.driver.supervisor().broker().capture_policy_state().is_err());
            rig.driver.cancel_active().unwrap();
        } else {
            let receipt = rig.driver.endpoint_mut().deliver(&dispatched.unwrap()).unwrap();
            rig.driver.accept_receipt(receipt).unwrap();
            assert_eq!(rig.driver.endpoint().execution_count(), 1);
        }
        conserved(&rig);
    }
}

#[test]
fn restoration_after_an_observed_outage_needs_new_source_image_and_new_review() {
    let directory = Directory::new(); directory.publish(1, fixture::snapshot());
    let (mut rig, _) = Rig::new(false); let mut source = directory.attach(&mut rig, StateLimits::default()); ready(&mut rig, &mut source);
    let before = source.status().closed.unwrap();
    fs::remove_file(directory.path()).unwrap();
    assert!(rig.driver.step_from_file(&mut source, || ElapsedTick(1), None).result.is_err());
    directory.publish(1, fixture::snapshot());
    source.read().unwrap();
    let after = source.status().closed.unwrap();
    assert!(after.through > before.through && after.marker_generation > before.marker_generation);
    assert!(rig.driver.step_from_file(&mut source, || ElapsedTick(1), None).result.is_err());
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
    rig.driver.cancel_active().unwrap();
    accept(&mut rig, &mut source, 2); review(&mut rig, &mut source, 2, 2);
    assert!(matches!(rig.driver.step_from_file(&mut source, || ElapsedTick(1), None).result.unwrap(), DriverEvent::PublicationResolved { .. }));
    assert_eq!(source.status().recorded_through, 2); conserved(&rig);
}

#[test]
fn dropping_the_adapter_closes_its_writer_but_does_not_block_receipt_reconciliation() {
    let directory = Directory::new(); directory.publish(1, fixture::snapshot());
    let (mut rig, _) = Rig::new(false); let mut source = directory.attach(&mut rig, StateLimits::default()); ready(&mut rig, &mut source);
    let ticket = rig.port.submit(1, &proposal()).unwrap();
    let permit = rig.driver.supervisor_mut().authorize_request(1, rig.inputs.as_ref(), &fixture::snapshot()).unwrap();
    let message = rig.driver.supervisor_mut().dispatch_request(1, DispatchKeys::single(&permit), rig.inputs.as_ref(), &fixture::snapshot()).unwrap();
    let receipt = rig.driver.endpoint_mut().deliver(&message).unwrap();
    rig.driver.supervisor_mut().acknowledgment_lost(1).unwrap();
    drop(source);
    assert!(!rig.driver.supervisor().broker().policy_state_status().unwrap().writer_live);
    assert!(rig.driver.supervisor().broker().capture_policy_state().is_err());
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.charged, 16);
    rig.driver.accept_receipt(receipt).unwrap();
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert_eq!(rig.driver.endpoint().execution_count(), 1); conserved(&rig);
}

#[test]
fn unchanged_reads_do_not_spend_event_quota_but_a_new_version_at_capacity_closes_it() {
    let directory = Directory::new(); directory.publish(1, fixture::snapshot());
    let (mut rig, _) = Rig::new(false);
    let mut source = directory.attach(&mut rig, StateLimits { events: 2, retained_bytes: 1_024 });
    for _ in 0..16 { source.read().unwrap(); }
    assert_eq!(source.status().recorded_through, 1);
    directory.publish(2, fixture::snapshot()); source.read().unwrap();
    assert_eq!(source.status().recorded_through, 2);
    directory.publish(3, fixture::snapshot());
    assert_eq!(source.read().unwrap_err(), EvidenceError::Data(Error::Limit));
    assert_eq!(source.status().fault, Some(Error::Limit));
    assert!(rig.driver.supervisor().broker().capture_policy_state().is_err());
    directory.publish(4, fixture::snapshot());
    assert_eq!(source.read().unwrap_err(), EvidenceError::Data(Error::Limit));
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.available, 100);
}

#[test]
fn incomplete_file_and_oversized_source_values_never_become_complete_in_the_gate() {
    let directory = Directory::new();
    let mut incomplete = fixture::snapshot(); incomplete.complete = false;
    directory.publish(1, incomplete);
    let (mut rig, _) = Rig::new(false); let mut source = directory.attach(&mut rig, StateLimits::default());
    assert_eq!(source.read().unwrap_err(), EvidenceError::Data(Error::Incomplete));
    assert!(rig.driver.supervisor().broker().capture_policy_state().is_err());
    assert_eq!(rig.driver.supervisor().broker().policy_state_status().unwrap().retained_events, 0);
    let mut bounded = fixture::snapshot(); bounded.values.insert(9, vec![1; MAX_STATE_VALUE_BYTES]);
    directory.publish(2, bounded.clone()); source.read().unwrap();
    assert_eq!(rig.driver.supervisor().broker().capture_policy_state().unwrap().snapshot(), &bounded);
    bounded.values.get_mut(&9).unwrap().push(1); directory.publish(3, bounded);
    assert_eq!(source.read().unwrap_err(), EvidenceError::Data(Error::Limit));
    assert!(rig.driver.supervisor().broker().capture_policy_state().is_err());
}

#[test]
fn older_caller_supplied_snapshot_cannot_override_fresh_captured_file_values() {
    let directory = Directory::new(); directory.publish(1, fixture::snapshot());
    let (mut rig, _) = Rig::new(false); let mut source = directory.attach(&mut rig, StateLimits::default()); ready(&mut rig, &mut source);
    let mut changed = fixture::snapshot(); changed.values.insert(7, vec![0]); directory.publish(2, changed);
    let captured = source.read().unwrap();
    assert!(rig.driver.supervisor_mut().authorize_request(1, rig.inputs.as_ref(), &fixture::snapshot()).is_err());
    assert!(rig.driver.supervisor_mut().authorize_request(1, rig.inputs.as_ref(), captured.snapshot()).is_err());
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.reserved, 0);
    assert_eq!(rig.driver.endpoint().execution_count(), 0); conserved(&rig);
}

#[test]
fn wrong_scope_bootstrap_does_not_install_a_gate_or_consume_a_valid_source() {
    let directory = Directory::new(); directory.publish(1, fixture::snapshot());
    let (mut rig, _) = Rig::new(false);
    let mut foreign = scope(); foreign.run += 1;
    let file = FileEvidenceSource::new(directory.path(), 9, foreign, MAX_EVIDENCE_FILE_BYTES).unwrap();
    assert_eq!(PolicyFileSource::attach(file, rig.driver.supervisor_mut().broker_mut(), 1, StateLimits::default()).unwrap_err(), Error::Binding);
    assert!(rig.driver.supervisor().broker().policy_state_status().is_none());
    let mut source = directory.attach(&mut rig, StateLimits::default()); source.read().unwrap();
    assert_eq!(rig.driver.supervisor().broker().capture_policy_state().unwrap().snapshot(), &fixture::snapshot());
}
