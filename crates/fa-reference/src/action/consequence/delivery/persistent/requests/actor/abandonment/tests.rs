//! Original requests, actual two-key effects and their unresolved liabilities.
//! No caller-authored outcome or replacement ledger supplies these controls.
use super::super::{FileRequestDisposition, JournalError};
use super::*;
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::EndpointOutcome;
use crate::action::consequence::delivery::persistent::{
    FileAbandonRequest, JournalIo, Reconciliation,
};
use crate::action::consequence::oversight::actor::{ActorError, UnknownReason};

#[path = "../hold_diagnosis/tests/fixture.rs"]
mod fixture;
use fixture::*;

fn plan(result: FileAbandonmentPlanResult) -> FileAbandonmentPlan {
    let Ok(Knowledge::Known { value, basis }) = result else {
        panic!("expected original abandonment plan: {result:?}");
    };
    assert_eq!(value.basis(), basis);
    assert_eq!(value.can_abandon(), value.request().is_some());
    assert_eq!(value.can_abandon(), value.block_reason().is_none());
    if let Some(input) = value.request() {
        assert_eq!(input.expected(), basis);
        assert_eq!(input.operation(), value.operation());
    }
    value
}

fn receipt(result: FileAbandonmentResult) -> FileAbandonmentReceipt {
    let Ok(Knowledge::Known { value, basis }) = result else {
        panic!("expected original liability acknowledgment: {result:?}");
    };
    assert_eq!(value.after(), basis);
    assert_eq!(value.before().scope, basis.scope);
    assert_eq!(value.before().request, basis.request);
    assert_eq!(value.before().attempt, basis.attempt);
    assert_eq!(value.before().recorded_at, basis.recorded_at);
    assert_eq!(value.before().journal_revision + 1, basis.journal_revision);
    assert_eq!(value.before_stage(), ActionState::Unknown);
    assert_eq!(value.stage(), ActionState::IrrecoverablyUnknown);
    value
}

fn unknown<T: fmt::Debug + PartialEq>(
    result: Result<Knowledge<T, FileHoldDiagnosisBasis>, FileAbandonmentError>,
) {
    assert_eq!(
        result,
        Ok(Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        })
    );
}

fn dispatched(root: &Root) -> FileOversight {
    let (mut host, reviewer) = create(root);
    host.submit_request(host.revision(), 701, spec(b"ok"), snapshot())
        .unwrap();
    dispatch_existing(&mut host, &reviewer, 701);
    host
}

fn uncertain(root: &Root, executed: bool) -> FileOversight {
    let mut host = dispatched(root);
    if executed {
        assert!(matches!(
            host.publish(host.revision(), 1).unwrap(),
            EndpointOutcome::Executed { .. }
        ));
        // The actual effect happened, but its receipt is now unavailable to
        // reconciliation. An expired receipt is never a nonexecution receipt.
        host.observe_time(host.revision(), ElapsedTick(1001)).unwrap();
        assert_eq!(
            host.reconcile(host.revision(), 1).unwrap(),
            Reconciliation::RetentionExpired
        );
    } else {
        assert_eq!(
            host.reconcile(host.revision(), 1).unwrap(),
            Reconciliation::AwaitingResolution
        );
    }
    assert_eq!(
        host.request_status(701).unwrap().disposition,
        FileRequestDisposition::Admitted {
            attempt: 1,
            stage: ActionState::Unknown,
        }
    );
    host
}

