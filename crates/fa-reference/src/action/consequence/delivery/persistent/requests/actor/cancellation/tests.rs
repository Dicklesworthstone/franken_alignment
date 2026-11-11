//! Actual original requests and two-key authority exercise Cancel, never Hold.
use super::super::{FileRequestDisposition, JournalError};
use super::*;
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::EndpointOutcome;
use crate::action::consequence::delivery::persistent::{
    FileAbandonRequest, JournalIo, Reconciliation,
};
use crate::action::consequence::oversight::actor::{ActorError, ActorOutcome, UnknownReason};

#[path = "../hold_diagnosis/tests/fixture.rs"]
mod fixture;
use fixture::*;

fn plan(result: FileCancellationPlanResult) -> FileCancellationPlan {
    let Ok(Knowledge::Known { value, basis }) = result else {
        panic!("expected original cancellation plan: {result:?}");
    };
    assert_eq!(value.basis(), basis);
    assert_eq!(value.can_cancel(), value.request().is_some());
    assert_eq!(value.can_cancel(), value.block_reason().is_none());
    if let Some(input) = value.request() {
        assert_eq!(input.expected(), basis);
        assert_eq!(input.operation(), value.operation());
    }
    value
}

fn receipt(result: FileCancellationResult) -> FileCancellationReceipt {
    let Ok(Knowledge::Known { value, basis }) = result else {
        panic!("expected original cancellation acknowledgment: {result:?}");
    };
    assert_eq!(value.after(), basis);
    assert_eq!(value.before().scope, basis.scope);
    assert_eq!(value.before().request, basis.request);
    assert_eq!(value.before().attempt, basis.attempt);
    assert_eq!(value.before().recorded_at, basis.recorded_at);
    assert_eq!(value.before().journal_revision + 1, basis.journal_revision);
    assert_eq!(value.stage(), ActionState::Cancelled);
    value
}

fn unknown<T: fmt::Debug + PartialEq>(
    result: Result<Knowledge<T, FileHoldDiagnosisBasis>, FileCancellationError>,
) {
    assert_eq!(
        result,
        Ok(Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable
        })
    );
}

fn pending(root: &Root) -> FileOversight {
    let mut host = owner(root);
    host.submit_request(host.revision(), 701, spec(b"ok"), snapshot())
        .unwrap();
    host
}

