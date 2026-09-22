//! Exercise the public driver over real request journals and helper sockets.
use super::*;
use crate::action::consequence::delivery::persistent::observed::driver::{
    FileDriverEvent, FileDriverLaunch, FileDriverPhase, FileSupervisedDriver,
};
use crate::action::consequence::delivery::persistent::requests::actor::FileActorPort;
use crate::action::consequence::oversight::helper_workers::HelperLimits;
use std::io::Read;
use std::os::unix::net::UnixStream;
use std::time::Duration;

type DriverPair = (FileActorPort<FileOversight>, FileSupervisedDriver, UnixStream);

fn input_for(host: &FileOversight, request: u64) -> CommitteeInput {
    let action = host.request_action(request).unwrap();
    let helper = &host.profile.committee.members()["reviewer"];
    let epoch = action.spec().policy_epoch;
    let mut bytes = action_frame(action);
    let boundary = bytes.len();
    bytes.extend_from_slice(helper.question());
    let end = bytes.len();
    let actual = ActualHelperInput::new(bytes, helper.profile_at(epoch), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
    ], Vec::new()).unwrap();
    let view = EvidenceViewManifest::new(actual, AuthorizationProjection {
        projection_id: 7, policy_epoch: epoch, projected_originals: Vec::new(),
    }, Vec::new()).unwrap();
    CommitteeInput::capture(action, &host.profile.committee,
        BTreeMap::from([("reviewer".to_owned(), view)])).unwrap()
}

fn start(host: FileOversight, request: u64, round: u64) -> DriverPair {
    let inputs = input_for(&host, request);
    let (port, mut driver) = host.into_supervised_driver();
    let (worker, peer) = UnixStream::pair().unwrap();
    driver.start_review(FileDriverLaunch {
        request, round, evidence_root: [9; 32],
        window: ReviewWindow { commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) },
        expected_input_revision: 0, inputs,
        workers: BTreeMap::from([("reviewer".to_owned(), worker)]),
        limits: HelperLimits::default(),
    }, snapshot(), || ElapsedTick(1)).unwrap();
    assert_eq!(driver.phase(), FileDriverPhase::Reviewing { request });
    assert!(driver.next_review_deadline().is_some());
    (port, driver, peer)
}