#[test]
fn real_executed_and_unexecuted_uncertainty_remain_charged_after_explicit_abandonment() {
    for executed in [false, true] {
        let root = Root::new();
        let host = uncertain(&root, executed);
        let before = host.inspect();
        let (actor, mut supervisor) = host.into_actor_gateway();
        let ticket = actor.submit(701, &proposal(b"ok")).unwrap();
        let (port, grant) = supervisor
            .abandonment_port(3, 1, MAX_ABANDONMENT_REPLAY_EVENTS)
            .unwrap();
        supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
        let files = root.files();
        let offered = plan(port.next(701, 401));
        assert_eq!((offered.basis().request, offered.basis().attempt), (701, 1));
        assert_eq!(offered.stage(), ActionState::Unknown);
        assert_eq!(plan(port.clone().next(701, 401)), offered);
        assert_eq!(root.files(), files);
        assert_eq!(grant.remaining_operations(), 1);
        assert!(supervisor.owner.borrow().snapshot.is_some());

        let input = offered.request().unwrap();
        let acknowledged = receipt(port.abandon(input));
        assert_eq!(acknowledged.charged_units(), 8);
        assert_eq!(acknowledged.before(), offered.basis());
        assert_eq!(grant.remaining_operations(), 0);
        let after = supervisor.host().unwrap().inspect();
        assert_eq!(after.executions, u64::from(executed));
        assert_eq!(after.executions, before.executions);
        assert_eq!(after.payload, before.payload);
        assert_eq!(after.control.ledger.available, before.control.ledger.available);
        assert_eq!(after.control.ledger.reserved, before.control.ledger.reserved);
        assert_eq!(after.control.ledger.charged, 8);
        assert_eq!(after.control.ledger.elapsed, before.control.ledger.elapsed);
        assert_eq!(after.control.ledger.epoch, before.control.ledger.epoch);
        assert_eq!(after.control.suspended, before.control.suspended);
        assert_eq!(after.stop, before.stop);
        assert!(supervisor.owner.borrow().snapshot.is_none());
        assert_eq!(
            actor.poll(&ticket),
            Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }
        );
        assert_eq!(
            actor.submit(702, &proposal(b"ok")).unwrap_err(),
            ActorError::Unavailable
        );

        // Native withdrawal removes the sendable handle, not the liability.
        {
            let mut host = supervisor.host_mut().unwrap();
            let revision = host.revision();
            assert!(host.publish(revision, 1).is_err());
            assert_eq!(host.inspect(), after);
            assert_eq!(host.abandonment_receipt(401).unwrap().charged_units(), 8);
        }
        supervisor.set_snapshot(after.revision, Some(snapshot())).unwrap();
        let files = root.files();
        assert_eq!(receipt(port.clone().abandon(input)), acknowledged);
        assert_eq!(root.files(), files);
        assert!(supervisor.owner.borrow().snapshot.is_some());
        assert_eq!(grant.remaining_operations(), 0);
        assert_eq!(
            plan(port.next(701, 402)).block_reason(),
            Some(FileAbandonmentBlockReason::IrrecoverablyUnknown)
        );
    }
}

#[test]
fn abandonment_preserves_another_request_reservation_and_its_real_publication() {
    let root = Root::new();
    let (mut host, reviewer) = create(&root);
    host.submit_request(host.revision(), 701, spec(b"ok"), snapshot()).unwrap();
    dispatch_existing(&mut host, &reviewer, 701);
    host.reconcile(host.revision(), 1).unwrap();
    host.submit_request(host.revision(), 702, spec(b"ok"), snapshot()).unwrap();
    let (action, inputs, automatic, human_request) = prepare_existing(&mut host, 702);
    let revision = host.revision();
    let human = reviewer.approve(&mut host, revision, &human_request).unwrap();
    let before = host.inspect();
    assert_eq!((before.control.ledger.charged, before.control.ledger.reserved), (8, 8));
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, _grant) = supervisor.abandonment_port(1, 1, MAX_ABANDONMENT_REPLAY_EVENTS).unwrap();
    let input = plan(port.next(701, 501)).request().unwrap();
    assert_eq!(receipt(port.abandon(input)).charged_units(), 8);
    let after = supervisor.host().unwrap().inspect();
    assert_eq!(after.control.ledger.available, before.control.ledger.available);
    assert_eq!(after.control.ledger.charged, 8);
    assert_eq!(after.control.ledger.reserved, 8);
    assert_eq!(after.control.ledger.stages[&2], ActionState::Authorized);
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.dispatch(revision, &automatic, &human, &action, &inputs, snapshot()).unwrap();
        let revision = host.revision();
        assert!(matches!(host.publish(revision, 2).unwrap(), EndpointOutcome::Executed { .. }));
        let revision = host.revision();
        assert!(matches!(host.reconcile(revision, 2).unwrap(), Reconciliation::Resolved(_)));
    }
    let after = supervisor.host().unwrap().inspect();
    assert_eq!(after.executions, 1);
    assert_eq!(after.control.ledger.charged, 16);
    assert_eq!(after.control.ledger.reserved, 0);
    assert_eq!(after.control.ledger.stages[&1], ActionState::IrrecoverablyUnknown);
    assert_eq!(after.control.ledger.stages[&2], ActionState::Confirmed);
}