#[test]
fn authorized_cancellation_preserves_other_reserved_work_and_allows_its_real_publication() {
    let root = Root::new();
    let (mut host, reviewer) = create(&root);
    host.submit_request(host.revision(), 701, spec(b"ok"), snapshot())
        .unwrap();
    let (first_action, first_inputs, first_automatic, first_request) =
        prepare_existing(&mut host, 701);
    // Human custody remains separate from the new cancellation grant.
    let revision = host.revision();
    let first_human = reviewer
        .approve(&mut host, revision, &first_request)
        .unwrap();
    host.submit_request(host.revision(), 702, spec(b"ok"), snapshot())
        .unwrap();
    let (second_action, second_inputs, second_automatic, second_request) =
        prepare_existing(&mut host, 702);
    let revision = host.revision();
    let second_human = reviewer
        .approve(&mut host, revision, &second_request)
        .unwrap();
    let before = host.inspect();
    assert_eq!(before.control.ledger.reserved, 16);
    let (actor, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .cancellation_port(3, 1, MAX_CANCELLATION_REPLAY_EVENTS)
        .unwrap();
    let offered = plan(port.next(701, 401));
    assert_eq!(offered.stage(), ActionState::Authorized);
    assert_eq!((offered.basis().request, offered.basis().attempt), (701, 1));
    supervisor
        .set_snapshot(before.revision, Some(snapshot()))
        .unwrap();
    let files = root.files();
    assert_eq!(plan(port.clone().next(701, 401)), offered);
    assert_eq!(root.files(), files);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    let input = offered.request().unwrap();
    let cancelled = receipt(port.cancel(input));
    assert_eq!(cancelled.before_stage(), ActionState::Authorized);
    assert_eq!(cancelled.refunded_units(), 8);
    assert_eq!(grant.remaining_operations(), 0);
    let after = supervisor.host().unwrap().inspect();
    assert_eq!(
        after.control.ledger.available - before.control.ledger.available,
        8
    );
    assert_eq!(
        before.control.ledger.reserved - after.control.ledger.reserved,
        8
    );
    assert_eq!(after.control.ledger.reserved, 8);
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(after.control.ledger.stages[&2], ActionState::Authorized);
    assert_eq!(after.control.ledger.elapsed, before.control.ledger.elapsed);
    assert_eq!(after.executions, 0);
    assert!(after.stop.is_none() && !after.control.suspended);
    assert!(supervisor.owner.borrow().snapshot.is_none());
    assert_eq!(
        actor.submit(703, &proposal(b"ok")).unwrap_err(),
        ActorError::Unavailable
    );
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        assert!(
            host.dispatch(
                revision,
                &first_automatic,
                &first_human,
                &first_action,
                &first_inputs,
                snapshot()
            )
            .is_err()
        );
        let revision = host.revision();
        host.dispatch(
            revision,
            &second_automatic,
            &second_human,
            &second_action,
            &second_inputs,
            snapshot(),
        )
        .unwrap();
        let revision = host.revision();
        assert!(matches!(
            host.publish(revision, 2).unwrap(),
            EndpointOutcome::Executed { .. }
        ));
        let revision = host.revision();
        assert!(matches!(
            host.reconcile(revision, 2).unwrap(),
            Reconciliation::Resolved(_)
        ));
    }
    let later = supervisor.host().unwrap().inspect();
    assert_eq!(later.executions, 1);
    assert_eq!(later.control.ledger.charged, 8);
    supervisor
        .set_snapshot(later.revision, Some(snapshot()))
        .unwrap();
    let files = root.files();
    assert_eq!(receipt(port.clone().cancel(input)), cancelled);
    assert_eq!(supervisor.host().unwrap().inspect(), later);
    assert_eq!(root.files(), files);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    assert_eq!(
        plan(port.next(701, 402)).block_reason(),
        Some(FileCancellationBlockReason::Terminal)
    );
    let mut proposal = proposal(b"ok");
    proposal.target = later.target;
    actor.submit(703, &proposal).unwrap();
}

#[test]
fn reviewing_cancellation_is_source_free_and_retires_only_its_original_request() {
    let root = Root::new();
    let (host, source) = source_owner(&root);
    let (actor, mut supervisor) = host.into_actor_gateway();
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
    let ticket = actor.submit(701, &proposal(b"ok")).unwrap();
    let before = supervisor.host().unwrap().inspect();
    let source_before = source.status();
    let (port, grant) = supervisor
        .cancellation_port(2, 1, MAX_CANCELLATION_REPLAY_EVENTS)
        .unwrap();
    let offered = plan(port.next(701, 501));
    assert_eq!(offered.stage(), ActionState::Reviewing);
    let cancelled = receipt(port.cancel(offered.request().unwrap()));
    assert_eq!(cancelled.refunded_units(), 0);
    assert_eq!(source.status(), source_before);
    let after = supervisor.host().unwrap().inspect();
    assert_eq!(
        after.control.ledger.available,
        before.control.ledger.available
    );
    assert_eq!(after.control.ledger.charged, 0);
    assert_eq!(after.control.ledger.elapsed, before.control.ledger.elapsed);
    assert_eq!(after.executions, 0);
    assert_eq!(after.payload, before.payload);
    assert!(matches!(
        actor.poll(&ticket),
        Knowledge::Known {
            value: ActorOutcome::CancelledBeforeDispatch,
            ..
        }
    ));
    assert_eq!(grant.remaining_operations(), 0);
}

