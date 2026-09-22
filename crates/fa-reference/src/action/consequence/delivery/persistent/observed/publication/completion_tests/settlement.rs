//! Real original requests, rounds, approvals and endpoint transitions.
use super::*;
use crate::action::consequence::delivery::NonExecutionReason;
use crate::action::consequence::delivery::persistent::requests::{FileRequestDisposition, FileRequestStatus};

const REQUEST: u64 = 700;

fn request_ready(root: &Directory, p: FileOversightProfile) -> Ready {
    let (mut host, reviewer) = FileOversight::create(root.store(), p.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.enable_publication_guard(host.revision()).unwrap();
    let status = host.submit_request(host.revision(), REQUEST, ActionSpec { version: VERSION, scope: p.delivery.scope,
        target: Some(host.inspect().target), payload: b"visible".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: 0, deadline: ElapsedTick(100), units: 16 }, snapshot()).unwrap();
    assert_eq!(status.disposition, FileRequestDisposition::Admitted { attempt: 1, stage: ActionState::Reviewing });
    let action = host.request_action(REQUEST).unwrap().clone();
    let helper = &p.committee.members()["reviewer"];
    let mut bytes = action_frame(&action);
    let boundary = bytes.len();
    bytes.extend_from_slice(helper.question());
    let end = bytes.len();
    let actual = ActualHelperInput::new(bytes, helper.profile_at(0), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
    ], Vec::new()).unwrap();
    let manifest = EvidenceViewManifest::new(actual, AuthorizationProjection {
        projection_id: 7, policy_epoch: 0, projected_originals: Vec::new(),
    }, Vec::new()).unwrap();
    let inputs = CommitteeInput::capture(&action, &p.committee,
        BTreeMap::from([("reviewer".to_owned(), manifest)])).unwrap();
    host.record_inputs(host.revision(), 1, 0, inputs.clone()).unwrap();
    host.begin_review(host.revision(), 1, 101, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8),
    }, snapshot()).unwrap();
    let digest = commitment(101, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap();
    host.commit_review(host.revision(), 101, "reviewer", digest).unwrap();
    host.open_reveals(host.revision(), 101).unwrap();
    host.reveal_review(host.revision(), 101, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), 101, Some(&inputs), snapshot()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), 1, &inputs, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(10)).unwrap();
    let revision = host.revision();
    let human = reviewer.approve(&mut host, revision, &request).unwrap();
    Ready { host, reviewer, action, inputs, automatic, human }
}

fn disposition(stage: ActionState) -> FileRequestDisposition {
    FileRequestDisposition::Admitted { attempt: 1, stage }
}

fn settle(r: &mut Ready, at: u64) -> Result<FileRequestStatus, JournalError> {
    r.host.cancel_and_resolve_request(r.host.revision(), REQUEST, ElapsedTick(at))
}

#[test]
fn settlement_cancels_reserved_work_without_an_endpoint_receipt_or_reusable_keys() {
    let root = Directory::new();
    let mut r = request_ready(&root, profile());
    let before = r.host.inspect();
    assert_eq!(before.control.ledger.reserved, 16);
    let status = settle(&mut r, 2).unwrap();
    assert_eq!(status.disposition, disposition(ActionState::Cancelled));
    assert_eq!(status.generation, 1);
    let after = r.host.inspect();
    assert_eq!(after.revision, before.revision + 2);
    assert_eq!(after.control.ledger.available, 100);
    assert_eq!(after.control.ledger.reserved, 0);
    assert_eq!(after.control.ledger.charged, 0);
    assert_eq!(after.executions, 0);
    assert_eq!(after.payload, b"initial");
    assert_eq!(r.host.request_resolution(REQUEST), Ok(None));
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), after);
    assert!(r.complete(3).is_err());
    assert_eq!(r.host.inspect(), after);
    // The returned historical status is not a new clock observation.
    let bytes = r.bytes();
    assert_eq!(r.host.cancel_and_resolve_request(0, REQUEST, ElapsedTick(0)), Ok(status));
    assert_eq!(r.host.inspect(), after);
    assert_eq!(r.bytes(), bytes);
}

#[test]
fn settlement_seals_dispatched_work_and_a_delayed_publication_cannot_execute() {
    let root = Directory::new();
    let mut r = request_ready(&root, profile());
    dispatch_ready(&mut r);
    let before = r.host.inspect();
    // The deliberately weaker actor cancellation cannot pretend this is settled.
    r.host.cancel_request(r.host.revision(), REQUEST).unwrap();
    assert_eq!(r.host.inspect(), before);
    assert_eq!(r.host.request_resolution(REQUEST), Ok(None));
    let status = settle(&mut r, 2).unwrap();
    assert_eq!(status.disposition, disposition(ActionState::ConfirmedNotExecuted));
    let outcome = EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed };
    assert_eq!(r.host.request_resolution(REQUEST), Ok(Some(outcome)));
    let after = r.host.inspect();
    assert_eq!(after.revision, before.revision + 2);
    assert_eq!(after.control.ledger.available, 100);
    assert_eq!(after.control.ledger.charged, 0);
    assert_eq!(after.executions, 0);
    assert_eq!(after.payload, b"initial");
    let late = r.host.publish_checked(r.host.revision(), 1, Some(&r.inputs),
        snapshot(), ElapsedTick(3)).unwrap();
    assert_eq!(late.outcome, outcome);
    assert_eq!(late.basis, PublicationBasis::PreviouslyResolved);
    assert_eq!(r.host.inspect().executions, 0);
    assert_eq!(r.host.inspect().control.ledger.available, 100);
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), r.host.inspect());
}