#[test]
fn non_unknown_and_terminal_stages_never_acquire_abandonment_or_lose_snapshot() {
    for state in 0..6 {
        let root = Root::new();
        let (mut host, reviewer) = create(&root);
        host.submit_request(host.revision(), 701, spec(b"ok"), snapshot()).unwrap();
        match state {
            0 => {}
            1 => { let _ = prepare_existing(&mut host, 701); }
            2 => dispatch_existing(&mut host, &reviewer, 701),
            3 => {
                dispatch_existing(&mut host, &reviewer, 701);
                host.publish(host.revision(), 1).unwrap();
                host.reconcile(host.revision(), 1).unwrap();
            }
            4 => { host.cancel_request(host.revision(), 701).unwrap(); }
            5 => {
                dispatch_existing(&mut host, &reviewer, 701);
                host.reconcile(host.revision(), 1).unwrap();
                let view = host.inspect();
                host.abandon_unknown(host.revision(), FileAbandonRequest {
                    operation: 61,
                    attempt: 1,
                    expected_control_sequence: view.control.sequence,
                    expected_authority_epoch: view.control.ledger.epoch,
                }).unwrap();
            }
            _ => unreachable!(),
        }
        let before = host.inspect();
        let (_, mut supervisor) = host.into_actor_gateway();
        let (port, grant) = supervisor.abandonment_port(1, 1, MAX_ABANDONMENT_REPLAY_EVENTS).unwrap();
        supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
        let files = root.files();
        let offered = plan(port.next(701, 601));
        assert_eq!(offered.block_reason(), Some(match state {
            3 | 4 => FileAbandonmentBlockReason::Terminal,
            5 => FileAbandonmentBlockReason::IrrecoverablyUnknown,
            _ => FileAbandonmentBlockReason::NotUnknown,
        }));
        assert_eq!(port.abandon(FileAbandonmentRequest::new(601, offered.basis()).unwrap()),
            Err(FileAbandonmentError::Unsupported));
        assert_eq!(grant.remaining_operations(), 1);
        assert!(supervisor.owner.borrow().snapshot.is_some());
        assert_eq!(supervisor.host().unwrap().inspect(), before);
        assert_eq!(root.files(), files);
    }
}

#[test]
fn complete_predecessor_and_shared_retry_identity_reject_changed_or_stale_operations() {
    let root = Root::new();
    let mut host = uncertain(&root, false);
    host.submit_request(host.revision(), 702, spec(b"ok"), snapshot()).unwrap();
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.abandonment_port(2, 2, MAX_ABANDONMENT_REPLAY_EVENTS).unwrap();
    let input = plan(port.next(701, 7010)).request().unwrap();
    let original = input.expected();
    let mut variants = Vec::new();
    let mut changed = original;
    changed.scope.principal += 1;
    variants.push(changed);
    let mut changed = original;
    changed.request = 702;
    variants.push(changed);
    let mut changed = original;
    changed.attempt += 1;
    variants.push(changed);
    let mut changed = original;
    changed.request_generation += 1;
    variants.push(changed);
    let mut changed = original;
    changed.journal_revision += 1;
    variants.push(changed);
    let mut changed = original;
    changed.control_sequence += 1;
    variants.push(changed);
    let mut changed = original;
    changed.recorded_at = None;
    variants.push(changed);
    let before = supervisor.host().unwrap().inspect();
    supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
    let files = root.files();
    for basis in &variants {
        assert_eq!(port.abandon(FileAbandonmentRequest::new(7010, *basis).unwrap()),
            Err(FileAbandonmentError::StalePredecessor));
    }
    {
        let _exclusive = supervisor.owner.borrow_mut();
        unknown(port.abandon(input));
        unknown(port.next(701, 7010));
        assert!(matches!(supervisor.abandonment_port(1, 1, MAX_ABANDONMENT_REPLAY_EVENTS),
            Err(JournalError::Unavailable)));
    }
    assert_eq!(grant.remaining_operations(), 2);
    assert_eq!(grant.remaining_queries(), 1);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    assert_eq!(root.files(), files);
    let acknowledged = receipt(port.abandon(input));
    for basis in variants {
        assert_eq!(port.abandon(FileAbandonmentRequest::new(7010, basis).unwrap()),
            Err(FileAbandonmentError::IdempotencyConflict));
    }
    assert_eq!(port.abandon(FileAbandonmentRequest::new(7011, original).unwrap()),
        Err(FileAbandonmentError::StalePredecessor));
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(2)).unwrap();
    }
    let after = supervisor.host().unwrap().inspect();
    supervisor.set_snapshot(after.revision, Some(snapshot())).unwrap();
    let files = root.files();
    assert_eq!(receipt(port.clone().abandon(input)), acknowledged);
    assert_eq!(root.files(), files);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(plan(port.next(702, 7010)).block_reason(),
        Some(FileAbandonmentBlockReason::OperationAlreadyUsed));
}

