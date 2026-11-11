//! Actual original congress, separate human keys and native publication effects.
use super::*;
use super::super::{FileRequestDisposition, JournalError};
use crate::action::ActionState;
use crate::action::consequence::delivery::{EndpointOutcome, StopRequest};
use crate::action::consequence::delivery::persistent::{JournalIo, Reconciliation};
use crate::action::consequence::delivery::persistent::observed::narrowing::FileAuthorityNarrowRequest;
use crate::action::consequence::oversight::actor::UnknownReason;
use crate::Error;

#[path = "../hold_diagnosis/tests/fixture.rs"]
mod fixture;
use fixture::*;

fn plan(result: FileNarrowPlanResult) -> FileNarrowPlan {
    let Ok(Knowledge::Known { value, basis }) = result else { panic!("expected native plan: {result:?}"); };
    assert_eq!(value.basis(), basis);
    assert_eq!(value.can_narrow(), value.request().is_some());
    value
}
fn receipt(result: FileNarrowResult) -> FileNarrowReceipt {
    let Ok(Knowledge::Known { value, basis }) = result else { panic!("expected acknowledged restriction: {result:?}"); };
    assert_eq!(value.after(), basis);
    assert_eq!(value.before().journal_revision + 1, basis.journal_revision);
    assert_eq!(value.before().control_sequence + 1, basis.control_sequence);
    assert_eq!(value.before().authority_epoch, basis.authority_epoch);
    assert_eq!(value.before().recorded_at, basis.recorded_at);
    value
}
fn submit(host: &mut FileOversight, key: u64) {
    let mut selected = spec(b"ok");
    selected.target = Some(host.inspect().target);
    host.submit_request(host.revision(), key, selected, snapshot()).unwrap();
}
fn native(host: &FileOversight, operation: u64, targets: Vec<ResolvedTarget>) -> FileAuthorityNarrowRequest {
    let cut = host.inspect();
    FileAuthorityNarrowRequest::new(operation, cut.control.sequence, cut.control.ledger.epoch, targets).unwrap()
}