#[test]
fn dispatched_unknown_and_abandoned_effects_refuse_without_releasing_any_charge() {
    for state in 0..4 {
        let root = Root::new();
        let (mut host, reviewer) = create(&root);
        host.submit_request(host.revision(), 701, spec(b"ok"), snapshot())
            .unwrap();
        dispatch_existing(&mut host, &reviewer, 701);
        if state == 3 {
            host.publish(host.revision(), 1).unwrap();
        }
        if state != 0 {
            host.reconcile(host.revision(), 1).unwrap();
        }
        if state == 2 {
            let view = host.inspect();
            host.abandon_unknown(
                host.revision(),
                FileAbandonRequest {
                    operation: 61,
                    attempt: 1,
                    expected_control_sequence: view.control.sequence,
                    expected_authority_epoch: view.control.ledger.epoch,
                },
            )
            .unwrap();
        }
        let before = host.inspect();
        assert_eq!(before.control.ledger.charged, 8);
        let (_, mut supervisor) = host.into_actor_gateway();
        let (port, grant) = supervisor
            .cancellation_port(1, 1, MAX_CANCELLATION_REPLAY_EVENTS)
            .unwrap();
        supervisor
            .set_snapshot(before.revision, Some(snapshot()))
            .unwrap();
        let files = root.files();
        let offered = plan(port.next(701, 601));
        assert_eq!(
            offered.block_reason(),
            Some(match state {
                2 => FileCancellationBlockReason::IrrecoverablyUnknown,
                3 => FileCancellationBlockReason::Terminal,
                _ => FileCancellationBlockReason::AlreadyDispatched,
            })
        );
        assert_eq!(
            port.cancel(FileCancellationRequest::new(601, offered.basis()).unwrap()),
            Err(FileCancellationError::Unsupported)
        );
        assert_eq!(grant.remaining_operations(), 1);
        assert!(supervisor.owner.borrow().snapshot.is_some());
        assert_eq!(supervisor.host().unwrap().inspect(), before);
        assert_eq!(root.files(), files);
    }
}

#[test]
fn exact_predecessor_and_retry_identity_prevent_recancellation_at_another_cut() {
    let root = Root::new();
    let mut host = pending(&root);
    host.submit_request(host.revision(), 702, spec(b"ok"), snapshot())
        .unwrap();
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .cancellation_port(2, 2, MAX_CANCELLATION_REPLAY_EVENTS)
        .unwrap();
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
    supervisor
        .set_snapshot(before.revision, Some(snapshot()))
        .unwrap();
    let files = root.files();
    for basis in &variants {
        assert_eq!(
            port.cancel(FileCancellationRequest::new(7010, *basis).unwrap()),
            Err(FileCancellationError::StalePredecessor)
        );
    }
    {
        let _exclusive = supervisor.owner.borrow_mut();
        unknown(port.cancel(input));
        unknown(port.next(701, 7010));
        assert!(matches!(
            supervisor.cancellation_port(1, 1, MAX_CANCELLATION_REPLAY_EVENTS),
            Err(JournalError::Unavailable)
        ));
    }
    assert_eq!(grant.remaining_operations(), 2);
    assert_eq!(grant.remaining_queries(), 1);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    assert_eq!(root.files(), files);
    let cancelled = receipt(port.cancel(input));
    for basis in variants {
        assert_eq!(
            port.cancel(FileCancellationRequest::new(7010, basis).unwrap()),
            Err(FileCancellationError::IdempotencyConflict)
        );
    }
    assert_eq!(
        port.cancel(FileCancellationRequest::new(7011, original).unwrap()),
        Err(FileCancellationError::StalePredecessor)
    );
    let after = supervisor.host().unwrap().inspect();
    supervisor
        .set_snapshot(after.revision, Some(snapshot()))
        .unwrap();
    assert_eq!(receipt(port.clone().cancel(input)), cancelled);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(
        plan(port.next(702, 7010)).block_reason(),
        Some(FileCancellationBlockReason::OperationAlreadyUsed)
    );
}