#[test]
fn settlement_preserves_a_real_execution_when_its_acknowledgment_was_lost() {
    let root = Directory::new();
    let mut r = request_ready(&root, profile());
    dispatch_ready(&mut r);
    let publication = r.host.publish_checked(r.host.revision(), 1, Some(&r.inputs),
        snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(publication.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(r.host.request_resolution(REQUEST), Ok(None));
    assert_eq!(r.host.inspect().control.ledger.stages[&1], ActionState::Dispatching);
    let status = settle(&mut r, 3).unwrap();
    assert_eq!(status.disposition, disposition(ActionState::Confirmed));
    assert_eq!(r.host.request_resolution(REQUEST), Ok(Some(publication.outcome)));
    let after = r.host.inspect();
    assert_eq!(after.payload, b"visible");
    assert_eq!(after.executions, 1);
    assert_eq!(after.control.ledger.available, 84);
    assert_eq!(after.control.ledger.charged, 16);
    let bytes = r.bytes();
    assert_eq!(r.host.cancel_and_resolve_request(0, REQUEST, ElapsedTick(u64::MAX)), Ok(status));
    assert_eq!(r.host.inspect(), after);
    assert_eq!(r.bytes(), bytes);
}

#[test]
fn settlement_of_unknown_work_survives_reopen_without_old_approval_keys() {
    for executed in [false, true] {
        let root = Directory::new();
        let mut r = request_ready(&root, profile());
        dispatch_ready(&mut r);
        if executed {
            r.host.publish_checked(r.host.revision(), 1, Some(&r.inputs), snapshot(), ElapsedTick(2)).unwrap();
        } else {
            assert_eq!(r.host.reconcile(r.host.revision(), 1).unwrap(),
                super::super::super::Reconciliation::AwaitingResolution);
            assert_eq!(r.host.inspect().control.ledger.stages[&1], ActionState::Unknown);
        }
        drop(r);
        let (mut owner, _) = FileOversight::open(root.store(), profile()).unwrap();
        assert!(!owner.clock_ready());
        let status = owner.cancel_and_resolve_request(owner.revision(), REQUEST, ElapsedTick(3)).unwrap();
        assert_eq!(status.disposition, disposition(if executed {
            ActionState::Confirmed
        } else {
            ActionState::ConfirmedNotExecuted
        }));
        assert_eq!(owner.inspect().executions, u64::from(executed));
        assert_eq!(owner.inspect().control.ledger.charged, if executed { 16 } else { 0 });
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), owner.inspect());
    }
}

#[test]
fn settlement_does_not_cancel_fence_or_re_review_an_unrelated_request() {
    let root = Directory::new();
    let mut r = request_ready(&root, profile());
    dispatch_ready(&mut r);
    let mut spec = r.action.spec().clone();
    spec.required_witnesses.clear();
    spec.payload = b"unrelated".to_vec();
    let other = r.host.submit_request(r.host.revision(), 900, spec, snapshot()).unwrap();
    assert_eq!(other.disposition, FileRequestDisposition::Admitted { attempt: 2, stage: ActionState::Reviewing });
    let before = r.host.inspect();
    settle(&mut r, 2).unwrap();
    assert_eq!(r.host.request_status(900), Ok(other));
    let after = r.host.inspect();
    assert_eq!(after.dispatcher_epoch, before.dispatcher_epoch);
    assert_eq!(after.control.ledger.epoch, before.control.ledger.epoch);
    assert!(!after.control.suspended);
    assert!(after.stop.is_none());
    assert_eq!(after.control.ledger.available, 100);
    assert_eq!(after.executions, 0);
}

#[test]
fn settlement_uses_native_source_admission_without_clearing_an_interruption() {
    for dispatched in [false, true] {
        let root = Directory::new();
        let mut r = request_ready(&root, profile());
        if dispatched { dispatch_ready(&mut r); }
        // Existing private source-interruption seam; no fake evidence or receipt.
        r.host.source_interrupted = true;
        let status = settle(&mut r, 2).unwrap();
        assert_eq!(status.disposition, disposition(if dispatched {
            ActionState::ConfirmedNotExecuted
        } else {
            ActionState::Cancelled
        }));
        assert!(r.host.source_interrupted);
        assert_eq!(r.host.inspect().control.ledger.available, 100);
        let mut spec = r.action.spec().clone();
        spec.required_witnesses.clear();
        let before = r.host.inspect();
        assert_eq!(r.host.submit_request(r.host.revision(), 901, spec, snapshot()),
            Err(JournalError::Contract(Error::Incomplete)));
        assert_eq!(r.host.inspect(), before);
    }
}