#[test]
fn actual_two_key_publication_continues_inside_intersection_and_unknown_cost_stays_charged() {
    let root = Root::new();
    let (mut host, reviewer) = create(&root);
    // This is a real version-1 reservation. A separate actual publication will
    // advance the resource before a version-2 action is independently admitted.
    submit(&mut host, 700);
    let (outside, outside_inputs, outside_permit, outside_human) = prepare_existing(&mut host, 700);
    let revision = host.revision();
    let outside_human = reviewer.approve(&mut host, revision, &outside_human).unwrap();
    submit(&mut host, 701);
    dispatch_existing(&mut host, &reviewer, 701);
    assert_eq!(host.reconcile(host.revision(), 2).unwrap(), Reconciliation::AwaitingResolution);
    submit(&mut host, 702);
    dispatch_existing(&mut host, &reviewer, 702);
    assert_eq!(host.publish(host.revision(), 3).unwrap(), EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 3).unwrap();
    submit(&mut host, 703);
    let (inside, inputs, automatic, human) = prepare_existing(&mut host, 703);
    let revision = host.revision();
    let old_human = reviewer.approve(&mut host, revision, &human).unwrap();
    let retained = inside.spec().target.unwrap();
    assert_ne!(retained, outside.spec().target.unwrap());
    let before = host.inspect();
    let (actor, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.narrow_port(4, 2, MAX_NARROW_REPLAY_EVENTS).unwrap();
    supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
    let files = root.files();
    let offered = plan(port.next(900, vec![retained]));
    assert_eq!(offered.current_ceiling(), None);
    assert_eq!(offered.effective_targets(), &[retained]);
    assert_eq!(root.files(), files);
    assert_eq!(grant.remaining_operations(), 2);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    let input = offered.request().unwrap();
    let acknowledged = receipt(port.narrow(input.clone()));
    assert_eq!(acknowledged.previous_ceiling(), None);
    assert_eq!(acknowledged.cancelled(), &[1]);
    assert_eq!(acknowledged.refunded_units(), 8);
    assert_eq!(acknowledged.effective_targets(), &[retained]);
    assert_eq!(grant.remaining_operations(), 1);
    assert!(supervisor.owner.borrow().snapshot.is_none());
    let tail = supervisor.host().unwrap().capture_journal_tail(before.revision).unwrap().unwrap();
    assert_eq!(tail.events.len(), 1);
    let event = tail.events[0];
    use crate::action::consequence::delivery::persistent::observed::replay::tail::FileJournalEventKind;
    assert_eq!(event.kind(), FileJournalEventKind::NarrowAuthority);
    assert_eq!(FileJournalEventKind::from_label(event.kind().as_str()), Some(event.kind()));
    assert_eq!((event.attempt(), event.request(), event.round()), (None, None, None));
    assert_eq!(event.revision(), acknowledged.after().journal_revision);
    {
        let mut host = supervisor.host_mut().unwrap();
        let now = host.inspect();
        assert_eq!(now.control.ledger.stages[&1], ActionState::Cancelled);
        assert_eq!(now.control.ledger.stages[&2], ActionState::Unknown);
        assert_eq!(now.control.ledger.stages[&4], ActionState::Authorized);
        assert_eq!(now.control.ledger.charged, before.control.ledger.charged);
        assert_eq!(now.control.ledger.reserved, 8);
        assert_eq!(now.payload, before.payload);
        assert_eq!(now.executions, 1);
        let revision = host.revision();
        assert!(host.dispatch(revision, &outside_permit, &outside_human, &outside,
            &outside_inputs, snapshot()).is_err());
        let revision = host.revision();
        assert!(host.dispatch(revision, &automatic, &old_human, &inside, &inputs, snapshot()).is_err());
        // Reuse the retained automatic reservation, but acquire the separately
        // authorized human key for this exact new control predecessor.
        let revision = host.revision();
        let fresh = host.request_human_approval(revision, 2004, 4, &inputs, ElapsedTick(10)).unwrap();
        let revision = host.revision();
        let approved = reviewer.approve(&mut host, revision, &fresh).unwrap();
        let revision = host.revision();
        host.dispatch(revision, &automatic, &approved, &inside, &inputs, snapshot()).unwrap();
        let revision = host.revision();
        assert_eq!(host.publish(revision, 4).unwrap(), EndpointOutcome::Executed { resulting_version: 3 });
        let revision = host.revision();
        host.reconcile(revision, 4).unwrap();
        assert_eq!(host.inspect().control.ledger.stages[&2], ActionState::Unknown);
        assert_eq!(host.inspect().control.ledger.charged, before.control.ledger.charged + 8);
    }
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
    let files = root.files();
    assert_eq!(receipt(port.clone().narrow(input)), acknowledged);
    assert_eq!(root.files(), files);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    assert_eq!(grant.remaining_operations(), 1);
    let current = supervisor.host().unwrap().inspect().target;
    let wider = plan(port.next(901, vec![current, retained]));
    assert_eq!(wider.effective_targets(), &[retained]);
    assert_eq!(receipt(port.narrow(wider.request().unwrap())).effective_targets(), &[retained]);
    drop(actor);
}

#[test]
fn native_exact_retry_survives_reopen_without_widening_or_another_event() {
    let root = Root::new();
    let mut host = owner(&root);
    let first = native(&host, 7, vec![profile().delivery.target]);
    let original = host.narrow_authority(host.revision(), first.clone()).unwrap();
    let empty = native(&host, 8, Vec::new());
    host.narrow_authority(host.revision(), empty).unwrap();
    let before = host.inspect();
    let files = root.files();
    assert_eq!(host.narrow_authority(host.revision(), first.clone()).unwrap(), original);
    assert_eq!(host.inspect(), before);
    assert_eq!(root.files(), files);
    let changed = FileAuthorityNarrowRequest::new(7, first.expected_control_sequence(),
        first.expected_authority_epoch(), Vec::new()).unwrap();
    assert!(matches!(host.narrow_authority(host.revision(), changed), Err(JournalError::Contract(Error::Binding))));
    drop(host);
    let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
    let before = host.inspect();
    let files = root.files();
    assert_eq!(host.narrow_authority(host.revision(), first).unwrap(), original);
    assert_eq!(host.inspect(), before);
    assert_eq!(root.files(), files);
    assert_eq!(before.control.ceiling.unwrap().targets(), Vec::new());
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.narrow_port(1, 1, MAX_NARROW_REPLAY_EVENTS).unwrap();
    let offered = plan(port.next(7, vec![profile().delivery.target]));
    assert_eq!(offered.block_reason(), Some(FileNarrowBlockReason::OperationAlreadyUsed));
    assert_eq!(port.narrow(FileNarrowRequest::new(7, offered.basis(), vec![profile().delivery.target]).unwrap()),
        Err(FileNarrowError::IdempotencyConflict));
    assert_eq!(grant.remaining_operations(), 1);
}

#[test]
fn durable_operation_retention_is_finite_and_exact_retry_stays_available() {
    use crate::action::consequence::delivery::persistent::observed::narrowing::MAX_AUTHORITY_NARROWINGS;
    let root = Root::new();
    let mut host = owner(&root);
    let first = native(&host, 1, Vec::new());
    let original = host.narrow_authority(host.revision(), first.clone()).unwrap();
    for operation in 2..=MAX_AUTHORITY_NARROWINGS as u64 {
        let input = native(&host, operation, Vec::new());
        host.narrow_authority(host.revision(), input).unwrap();
    }
    let before = host.inspect();
    let files = root.files();
    let extra = native(&host, MAX_AUTHORITY_NARROWINGS as u64 + 1, Vec::new());
    assert_eq!(host.narrow_authority(host.revision(), extra), Err(JournalError::Contract(Error::Limit)));
    assert_eq!(host.narrow_authority(host.revision(), first).unwrap(), original);
    assert_eq!(host.inspect(), before);
    assert_eq!(root.files(), files);
    assert_eq!(before.control.sequence, MAX_AUTHORITY_NARROWINGS as u64);
    assert_eq!(before.control.ledger.epoch, 0);
}

#[test]
fn original_issuer_custody_does_not_follow_an_equal_scope_host_replacement() {
    let root = Root::new();
    let (_, mut supervisor) = owner(&root).into_actor_gateway();
    let (port, grant) = supervisor.narrow_port(3, 2, MAX_NARROW_REPLAY_EVENTS).unwrap();
    let input = plan(port.next(1, Vec::new())).request().unwrap();
    receipt(port.narrow(input.clone()));
    let replacement_root = Root::new();
    let replacement = owner(&replacement_root);
    let before = replacement.inspect();
    let files = replacement_root.files();
    *supervisor.host_mut().unwrap() = replacement;
    assert_eq!(port.next(2, Vec::new()), Ok(Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }));
    assert_eq!(port.narrow(input), Ok(Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }));
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(replacement_root.files(), files);
}

