#![cfg(unix)]

#[allow(dead_code)]
#[path = "support/supervised_driver.rs"]
mod fixture;

use fixture::{Rig, proposal};
use fa_reference::action::consequence::delivery::{EndpointOutcome, FilePublicationLimits, PublicationEndpoint};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorTicket, Knowledge};
use fa_reference::action::consequence::oversight::evidence_source::*;
use fa_reference::action::consequence::oversight::helper_client::ClientProgress;
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::action::consequence::oversight::supervised::{DriverError, DriverEvent, DriverPhase, FileReviewLaunch};
use fa_reference::action::{ActionState, ElapsedTick, Purpose, Scope};
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("fa-file-driver-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn file(&self) -> PathBuf { self.0.join("observations.json") }
    fn source(&self) -> FileEvidenceSource { FileEvidenceSource::new(self.file(), 9, scope(), MAX_EVIDENCE_FILE_BYTES).unwrap() }
    fn publish(&self, observed: &EvidenceSnapshot) {
        let staged = self.0.join("next.json"); fs::write(&staged, observed.encode()).unwrap(); fs::rename(staged, self.file()).unwrap();
    }
}
impl Drop for Directory { fn drop(&mut self) { fs::remove_dir_all(&self.0).unwrap(); } }
fn scope() -> Scope { Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect } }
fn evidence(generation: u64) -> EvidenceSnapshot {
    EvidenceSnapshot::new(EvidenceIdentity { source: 9, generation, scope: scope() }, fixture::snapshot(),
        BTreeMap::from([("alice".to_owned(), b"actual-alice-context".to_vec()), ("bob".to_owned(), b"actual-bob-context".to_vec())])).unwrap()
}
fn accept(rig: &mut Rig, source: &mut FileEvidenceSource, request: u64) -> ActorTicket {
    let ticket = rig.port.submit(request, &proposal()).unwrap();
    let observed = source.read().unwrap();
    let result = rig.driver.accept_next(observed.snapshot()).unwrap().unwrap();
    assert_eq!(result.request, request); assert!(result.result.unwrap().is_some()); ticket
}
fn start(rig: &mut Rig, source: &mut FileEvidenceSource, request: u64, round: u64) {
    let launch = rig.launch(request, round);
    let data = source.read().unwrap();
    rig.inputs = Some(data.inputs_for(rig.driver.supervisor().action(request).unwrap(), rig.driver.supervisor().broker().contracts()).unwrap());
    rig.driver.start_file_review(source, FileReviewLaunch {
        request, round, window: launch.window, expected_input_revision: launch.expected_input_revision,
        workers: launch.streams, limits: launch.limits,
    }).unwrap();
}
fn clients(rig: &mut Rig, verdicts: [Verdict; 2]) {
    for (i, (member, client)) in rig.clients.iter_mut().enumerate() {
        if client.step().unwrap() == ClientProgress::NeedsInference {
            assert_eq!(client.input().unwrap().actual_input(), rig.inputs.as_ref().unwrap().views()[member].actual_input());
            client.respond(verdicts[i], member.as_bytes()).unwrap();
        }
    }
}
fn finish(rig: &mut Rig, source: &mut FileEvidenceSource, verdicts: [Verdict; 2]) -> DriverEvent {
    for _ in 0..128 {
        clients(rig, verdicts);
        let progress = rig.driver.step_from_file(source, || ElapsedTick(1), None);
        match progress.result.unwrap() {
            DriverEvent::Workers { .. } => {},
            event => return event,
        }
    }
    panic!("bounded real socket fixture did not close");
}
fn approved(rig: &mut Rig, source: &mut FileEvidenceSource) {
    accept(rig, source, 1); start(rig, source, 1, 1);
    assert!(matches!(finish(rig, source, [Verdict::Allow; 2]), DriverEvent::ReviewApplied { .. }));
}
fn conserved(rig: &Rig) {
    let ledger = rig.driver.supervisor().broker().inspect().ledger;
    assert_eq!(ledger.available + ledger.reserved + ledger.charged, 100);
}