fn reviewing(root: &Directory) -> DriverPair {
    let p = profile();
    let (mut host, _reviewer) = FileOversight::create(root.store(), p.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.enable_publication_guard(host.revision()).unwrap();
    host.submit_request(host.revision(), REQUEST, ActionSpec {
        version: VERSION, scope: p.delivery.scope, target: Some(p.delivery.target),
        payload: b"visible".to_vec(), required_witnesses: Vec::new(), policy_epoch: 0,
        deadline: ElapsedTick(100), units: 16,
    }, snapshot()).unwrap();
    start(host, REQUEST, 101)
}

fn assert_peer_closed(peer: &mut UnixStream) {
    // A queued input frame may precede EOF; this asserts closure, not that no
    // bytes were previously sent. A still-open regressed worker fails promptly.
    peer.set_read_timeout(Some(Duration::from_millis(250))).unwrap();
    let mut bytes = Vec::new();
    peer.take(65_537).read_to_end(&mut bytes).unwrap();
    assert!(bytes.len() <= 65_536);
}

fn assert_idle_without_sampling(driver: &mut FileSupervisedDriver) {
    assert_eq!(driver.phase(), FileDriverPhase::Idle);
    assert!(matches!(driver.step_with_evidence(
        || panic!("terminal settlement must not sample time"),
        |_, _| panic!("terminal settlement must not reacquire evidence"), None,
    ).unwrap(), FileDriverEvent::Idle));
}

#[test]
fn driver_settlement_closes_active_review_sockets_and_preserves_the_original_port() {
    let root = Directory::new();
    let (_port, mut driver, mut peer) = reviewing(&root);
    let before = driver.supervisor().host().unwrap().inspect();
    let status = driver.cancel_and_resolve_active(ElapsedTick(2)).unwrap();
    assert_eq!(status.disposition, disposition(ActionState::Cancelled));
    assert_idle_without_sampling(&mut driver);
    assert!(driver.next_review_deadline().is_none());
    assert_peer_closed(&mut peer);
    assert!(driver.helpers_reaped()); // socket-only fixture, not a child-process claim
    let host = driver.supervisor().host().unwrap();
    assert_eq!(host.revision(), before.revision + 2);
    assert_eq!(host.request_status(REQUEST), Ok(status));
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert!(host.inspect().stop.is_none());
    assert_eq!(host.retained_requests(), 1);
}

#[test]
fn driver_settlement_preserves_a_healthy_review_on_stale_or_missing_inputs() {
    let root = Directory::new();
    let (_port, mut driver, mut peer) = reviewing(&root);
    let before = driver.supervisor().host().unwrap().inspect();
    for (revision, request, tick, error) in [
        (before.revision - 1, REQUEST, 2, Error::Stale),
        (before.revision, REQUEST, 0, Error::Stale),
        (before.revision, 999, 2, Error::Missing),
    ] {
        assert_eq!(driver.cancel_and_resolve_request(revision, request, ElapsedTick(tick)),
            Err(JournalError::Contract(error)));
        assert_eq!(driver.phase(), FileDriverPhase::Reviewing { request: REQUEST });
        assert!(driver.next_review_deadline().is_some());
        assert_eq!(driver.supervisor().host().unwrap().inspect(), before);
    }
    assert_eq!(driver.cancel_and_resolve_active(ElapsedTick(2)).unwrap().disposition,
        disposition(ActionState::Cancelled));
    assert_peer_closed(&mut peer);
    assert_idle_without_sampling(&mut driver);
}

#[test]
fn driver_settlement_of_an_old_terminal_request_cannot_retire_a_new_review() {
    let root = Directory::new();
    let mut r = request_ready(&root, profile());
    let old = settle(&mut r, 1).unwrap();
    let mut spec = r.action.spec().clone();
    spec.required_witnesses.clear();
    r.host.submit_request(r.host.revision(), 900, spec, snapshot()).unwrap();
    let Ready { host, .. } = r;
    let (_port, mut driver, mut peer) = start(host, 900, 102);
    let before = driver.supervisor().host().unwrap().inspect();
    assert_eq!(driver.cancel_and_resolve_request(0, REQUEST, ElapsedTick(0)), Ok(old));
    assert_eq!(driver.phase(), FileDriverPhase::Reviewing { request: 900 });
    assert!(driver.next_review_deadline().is_some());
    assert_eq!(driver.supervisor().host().unwrap().inspect(), before);
    let next = driver.cancel_and_resolve_active(ElapsedTick(2)).unwrap();
    assert_eq!(next.request, 900);
    assert_eq!(next.disposition, FileRequestDisposition::Admitted { attempt: 2, stage: ActionState::Cancelled });
    assert_peer_closed(&mut peer);
}

#[test]
fn driver_settlement_resolves_reopened_obligations_without_provider_or_reviewer() {
    for executed in [false, true] {
        let root = Directory::new();
        let mut r = request_ready(&root, profile());
        dispatch_ready(&mut r);
        if executed {
            r.host.publish_checked(r.host.revision(), 1, Some(&r.inputs),
                snapshot(), ElapsedTick(2)).unwrap();
        }
        drop(r);
        let (host, _reviewer) = FileOversight::open(root.store(), profile()).unwrap();
        assert!(!host.clock_ready());
        let (_port, mut driver) = host.into_supervised_driver();
        driver.resume_reconciliation(REQUEST).unwrap();
        assert_eq!(driver.phase(), FileDriverPhase::AwaitingReconciliation { request: REQUEST });
        let status = driver.cancel_and_resolve_active(ElapsedTick(3)).unwrap();
        assert_eq!(status.disposition, disposition(if executed {
            ActionState::Confirmed
        } else {
            ActionState::ConfirmedNotExecuted
        }));
        assert_idle_without_sampling(&mut driver);
        let owner = driver.supervisor().host().unwrap();
        assert_eq!(owner.inspect().executions, u64::from(executed));
        assert_eq!(owner.inspect().control.ledger.charged, if executed { 16 } else { 0 });
        assert!(owner.request_resolution(REQUEST).unwrap().is_some());
    }
}

#[test]
fn driver_settlement_storage_failure_retires_io_without_claiming_a_refund() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
        JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new();
        let (_port, mut driver, mut peer) = reviewing(&root);
        let before = driver.supervisor().host().unwrap().inspect();
        {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            let store = &mut host.store;
            store.fail_once(barrier);
        }
        let JournalError::Io(failure) = driver.cancel_and_resolve_active(ElapsedTick(2)).unwrap_err() else {
            panic!("selected storage barrier");
        };
        assert_eq!(failure.operation, barrier);
        assert_eq!(driver.phase(), FileDriverPhase::Idle);
        assert_peer_closed(&mut peer);
        assert_eq!(driver.supervisor().host().unwrap().inspect(), before);
        assert!(matches!(driver.step_with_evidence(
            || panic!("faulted owner must not sample time"),
            |_, _| panic!("faulted owner must not capture evidence"), None,
        ), Err(JournalError::Unavailable)));
        assert_eq!(driver.cancel_and_resolve_request(0, REQUEST, ElapsedTick(0)),
            Err(JournalError::Unavailable));
        let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
        if barrier == JournalIo::DirectorySync {
            assert_eq!(disk.revision, before.revision + 2);
            assert_eq!(disk.control.ledger.stages[&1], ActionState::Cancelled);
        } else {
            assert_eq!(disk, before);
        }
        assert_eq!(disk.executions, 0);
    }
}