#[test]
fn malformed_stale_short_and_reused_requests_do_not_spend_mutation_or_snapshot() {
    let root = Root::new();
    let host = owner(&root);
    let before = host.inspect();
    let (_, mut supervisor) = host.into_actor_gateway();
    supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
    let (short, short_grant) = supervisor.narrow_port(1, 1, before.revision as usize).unwrap();
    let short_plan = plan(short.next(1, Vec::new()));
    assert_eq!(short_plan.block_reason(), Some(FileNarrowBlockReason::ReplayLimit));
    assert_eq!(short.narrow(FileNarrowRequest::new(1, short_plan.basis(), Vec::new()).unwrap()), Err(FileNarrowError::Capacity));
    assert_eq!(short_grant.remaining_operations(), 1);
    let (port, grant) = supervisor.narrow_port(2, 1, MAX_NARROW_REPLAY_EVENTS).unwrap();
    let target = profile().delivery.target;
    assert_eq!(port.next(0, Vec::new()), Err(FileNarrowError::MalformedRequest));
    assert_eq!(port.next(1, vec![target, target]), Err(FileNarrowError::MalformedRequest));
    assert_eq!(grant.remaining_queries(), 2);
    let offered = plan(port.next(1, vec![target]));
    let files = root.files();
    for field in 0..6 {
        let mut basis = offered.basis();
        match field {
            0 => basis.scope.run += 1,
            1 => basis.scope.authority += 1,
            2 => basis.journal_revision += 1,
            3 => basis.control_sequence += 1,
            4 => basis.authority_epoch += 1,
            _ => basis.recorded_at = None,
        }
        assert_eq!(port.narrow(FileNarrowRequest::new(1, basis, vec![target]).unwrap()), Err(FileNarrowError::StalePredecessor));
    }
    assert_eq!(root.files(), files);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    assert_eq!(grant.remaining_operations(), 1);
    let input = offered.request().unwrap();
    receipt(port.narrow(input.clone()));
    assert_eq!(port.narrow(FileNarrowRequest::new(1, input.expected(), Vec::new()).unwrap()), Err(FileNarrowError::IdempotencyConflict));
    let another = plan(port.next(2, vec![target]));
    assert_eq!(another.block_reason(), Some(FileNarrowBlockReason::OperationLimit));
    assert_eq!(port.narrow(FileNarrowRequest::new(2, another.basis(), vec![target]).unwrap()), Err(FileNarrowError::Capacity));
    assert_eq!(port.next(3, Vec::new()), Err(FileNarrowError::Capacity));
}