#[test]
fn file_inputs_reach_real_helpers_and_both_final_reads_precede_publication() {
    let directory = Directory::new(); directory.publish(&evidence(1));
    let mut source = directory.source(); let (mut rig, _) = Rig::new(false);
    let ticket = accept(&mut rig, &mut source, 1); start(&mut rig, &mut source, 1, 1);
    assert!(matches!(finish(&mut rig, &mut source, [Verdict::Allow; 2]), DriverEvent::ReviewApplied { .. }));
    assert_eq!(rig.driver.endpoint().payload(), b"old");
    let result = rig.driver.step_from_file(&mut source, || ElapsedTick(1), None);
    assert_eq!(result.observations, vec![Ok(evidence(1).identity()); 2]);
    assert!(matches!(result.result.unwrap(), DriverEvent::PublicationResolved { receipt, .. }
        if matches!(receipt.outcome(), EndpointOutcome::Executed { .. })));
    assert_eq!(rig.driver.endpoint().payload(), b"publish");
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert_eq!(rig.driver.endpoint().execution_count(), 1); conserved(&rig);
}

#[test]
fn source_change_after_reservation_cannot_reuse_the_earlier_approval() {
    let directory = Directory::new(); directory.publish(&evidence(1));
    let mut source = directory.source(); let (mut rig, _) = Rig::new(false); approved(&mut rig, &mut source);
    let mut clock_calls = 0;
    let result = rig.driver.step_from_file(&mut source, || {
        clock_calls += 1;
        if clock_calls == 3 { directory.publish(&evidence(2)); }
        ElapsedTick(1)
    }, None);
    assert_eq!(result.observations, vec![Ok(evidence(1).identity()), Ok(evidence(2).identity())]);
    assert!(matches!(result.result, Err(DriverError::Control(Error::Stale))));
    let ledger = rig.driver.supervisor().broker().inspect().ledger;
    assert_eq!((ledger.available, ledger.reserved, ledger.charged), (84, 16, 0));
    assert_eq!(rig.driver.endpoint().payload(), b"old");
    assert!(rig.driver.step_from_file(&mut source, || ElapsedTick(1), None).result.is_err());
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.reserved, 16);
    rig.driver.cancel_active().unwrap();
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.available, 100);
    accept(&mut rig, &mut source, 2); start(&mut rig, &mut source, 2, 2);
    finish(&mut rig, &mut source, [Verdict::Allow; 2]);
    assert!(matches!(rig.driver.step_from_file(&mut source, || ElapsedTick(1), None).result.unwrap(), DriverEvent::PublicationResolved { .. }));
    conserved(&rig);
}

#[test]
fn failed_second_read_withdraws_approval_without_refunding_the_reservation() {
    let directory = Directory::new(); directory.publish(&evidence(1));
    let mut source = directory.source(); let (mut rig, _) = Rig::new(false); approved(&mut rig, &mut source);
    let attempt = rig.driver.supervisor().attempt(1).unwrap();
    let revision = rig.driver.supervisor().broker().input_revision(attempt).unwrap();
    let mut calls = 0;
    let result = rig.driver.step_from_file(&mut source, || {
        calls += 1; if calls == 3 { fs::remove_file(directory.file()).unwrap(); } ElapsedTick(1)
    }, None);
    assert!(matches!(result.observations.as_slice(), [Ok(_), Err(EvidenceError::Io(_))]));
    assert!(result.result.is_err());
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.reserved, 16);
    assert!(rig.driver.supervisor().broker().input_revision(attempt).unwrap() > revision);
    directory.publish(&evidence(1));
    assert!(rig.driver.step_from_file(&mut source, || ElapsedTick(1), None).result.is_err());
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
    rig.driver.cancel_active().unwrap(); conserved(&rig);
}