#[test]
fn finite_work_limits_refuse_before_mutation_and_cached_retry_is_free() {
    let root = Root::new();
    let host = uncertain(&root, false);
    let before = host.inspect();
    let events = before.revision as usize;
    let (_, mut supervisor) = host.into_actor_gateway();
    supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
    let (short, short_grant) = supervisor.abandonment_port(1, 1, events).unwrap();
    let offered = plan(short.next(701, 801));
    assert_eq!(offered.block_reason(), Some(FileAbandonmentBlockReason::ReplayLimit));
    let input = FileAbandonmentRequest::new(801, offered.basis()).unwrap();
    let files = root.files();
    assert_eq!(short.abandon(input), Err(FileAbandonmentError::Capacity));
    assert_eq!(short.next(701, 801), Err(FileAbandonmentError::Capacity));
    assert_eq!(short_grant.remaining_operations(), 1);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    assert_eq!(root.files(), files);
    let (exact, grant) = supervisor.abandonment_port(1, 1, events + 1).unwrap();
    let acknowledged = receipt(exact.abandon(input));
    assert_eq!(grant.max_replay_events(), events + 1);
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(receipt(exact.clone().abandon(input)), acknowledged);
    assert_eq!(plan(exact.next(701, 801)).block_reason(),
        Some(FileAbandonmentBlockReason::OperationAlreadyUsed));
    assert_eq!(exact.next(701, 802), Err(FileAbandonmentError::Capacity));
}