#[test]
fn settlement_predecessor_clock_and_retention_refusals_commit_no_partial_time() {
    let root = Directory::new();
    let mut r = request_ready(&root, profile());
    dispatch_ready(&mut r);
    let before = r.host.inspect();
    let bytes = r.bytes();
    for (revision, request, tick, error) in [
        (before.revision - 1, REQUEST, 2, Error::Stale),
        (before.revision, REQUEST, 0, Error::Stale),
        (before.revision, REQUEST, 1001, Error::Stale), // exact retention end
        (before.revision, 0, 2, Error::Missing),
    ] {
        assert_eq!(r.host.cancel_and_resolve_request(revision, request, ElapsedTick(tick)),
            Err(JournalError::Contract(error)));
        assert_eq!(r.host.inspect(), before);
        assert_eq!(r.bytes(), bytes);
        assert!(r.host.storage_failure().is_none());
    }
    // Independent virtual-clock control strictly inside the retention interval.
    let other = Directory::new();
    let mut valid = request_ready(&other, profile());
    dispatch_ready(&mut valid);
    assert_eq!(settle(&mut valid, 1000).unwrap().disposition,
        disposition(ActionState::ConfirmedNotExecuted));
}

#[test]
fn settlement_preflights_both_event_slots_before_releasing_a_charge() {
    let baseline_root = Directory::new();
    let mut baseline = request_ready(&baseline_root, profile());
    dispatch_ready(&mut baseline);
    let count = usize::try_from(baseline.host.revision()).unwrap();
    for spare in [1, 2] {
        let root = Directory::new();
        let mut p = profile();
        p.delivery.limits.events = count + spare;
        let mut r = request_ready(&root, p.clone());
        dispatch_ready(&mut r);
        let before = r.host.inspect();
        let bytes = r.bytes();
        let result = settle(&mut r, 2);
        if spare == 1 {
            assert_eq!(result, Err(JournalError::Contract(Error::Limit)));
            assert_eq!(r.host.inspect(), before);
            assert_eq!(r.bytes(), bytes);
            assert!(r.host.storage_failure().is_none());
        } else {
            assert_eq!(result.unwrap().disposition, disposition(ActionState::ConfirmedNotExecuted));
            assert_eq!(r.host.revision(), (count + 2) as u64);
            assert_eq!(r.host.inspect().control.ledger.available, 100);
        }
        assert_eq!(FileOversight::read_publication(root.store(), &p).unwrap(), r.host.inspect());
    }
}

#[test]
fn settlement_all_storage_barriers_preserve_old_or_complete_native_outcomes() {
    for phase in 0..3 {
        for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
            JournalIo::Rename, JournalIo::DirectorySync] {
            let root = Directory::new();
            let mut r = request_ready(&root, profile());
            if phase != 0 { dispatch_ready(&mut r); }
            if phase == 2 {
                r.host.publish_checked(r.host.revision(), 1, Some(&r.inputs),
                    snapshot(), ElapsedTick(2)).unwrap();
            }
            let before = r.host.inspect();
            r.host.store.fail_once(barrier);
            let JournalError::Io(failure) = settle(&mut r, 3).unwrap_err() else {
                panic!("original replacement barrier must fail");
            };
            assert_eq!(failure.operation, barrier);
            assert_eq!(r.host.inspect(), before);
            assert!(!r.host.clock_ready());
            assert_eq!(settle(&mut r, 3), Err(JournalError::Unavailable));
            let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
            let expected = match phase {
                0 => ActionState::Cancelled,
                1 => ActionState::ConfirmedNotExecuted,
                2 => ActionState::Confirmed,
                _ => unreachable!(),
            };
            if barrier == JournalIo::DirectorySync {
                assert_eq!(disk.revision, before.revision + 2);
                assert_eq!(disk.control.ledger.stages[&1], expected);
                assert_eq!(disk.control.ledger.reserved, 0);
                assert_eq!(disk.control.ledger.charged, if phase == 2 { 16 } else { 0 });
            } else {
                assert_eq!(disk, before);
            }
            assert_eq!(disk.executions, u64::from(phase == 2));
            drop(r);
            let (mut owner, _) = FileOversight::open(root.store(), profile()).unwrap();
            let status = owner.cancel_and_resolve_request(owner.revision(), REQUEST, ElapsedTick(4)).unwrap();
            assert_eq!(status.disposition, disposition(expected));
            assert_eq!(owner.inspect().control.ledger.available, if phase == 2 { 84 } else { 100 });
            assert_eq!(owner.inspect().executions, u64::from(phase == 2));
        }
    }
}

#[test]
fn settlement_terminal_retry_still_refuses_a_faulted_owner() {
    let root = Directory::new();
    let mut r = request_ready(&root, profile());
    settle(&mut r, 2).unwrap();
    r.host.store.fail_once(JournalIo::DirectorySync);
    assert!(matches!(r.host.observe_time(r.host.revision(), ElapsedTick(3)), Err(JournalError::Io(_))));
    assert_eq!(r.host.cancel_and_resolve_request(0, REQUEST, ElapsedTick(0)),
        Err(JournalError::Unavailable));
}