#[test]
fn revoked_foreign_lost_and_busy_custody_cannot_reenter_or_read_the_owner() {
    let root = Root::new();
    let host = owner(&root);
    let (actor, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.narrow_port(3, 1, MAX_NARROW_REPLAY_EVENTS).unwrap();
    let input = plan(port.next(1, Vec::new())).request().unwrap();
    let other_root = Root::new();
    let (other, _other_owner) = owner(&other_root).into_actor_gateway();
    assert!(port.belongs_to(&actor));
    assert!(!port.belongs_to(&other));
    {
        let _busy = supervisor.owner.borrow_mut();
        assert_eq!(port.narrow(input.clone()), Ok(Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }));
        assert_eq!(port.next(2, Vec::new()), Ok(Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }));
    }
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(grant.remaining_operations(), 1);
    let cloned = port.clone();
    grant.revoke();
    assert_eq!(cloned.narrow(input), Ok(Knowledge::Withheld { authority_required: "supervisor_narrow" }));
    let (lost, _grant) = supervisor.narrow_port(1, 1, MAX_NARROW_REPLAY_EVENTS).unwrap();
    drop(supervisor);
    assert_eq!(lost.next(3, Vec::new()), Ok(Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }));
}

#[test]
fn all_original_storage_failures_spend_once_and_never_return_candidate_refunds() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Root::new();
        let (mut host, _) = create(&root);
        submit(&mut host, 700);
        prepare_existing(&mut host, 700);
        let before = host.inspect();
        let (_, mut supervisor) = host.into_actor_gateway();
        let (port, grant) = supervisor.narrow_port(1, 1, MAX_NARROW_REPLAY_EVENTS).unwrap();
        let input = plan(port.next(1, Vec::new())).request().unwrap();
        supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
        supervisor.host().unwrap().fail_storage_once(barrier);
        assert_eq!(port.narrow(input.clone()), Err(FileNarrowError::Unavailable));
        assert_eq!(grant.remaining_operations(), 0);
        assert!(supervisor.owner.borrow().snapshot.is_none());
        assert_eq!(supervisor.host().unwrap().inspect(), before);
        assert_eq!(port.narrow(input), Ok(Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }));
        let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
        let replaced = barrier == JournalIo::DirectorySync;
        assert_eq!(disk.revision, before.revision + u64::from(replaced));
        assert_eq!(disk.control.ledger.stages[&1], if replaced { ActionState::Cancelled } else { ActionState::Authorized });
        assert_eq!(disk.control.ledger.reserved, if replaced { 0 } else { 8 });
        assert_eq!(disk.control.ledger.charged, before.control.ledger.charged);
        assert_eq!(disk.executions, before.executions);
        assert_eq!(disk.payload, before.payload);
    }
}

#[test]
fn source_refusal_and_stop_do_not_gain_authority_from_narrowing() {
    let root = Root::new();
    let (mut host, mut source) = source_owner(&root);
    let revision = host.revision();
    assert!(host.refresh_file_source(revision, &mut source, ElapsedTick(0)).is_err());
    let source_status = host.file_source_status().unwrap();
    assert_eq!(source_status.last_refusal, Some(Error::Stale));
    assert!(host.capture_file_policy_state().is_err());
    let reads = source.status().read_attempts;
    let request = native(&host, 1, Vec::new());
    host.narrow_authority(host.revision(), request).unwrap();
    assert_eq!(host.file_source_status().unwrap(), source_status);
    assert!(host.capture_file_policy_state().is_err());
    assert_eq!(source.status().read_attempts, reads);
    let before = host.inspect();
    host.request_stop(host.revision(), StopRequest { operation: 99,
        expected_control_sequence: before.control.sequence, expected_authority_epoch: before.control.ledger.epoch }).unwrap();
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.narrow_port(1, 1, MAX_NARROW_REPLAY_EVENTS).unwrap();
    let offered = plan(port.next(2, Vec::new()));
    assert_eq!(offered.block_reason(), Some(FileNarrowBlockReason::Stopped));
    assert_eq!(port.narrow(FileNarrowRequest::new(2, offered.basis(), Vec::new()).unwrap()), Err(FileNarrowError::Unsupported));
    assert_eq!(grant.remaining_operations(), 1);
}

#[path = "unsupported.rs"]
mod unsupported;