#[test]
fn original_request_mapping_grant_custody_and_durable_operation_namespace_remain_separate() {
    let root = Root::new();
    let (mut host, reviewer) = create(&root);
    host.propose(host.revision(), 1, spec(b"ok"), snapshot()).unwrap();
    host.submit_request(host.revision(), 901, spec(b"ok"), snapshot()).unwrap();
    dispatch_existing(&mut host, &reviewer, 901);
    host.reconcile(host.revision(), 2).unwrap();
    let mut invalid = spec(b"ok");
    invalid.target.as_mut().unwrap().expected_version = 99;
    host.submit_request(host.revision(), 903, invalid, snapshot()).unwrap();
    let (actor, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.abandonment_port(5, 2, MAX_ABANDONMENT_REPLAY_EVENTS).unwrap();
    assert!(port.belongs_to(&actor));
    unknown(port.next(1, 9010));
    unknown(port.next(903, 9010));
    let input = plan(port.next(901, 9010)).request().unwrap();
    assert_eq!(input.expected().attempt, 2);
    let mut alias = input.expected();
    alias.request = 1;
    alias.attempt = 1;
    unknown(port.abandon(FileAbandonmentRequest::new(9010, alias).unwrap()));
    let other = Root::new();
    let (foreign, _other_supervisor) = owner(&other).into_actor_gateway();
    assert!(!port.belongs_to(&foreign));
    let _ = receipt(port.abandon(input));
    assert_eq!(supervisor.host().unwrap().inspect().control.ledger.stages[&1], ActionState::Reviewing);
    let mut wrong = port.clone();
    wrong.scope.branch += 1;
    assert!(!wrong.belongs_to(&actor));
    unknown(wrong.abandon(input));
    unknown(wrong.next(901, 9010));
    let (replacement, replacement_grant) = supervisor.abandonment_port(1, 1, MAX_ABANDONMENT_REPLAY_EVENTS).unwrap();
    assert_eq!(replacement.abandon(input), Err(FileAbandonmentError::IdempotencyConflict));
    assert_eq!(plan(replacement.next(901, 9010)).block_reason(),
        Some(FileAbandonmentBlockReason::OperationAlreadyUsed));
    assert_eq!(replacement_grant.remaining_operations(), 1);
    let files = root.files();
    grant.revoke();
    assert_eq!(port.abandon(input), Ok(Knowledge::Withheld { authority_required: "supervisor_abandonment" }));
    assert_eq!(port.next(901, 9010), Ok(Knowledge::Withheld { authority_required: "supervisor_abandonment" }));
    assert_eq!(root.files(), files);
    drop(supervisor);
    unknown(replacement.abandon(input));

    // Exclusive native recovery preserves the operation ID, while a new live
    // grant cannot import the old one-use cache or reapply the disposition.
    let (host, _) = FileOversight::open(root.store(), profile()).unwrap();
    let (_, reopened) = host.into_actor_gateway();
    let (port, grant) = reopened.abandonment_port(1, 1, MAX_ABANDONMENT_REPLAY_EVENTS).unwrap();
    assert_eq!(plan(port.next(901, 9010)).block_reason(),
        Some(FileAbandonmentBlockReason::OperationAlreadyUsed));
    assert_eq!(port.abandon(input), Err(FileAbandonmentError::IdempotencyConflict));
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(reopened.host().unwrap().inspect().control.ledger.charged, 8);
}

#[test]
fn every_original_storage_barrier_spends_once_without_exporting_an_unacknowledged_receipt() {
    for barrier in [
        JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
        JournalIo::Rename, JournalIo::DirectorySync,
    ] {
        let root = Root::new();
        let host = uncertain(&root, false);
        let before = host.inspect();
        let (_, mut supervisor) = host.into_actor_gateway();
        let (port, grant) = supervisor.abandonment_port(1, 1, MAX_ABANDONMENT_REPLAY_EVENTS).unwrap();
        let input = plan(port.next(701, 1001)).request().unwrap();
        supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
        supervisor.host().unwrap().fail_storage_once(barrier);
        assert_eq!(port.abandon(input), Err(FileAbandonmentError::Unavailable));
        assert_eq!(grant.remaining_operations(), 0);
        assert!(supervisor.owner.borrow().snapshot.is_none());
        assert_eq!(supervisor.host().unwrap().inspect(), before);
        unknown(port.clone().abandon(input));
        let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
        let replaced = barrier == JournalIo::DirectorySync;
        assert_eq!(disk.revision, before.revision + u64::from(replaced));
        assert_eq!(disk.control.ledger.stages[&1],
            if replaced { ActionState::IrrecoverablyUnknown } else { ActionState::Unknown });
        assert_eq!(disk.control.ledger.available, before.control.ledger.available);
        assert_eq!(disk.control.ledger.reserved, before.control.ledger.reserved);
        assert_eq!(disk.control.ledger.charged, 8);
        assert_eq!(disk.executions, before.executions);
        assert_eq!(disk.payload, before.payload);
    }
}

#[test]
fn constructor_bounds_and_interrupted_operations_never_reenter_native_work() {
    let root = Root::new();
    let host = uncertain(&root, false);
    let (_, mut supervisor) = host.into_actor_gateway();
    for args in [
        (0, 1, 1), (1, 0, 1), (1, 1, 0),
        (MAX_ABANDONMENT_QUERIES + 1, 1, 1),
        (1, MAX_ABANDONMENT_OPERATIONS + 1, 1),
        (1, 1, MAX_ABANDONMENT_REPLAY_EVENTS + 1),
    ] {
        assert!(supervisor.abandonment_port(args.0, args.1, args.2).is_err());
    }
    for args in [(1, 1, 1, 0), (1, 1, 2, 3), (1, 1, 2, usize::MAX)] {
        assert!(supervisor.abandonment_port_with_numerical_budget(args.0, args.1, args.2, args.3).is_err());
    }
    let (port, grant) = supervisor.abandonment_port(1, 1, MAX_ABANDONMENT_REPLAY_EVENTS).unwrap();
    assert_eq!(port.next(0, 1), Err(FileAbandonmentError::MalformedRequest));
    assert_eq!(port.next(701, 0), Err(FileAbandonmentError::MalformedRequest));
    let input = plan(port.next(701, 1101)).request().unwrap();
    assert_eq!(FileAbandonmentRequest::new(0, input.expected()),
        Err(FileAbandonmentError::MalformedRequest));
    // Representation of an unwind after entry: no result exists, and the
    // consumed key/work cannot start another native transaction.
    grant.grant.records.borrow_mut().insert(input.operation(), Record { request: input, result: None });
    grant.grant.operations.set(0);
    let before = supervisor.host().unwrap().inspect();
    supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
    let files = root.files();
    assert_eq!(port.abandon(input), Err(FileAbandonmentError::Unavailable));
    assert_eq!(root.files(), files);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert!(supervisor.owner.borrow().snapshot.is_some());
}

mod numerical;