#[test]
fn changed_context_during_helper_io_requires_an_explicit_new_round() {
    let directory = Directory::new(); directory.publish(&evidence(1));
    let mut source = directory.source(); let (mut rig, _) = Rig::new(false);
    accept(&mut rig, &mut source, 1); start(&mut rig, &mut source, 1, 1);
    directory.publish(&evidence(2));
    assert!(matches!(finish(&mut rig, &mut source, [Verdict::Allow; 2]), DriverEvent::ReviewRejected { error: Error::Stale, .. }));
    assert_eq!(rig.driver.phase(), DriverPhase::Idle);
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.available, 100);
    start(&mut rig, &mut source, 1, 2);
    assert!(matches!(finish(&mut rig, &mut source, [Verdict::Allow; 2]), DriverEvent::ReviewApplied { .. }));
    assert!(matches!(rig.driver.step_from_file(&mut source, || ElapsedTick(1), None).result.unwrap(), DriverEvent::PublicationResolved { .. }));
    conserved(&rig);
}

#[test]
fn restrictive_review_survives_source_outage_but_permitting_review_does_not() {
    for verdicts in [[Verdict::Hold, Verdict::Allow], [Verdict::Allow; 2]] {
        let directory = Directory::new(); directory.publish(&evidence(1));
        let mut source = directory.source(); let (mut rig, _) = Rig::new(false);
        accept(&mut rig, &mut source, 1); start(&mut rig, &mut source, 1, 1);
        fs::remove_file(directory.file()).unwrap();
        match finish(&mut rig, &mut source, verdicts) {
            DriverEvent::ReviewApplied { receipt, .. } => assert_eq!(receipt.policy.control.decision.consequence,
                fa_reference::action::consequence::Consequence::HoldEffect),
            DriverEvent::ReviewRejected { .. } => assert_eq!(verdicts, [Verdict::Allow; 2]),
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(rig.driver.endpoint().execution_count(), 0);
        assert_eq!(rig.driver.supervisor().broker().inspect().ledger.available, 100);
    }
}

#[test]
fn a_valid_incomplete_document_does_not_preserve_a_previous_approval() {
    let directory = Directory::new(); directory.publish(&evidence(1));
    let mut source = directory.source(); let (mut rig, _) = Rig::new(false); approved(&mut rig, &mut source);
    let before = rig.driver.supervisor().broker().input_revision(1).unwrap();
    let original = evidence(2); let mut state = original.snapshot().clone(); state.complete = false;
    let incomplete = EvidenceSnapshot::new(original.identity(), state, original.contexts().clone()).unwrap();
    directory.publish(&incomplete);
    let result = rig.driver.step_from_file(&mut source, || ElapsedTick(1), None);
    assert!(matches!(result.result, Err(DriverError::Control(Error::Incomplete))));
    assert!(rig.driver.supervisor().broker().input_revision(1).unwrap() > before);
    assert_eq!(rig.driver.endpoint().execution_count(), 0); conserved(&rig);
}

#[test]
fn two_key_publication_still_needs_both_keys_and_new_source_data_stales_the_human_key() {
    for change in [false, true] {
        let directory = Directory::new(); directory.publish(&evidence(1));
        let mut source = directory.source(); let (mut rig, reviewer) = Rig::new(true); approved(&mut rig, &mut source);
        assert!(matches!(rig.driver.step_from_file(&mut source, || ElapsedTick(1), None).result.unwrap(), DriverEvent::AwaitingHuman { .. }));
        assert_eq!(rig.driver.supervisor().broker().inspect().ledger.reserved, 0);
        let request = rig.driver.request_human_approval(90, rig.inputs.as_ref(), ElapsedTick(20)).unwrap();
        let key = reviewer.unwrap().approve(&request, ElapsedTick(1)).unwrap();
        if change { directory.publish(&evidence(2)); }
        let result = rig.driver.step_from_file(&mut source, || ElapsedTick(1), Some(&key));
        if change {
            assert!(result.result.is_err()); assert_eq!(rig.driver.endpoint().execution_count(), 0);
            assert_eq!(rig.driver.supervisor().broker().human_status(90).unwrap().disposition, HumanDisposition::Approved);
        } else {
            assert!(matches!(result.result.unwrap(), DriverEvent::PublicationResolved { .. }));
            assert_eq!(rig.driver.supervisor().broker().human_status(90).unwrap().disposition, HumanDisposition::Consumed);
        }
        conserved(&rig);
    }
}

#[test]
fn a_slow_final_source_read_cannot_hide_action_expiry() {
    let directory = Directory::new(); directory.publish(&evidence(1));
    let mut source = directory.source(); let (mut rig, _) = Rig::new(false); approved(&mut rig, &mut source);
    let mut calls = 0;
    let result = rig.driver.step_from_file(&mut source, || { calls += 1; ElapsedTick(if calls == 4 { 100 } else { 1 }) }, None);
    assert!(matches!(result.result, Err(DriverError::Control(Error::Stale))));
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.reserved, 16);
    assert_eq!(rig.driver.endpoint().execution_count(), 0); conserved(&rig);
}