#[test]
fn bounded_plans_operations_and_replay_preserve_refused_snapshots() {
    let root = Root::new();
    let mut host = pending(&root);
    host.submit_request(host.revision(), 702, spec(b"ok"), snapshot())
        .unwrap();
    let (_, mut supervisor) = host.into_actor_gateway();
    let before = supervisor.host().unwrap().inspect();
    let events = before.revision as usize;
    supervisor
        .set_snapshot(before.revision, Some(snapshot()))
        .unwrap();
    let (short, short_grant) = supervisor.cancellation_port(1, 1, events).unwrap();
    let offered = plan(short.next(701, 801));
    assert_eq!(
        offered.block_reason(),
        Some(FileCancellationBlockReason::ReplayLimit)
    );
    let input = FileCancellationRequest::new(801, offered.basis()).unwrap();
    let files = root.files();
    assert_eq!(short.cancel(input), Err(FileCancellationError::Capacity));
    assert_eq!(short.next(701, 802), Err(FileCancellationError::Capacity));
    assert_eq!(short_grant.remaining_operations(), 1);
    assert_eq!(root.files(), files);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    let (exact, grant) = supervisor.cancellation_port(2, 1, events + 1).unwrap();
    let cancelled = receipt(exact.cancel(input));
    assert_eq!(grant.max_replay_events(), events + 1);
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(receipt(exact.clone().cancel(input)), cancelled);
    let blocked = plan(exact.next(702, 802));
    assert_eq!(
        blocked.block_reason(),
        Some(FileCancellationBlockReason::ReplayLimit)
    );

    let (only_one, one_grant) = supervisor
        .cancellation_port(1, 1, MAX_CANCELLATION_REPLAY_EVENTS)
        .unwrap();
    let input = plan(only_one.next(702, 802)).request().unwrap();
    let _ = receipt(only_one.cancel(input));
    assert_eq!(one_grant.remaining_queries(), 0);
    assert_eq!(one_grant.remaining_operations(), 0);
    assert_eq!(
        only_one.next(702, 802),
        Err(FileCancellationError::Capacity)
    );
    assert_eq!(
        receipt(only_one.cancel(input)).stage(),
        ActionState::Cancelled
    );
}