#[test]
fn driver_settlement_retention_failure_keeps_the_pending_charge_and_query_phase() {
    let root = Directory::new();
    let mut r = request_ready(&root, profile());
    dispatch_ready(&mut r);
    let Ready { host, .. } = r;
    let (_port, mut driver) = host.into_supervised_driver();
    driver.resume_reconciliation(REQUEST).unwrap();
    let before = driver.supervisor().host().unwrap().inspect();
    assert_eq!(driver.cancel_and_resolve_active(ElapsedTick(1001)),
        Err(JournalError::Contract(Error::Stale)));
    assert_eq!(driver.phase(), FileDriverPhase::AwaitingReconciliation { request: REQUEST });
    assert_eq!(driver.supervisor().host().unwrap().inspect(), before);
    assert_eq!(before.control.ledger.charged, 16);
    assert_eq!(before.executions, 0);
}

#[test]
fn driver_settlement_pending_sweep_preflights_clock_and_reconciliation_together() {
    let baseline_root = Directory::new();
    let mut baseline = request_ready(&baseline_root, profile());
    dispatch_ready(&mut baseline);
    let count = baseline.host.revision() as usize;
    for spare in [1, 2] {
        let root = Directory::new();
        let mut p = profile();
        p.delivery.limits.events = count + spare;
        let mut r = request_ready(&root, p.clone());
        dispatch_ready(&mut r);
        let Ready { host, .. } = r;
        let (_port, mut driver) = host.into_supervised_driver();
        driver.resume_reconciliation(REQUEST).unwrap();
        let before = driver.supervisor().host().unwrap().inspect();
        // Exact original human-key deadline. Only endpoint evidence refunds it.
        let result = driver.reconcile_pending(ElapsedTick(10));
        if spare == 1 {
            assert_eq!(result, Err(JournalError::Contract(Error::Limit)));
            assert_eq!(driver.supervisor().host().unwrap().inspect(), before);
            assert_eq!(FileOversight::read_publication(root.store(), &p).unwrap(), before);
            assert_eq!(driver.phase(), FileDriverPhase::AwaitingReconciliation { request: REQUEST });
        } else {
            let outcomes = result.unwrap();
            assert_eq!(outcomes[&1], Ok(super::super::super::super::Reconciliation::Resolved(
                EndpointOutcome::NotExecuted { reason: NonExecutionReason::DeadlineElapsed })));
            let after = driver.supervisor().host().unwrap().inspect();
            assert_eq!(after.revision, before.revision + 2);
            assert_eq!(after.control.ledger.available, 100);
            assert_eq!(after.control.ledger.charged, 0);
            assert_eq!(after.executions, 0);
            assert_eq!(FileOversight::read_publication(root.store(), &p).unwrap(), after);
            assert!(matches!(driver.step_with_evidence(
                || panic!("terminal native ledger must precede sampling time"),
                |_, _| panic!("terminal native ledger must precede evidence capture"), None,
            ).unwrap(), FileDriverEvent::Stopped { request: REQUEST, stage: ActionState::ConfirmedNotExecuted }));
        }
    }
}