#[test]
fn file_setup_rejects_missing_context_without_sending_a_helper_packet() {
    let directory = Directory::new(); directory.publish(&evidence(1));
    let mut source = directory.source(); let (mut rig, _) = Rig::new(false); accept(&mut rig, &mut source, 1);
    let launch = rig.launch(1, 1);
    let data = evidence(2); let mut contexts = data.contexts().clone(); contexts.remove("bob");
    directory.publish(&EvidenceSnapshot::new(data.identity(), data.snapshot().clone(), contexts).unwrap());
    assert!(rig.driver.start_file_review(&mut source, FileReviewLaunch {
        request: 1, round: 1, window: launch.window, expected_input_revision: launch.expected_input_revision,
        workers: launch.streams, limits: launch.limits,
    }).is_err());
    assert_eq!(rig.driver.phase(), DriverPhase::Idle);
    assert_eq!(rig.driver.supervisor().broker().captured_input_bytes(), 0);
    assert_eq!(rig.driver.endpoint().execution_count(), 0); conserved(&rig);
}

#[test]
fn endpoint_recovery_and_receipts_do_not_depend_on_the_observation_file() {
    let directory = Directory::new(); directory.publish(&evidence(1));
    let (endpoint, recovery) = PublicationEndpoint::create_file_publication(directory.0.join("publication"),
        fixture::target(), b"old".to_vec(), 200, 16, FilePublicationLimits { mutations: 128, bytes: 1_048_576 }).unwrap();
    let (mut rig, _) = Rig::with_endpoint(endpoint, false); let mut source = directory.source();
    approved(&mut rig, &mut source);
    // Fail a real storage stage rather than manufacturing a nonexecution receipt.
    fs::write(recovery.directory().join("publication.pending"), b"occupied").unwrap();
    assert!(matches!(rig.driver.step_from_file(&mut source, || ElapsedTick(1), None).result.unwrap(), DriverEvent::DeliveryUnknown { .. }));
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.charged, 16);
    fs::remove_file(directory.file()).unwrap(); rig.clients.clear(); rig.inputs = None;
    let (offline, old_endpoint) = rig.driver.detach_endpoint(); drop(old_endpoint);
    let endpoint = recovery.reopen().unwrap();
    rig.driver = offline.reconnect(endpoint, ElapsedTick(100)).unwrap();
    rig.driver.reconcile_pending(ElapsedTick(100)).unwrap();
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.stages[&1], ActionState::ConfirmedNotExecuted);
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.available, 100);
    assert_eq!(rig.driver.endpoint().execution_count(), 0); conserved(&rig);
}