#[test]
fn grant_custody_owner_identity_and_mapping_never_come_from_an_observation() {
    let root = Root::new();
    let mut host = owner(&root);
    host.propose(host.revision(), 1, spec(b"ok"), snapshot())
        .unwrap();
    host.submit_request(host.revision(), 901, spec(b"ok"), snapshot())
        .unwrap();
    let mut invalid = spec(b"ok");
    invalid.target.as_mut().unwrap().expected_version = 99;
    host.submit_request(host.revision(), 903, invalid, snapshot())
        .unwrap();
    let (actor, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .cancellation_port(5, 2, MAX_CANCELLATION_REPLAY_EVENTS)
        .unwrap();
    assert!(port.belongs_to(&actor));
    unknown(port.next(1, 9010));
    unknown(port.next(903, 9010));
    let input = plan(port.next(901, 9010)).request().unwrap();
    assert_eq!(input.expected().attempt, 2);
    let mut alias = input.expected();
    alias.request = 1;
    alias.attempt = 1;
    unknown(port.cancel(FileCancellationRequest::new(9010, alias).unwrap()));
    let other = Root::new();
    let (foreign, _other_supervisor) = owner(&other).into_actor_gateway();
    assert!(!port.belongs_to(&foreign));
    let _ = receipt(port.cancel(input));
    assert_eq!(
        supervisor.host().unwrap().inspect().control.ledger.stages[&1],
        ActionState::Reviewing
    );
    let mut wrong = port.clone();
    wrong.scope.branch += 1;
    assert!(!wrong.belongs_to(&actor));
    unknown(wrong.cancel(input));
    unknown(wrong.next(901, 9010));
    let (replacement, _replacement_grant) = supervisor
        .cancellation_port(1, 1, MAX_CANCELLATION_REPLAY_EVENTS)
        .unwrap();
    assert_eq!(
        replacement.cancel(input),
        Err(FileCancellationError::StalePredecessor)
    );
    let files = root.files();
    grant.revoke();
    assert_eq!(
        port.cancel(input),
        Ok(Knowledge::Withheld {
            authority_required: "supervisor_cancellation"
        })
    );
    assert_eq!(
        port.next(901, 9010),
        Ok(Knowledge::Withheld {
            authority_required: "supervisor_cancellation"
        })
    );
    assert_eq!(root.files(), files);
    drop(supervisor);
    unknown(replacement.cancel(input));
}

#[test]
fn all_original_storage_barriers_charge_once_and_never_export_a_candidate_refund() {
    for barrier in [
        JournalIo::Stage,
        JournalIo::Write,
        JournalIo::FileSync,
        JournalIo::Rename,
        JournalIo::DirectorySync,
    ] {
        let root = Root::new();
        let (mut host, _) = create(&root);
        host.submit_request(host.revision(), 701, spec(b"ok"), snapshot())
            .unwrap();
        let _ = prepare_existing(&mut host, 701);
        let before = host.inspect();
        let (_, mut supervisor) = host.into_actor_gateway();
        let (port, grant) = supervisor
            .cancellation_port(1, 1, MAX_CANCELLATION_REPLAY_EVENTS)
            .unwrap();
        let input = plan(port.next(701, 1001)).request().unwrap();
        supervisor
            .set_snapshot(before.revision, Some(snapshot()))
            .unwrap();
        supervisor.host().unwrap().fail_storage_once(barrier);
        assert_eq!(port.cancel(input), Err(FileCancellationError::Unavailable));
        assert_eq!(grant.remaining_operations(), 0);
        assert!(supervisor.owner.borrow().snapshot.is_none());
        assert_eq!(supervisor.host().unwrap().inspect(), before);
        unknown(port.cancel(input));
        let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
        let replaced = barrier == JournalIo::DirectorySync;
        assert_eq!(disk.revision, before.revision + u64::from(replaced));
        assert_eq!(disk.control.ledger.reserved, if replaced { 0 } else { 8 });
        assert_eq!(
            disk.control.ledger.available,
            if replaced { 100 } else { 92 }
        );
        assert_eq!(disk.control.ledger.charged, 0);
        assert_eq!(disk.executions, 0);
    }
}

#[test]
fn constructor_bounds_and_interrupted_operations_never_reenter_native_work() {
    let root = Root::new();
    let host = pending(&root);
    let (_, mut supervisor) = host.into_actor_gateway();
    for args in [
        (0, 1, 1),
        (1, 0, 1),
        (1, 1, 0),
        (MAX_CANCELLATION_QUERIES + 1, 1, 1),
        (1, MAX_CANCELLATION_OPERATIONS + 1, 1),
        (1, 1, MAX_CANCELLATION_REPLAY_EVENTS + 1),
    ] {
        assert!(
            supervisor
                .cancellation_port(args.0, args.1, args.2)
                .is_err()
        );
    }
    for args in [(1, 1, 1, 0), (1, 1, 2, 3), (1, 1, 2, usize::MAX)] {
        assert!(
            supervisor
                .cancellation_port_with_numerical_budget(args.0, args.1, args.2, args.3)
                .is_err()
        );
    }
    let (port, grant) = supervisor
        .cancellation_port(1, 1, MAX_CANCELLATION_REPLAY_EVENTS)
        .unwrap();
    assert_eq!(
        port.next(0, 1),
        Err(FileCancellationError::MalformedRequest)
    );
    assert_eq!(
        port.next(701, 0),
        Err(FileCancellationError::MalformedRequest)
    );
    let input = plan(port.next(701, 1101)).request().unwrap();
    assert_eq!(
        FileCancellationRequest::new(0, input.expected()),
        Err(FileCancellationError::MalformedRequest)
    );
    // Exact representation of a caught unwind after entry: the key is retained,
    // its work spent, and no result exists. Retrying must never execute Cancel.
    grant.grant.records.borrow_mut().insert(
        input.operation(),
        Record {
            request: input,
            result: None,
        },
    );
    grant.grant.operations.set(0);
    let before = supervisor.host().unwrap().inspect();
    supervisor
        .set_snapshot(before.revision, Some(snapshot()))
        .unwrap();
    let files = root.files();
    assert_eq!(port.cancel(input), Err(FileCancellationError::Unavailable));
    assert_eq!(root.files(), files);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert!(supervisor.owner.borrow().snapshot.is_some());
}

mod numerical;
