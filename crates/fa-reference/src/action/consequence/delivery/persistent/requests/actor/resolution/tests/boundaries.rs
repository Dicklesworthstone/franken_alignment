use super::*;
use crate::Error;
use crate::action::consequence::delivery::StopRequest;
use crate::action::consequence::delivery::persistent::FileAbandonRequest;

#[test]
fn full_predecessors_and_original_operation_keys_are_checked_before_mutation() {
    let root = Root::new();
    let mut host = dispatched(&root, false);
    host.submit_request(host.revision(), 702, spec(b"ok"), snapshot()).unwrap();
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.resolution_port(2, 2, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
    let offered = plan(port.next(701, 77));
    let input = offered.request().unwrap();
    let original = offered.basis();
    let mut changes = Vec::new();
    let mut changed = original;
    changed.scope.principal += 1;
    changes.push(changed);
    let mut changed = original;
    changed.request = 702;
    changes.push(changed);
    let mut changed = original;
    changed.attempt += 1;
    changes.push(changed);
    let mut changed = original;
    changed.request_generation += 1;
    changes.push(changed);
    let mut changed = original;
    changed.journal_revision += 1;
    changes.push(changed);
    let mut changed = original;
    changed.control_sequence += 1;
    changes.push(changed);
    let mut changed = original;
    changed.recorded_at = None;
    changes.push(changed);

    let inspection = supervisor.host().unwrap().inspect();
    supervisor.set_snapshot(inspection.revision, Some(snapshot())).unwrap();
    let image = root.files();
    for basis in &changes {
        assert_eq!(
            port.resolve(FileResolutionRequest::new(77, *basis).unwrap()),
            Err(FileResolutionError::StalePredecessor)
        );
    }
    {
        let _exclusive = supervisor.owner.borrow_mut();
        unknown(port.resolve(input));
        unknown(port.next(701, 77));
        assert!(matches!(
            supervisor.resolution_port(1, 1, MAX_RESOLUTION_REPLAY_EVENTS),
            Err(JournalError::Unavailable)
        ));
    }
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(grant.remaining_operations(), 2);
    assert_eq!(supervisor.host().unwrap().inspect(), inspection);
    assert_eq!(root.files(), image);
    assert!(supervisor.owner.borrow().snapshot.is_some());

    let resolved = receipt(port.resolve(input));
    assert!(supervisor.owner.borrow().snapshot.is_none());
    let inspection = supervisor.host().unwrap().inspect();
    let image = root.files();
    for basis in changes {
        assert_eq!(
            port.clone().resolve(FileResolutionRequest::new(77, basis).unwrap()),
            Err(FileResolutionError::IdempotencyConflict)
        );
    }
    assert_eq!(
        port.resolve(FileResolutionRequest::new(78, original).unwrap()),
        Err(FileResolutionError::StalePredecessor)
    );
    assert_eq!(receipt(port.resolve(input)), resolved);
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(supervisor.host().unwrap().inspect(), inspection);
    assert_eq!(root.files(), image);
}

#[test]
fn pure_next_preserves_missing_source_and_a_real_unused_admission_snapshot() {
    let root = Root::new();
    let (host, source) = source_owner(&root);
    let (actor, mut supervisor) = host.into_actor_gateway();
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
    actor.submit(701, &proposal(b"ok")).unwrap();
    let before = supervisor.host().unwrap().inspect();
    supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
    let reads = source.status().read_attempts;
    std::fs::remove_file(root.source()).unwrap();
    let image = root.files();
    let (port, grant) = supervisor.resolution_port(2, 1, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
    let offered = plan(port.next(701, 91));
    assert_eq!(offered.stage(), ActionState::Reviewing);
    assert_eq!(offered.basis().request_generation, 0);
    assert_eq!(offered.block_reason(), Some(FileResolutionBlockReason::NotDispatched));
    unknown(port.clone().next(999, 91));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(port.next(701, 91), Err(FileResolutionError::Capacity));
    assert_eq!(source.status().read_attempts, reads);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), image);
    for secret in ["unselected private source", "private helper input", "approve?"] {
        assert!(!format!("{offered:?}").contains(secret));
    }
    actor.submit(702, &proposal(b"ok")).unwrap();
    assert!(matches!(
        supervisor.host().unwrap().request_status(702).unwrap().disposition,
        FileRequestDisposition::Admitted { attempt: 2, .. }
    ));
    assert_eq!(source.status().read_attempts, reads);
}

#[test]
fn independent_finite_budgets_share_across_clones_without_reserving_next_keys() {
    let root = Root::new();
    let host = dispatched(&root, false);
    let (_, supervisor) = host.into_actor_gateway();
    for (queries, operations, events) in [(0, 1, 1), (1, 0, 1), (1, 1, 0)] {
        assert!(matches!(
            supervisor.resolution_port(queries, operations, events),
            Err(JournalError::Contract(Error::InvalidInput))
        ));
    }
    for (queries, operations, events) in [
        (MAX_RESOLUTION_QUERIES + 1, 1, 1),
        (1, MAX_RESOLUTION_OPERATIONS + 1, 1),
        (1, 1, MAX_RESOLUTION_REPLAY_EVENTS + 1),
    ] {
        assert!(matches!(
            supervisor.resolution_port(queries, operations, events),
            Err(JournalError::Contract(Error::Limit))
        ));
    }
    let (port, grant) = supervisor.resolution_port(3, 1, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
    assert_eq!(port.next(0, 91), Err(FileResolutionError::MalformedRequest));
    assert_eq!(port.next(701, 0), Err(FileResolutionError::MalformedRequest));
    assert_eq!(grant.remaining_queries(), 3);
    let first = plan(port.next(701, 91));
    assert_eq!(plan(port.clone().next(701, 91)), first);
    assert_eq!(grant.remaining_operations(), 1);
    assert!(FileResolutionRequest::new(0, first.basis()).is_err());
    let input = first.request().unwrap();
    let resolved = receipt(port.clone().resolve(input));
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(grant.remaining_operations(), 0);
    let next = plan(port.next(701, 92));
    assert_eq!(next.block_reason(), Some(FileResolutionBlockReason::OperationLimit));
    assert_eq!(
        port.resolve(FileResolutionRequest::new(92, next.basis()).unwrap()),
        Err(FileResolutionError::Capacity)
    );
    let before = supervisor.host().unwrap().inspect();
    let image = root.files();
    assert_eq!(receipt(port.resolve(input)), resolved);
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), image);
}

#[test]
fn replay_allowance_includes_the_new_record_and_never_consumes_a_refused_snapshot() {
    let root = Root::new();
    let host = dispatched(&root, false);
    let (_, mut supervisor) = host.into_actor_gateway();
    let before = supervisor.host().unwrap().inspect();
    let events = usize::try_from(before.revision).unwrap();
    supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
    let (limited, grant) = supervisor.resolution_port(1, 1, events).unwrap();
    let offered = plan(limited.next(701, 101));
    assert_eq!(offered.block_reason(), Some(FileResolutionBlockReason::ReplayLimit));
    let image = root.files();
    assert_eq!(
        limited.resolve(FileResolutionRequest::new(101, offered.basis()).unwrap()),
        Err(FileResolutionError::Capacity)
    );
    assert_eq!(grant.remaining_operations(), 1);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), image);

    let (exact, grant) = supervisor.resolution_port(1, 1, events + 1).unwrap();
    let offered = plan(exact.next(701, 101));
    let resolved = receipt(exact.resolve(offered.request().unwrap()));
    assert_eq!(resolved.after().journal_revision, (events + 1) as u64);
    assert_eq!(grant.max_replay_events(), events + 1);
    assert_eq!(grant.remaining_operations(), 0);
    assert!(supervisor.owner.borrow().snapshot.is_none());
}

#[test]
fn scope_allocation_and_revocation_control_even_cached_acknowledgments() {
    let root = Root::new();
    let host = dispatched(&root, true);
    let (actor, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.resolution_port(3, 1, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
    assert!(port.belongs_to(&actor));
    let offered = plan(port.next(701, 111));
    let input = offered.request().unwrap();
    let _resolved = receipt(port.resolve(input));
    let mut wrong = port.clone();
    wrong.scope.branch += 1;
    assert!(!wrong.belongs_to(&actor));
    unknown(wrong.next(701, 111));
    unknown(wrong.resolve(input));
    let other = Root::new();
    let (foreign, _other_supervisor) = owner(&other).into_actor_gateway();
    assert!(!port.belongs_to(&foreign));
    assert_eq!(grant.remaining_queries(), 1);
    let image = root.files();
    grant.revoke();
    assert_eq!(
        port.clone().resolve(input),
        Ok(Knowledge::Withheld { authority_required: "supervisor_resolution" })
    );
    assert_eq!(
        port.next(701, 112),
        Ok(Knowledge::Withheld { authority_required: "supervisor_resolution" })
    );
    assert_eq!(root.files(), image);
}

#[test]
fn cached_results_do_not_adopt_a_replacement_grant_or_reopened_owner() {
    let root = Root::new();
    let host = dispatched(&root, true);
    let (actor, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.resolution_port(1, 1, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
    let input = plan(port.next(701, 121)).request().unwrap();
    let _resolved = receipt(port.resolve(input));
    let (replacement, _replacement_grant) =
        supervisor.resolution_port(1, 1, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
    assert_eq!(replacement.resolve(input), Err(FileResolutionError::StalePredecessor));
    drop(supervisor);
    assert!(port.owner.upgrade().is_none());
    unknown(port.resolve(input));
    assert_eq!(grant.remaining_operations(), 0);
    let (reopened, _reviewer) = FileOversight::open(root.store(), profile()).unwrap();
    let (new_actor, new_supervisor) = reopened.into_actor_gateway();
    assert!(port.belongs_to(&actor));
    assert!(!port.belongs_to(&new_actor));
    let (new_port, _new_grant) =
        new_supervisor.resolution_port(1, 1, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
    let image = root.files();
    assert_eq!(new_port.resolve(input), Err(FileResolutionError::StalePredecessor));
    unknown(port.resolve(input));
    assert_eq!(root.files(), image);
}

#[test]
fn a_reopened_historical_tick_cannot_replace_a_new_supervisor_time_observation() {
    let root = Root::new();
    let host = dispatched(&root, true);
    drop(host);
    let (reopened, _reviewer) = FileOversight::open(root.store(), profile()).unwrap();
    assert!(!reopened.clock_ready());
    assert_eq!(reopened.inspect().control.ledger.elapsed, Some(ElapsedTick(1)));
    let (_, mut supervisor) = reopened.into_actor_gateway();
    let (port, grant) = supervisor.resolution_port(2, 1, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
    let before = supervisor.host().unwrap().inspect();
    let image = root.files();
    let blocked = plan(port.next(701, 131));
    assert_eq!(blocked.stage(), ActionState::Unknown);
    assert_eq!(blocked.block_reason(), Some(FileResolutionBlockReason::ClockUnavailable));
    assert_eq!(
        port.resolve(FileResolutionRequest::new(131, blocked.basis()).unwrap()),
        Err(FileResolutionError::Unavailable)
    );
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), image);
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(2)).unwrap();
    }
    let offered = plan(port.next(701, 131));
    let resolved = receipt(port.resolve(offered.request().unwrap()));
    assert_eq!(resolved.stage(), ActionState::Confirmed);
    assert_eq!(resolved.before().recorded_at, Some(ElapsedTick(2)));
    assert_eq!(resolved.after().recorded_at, Some(ElapsedTick(2)));
    assert_eq!(supervisor.host().unwrap().inspect().executions, 1);
}

#[test]
fn native_write_failure_spends_once_invalidates_snapshot_and_emits_no_receipt() {
    let root = Root::new();
    let host = dispatched(&root, true);
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.resolution_port(1, 1, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
    let offered = plan(port.next(701, 141));
    let input = offered.request().unwrap();
    let before = supervisor.host().unwrap().inspect();
    supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
    std::fs::write(root.store().join("delivery.pending"), b"occupied stage").unwrap();
    assert_eq!(port.resolve(input), Err(FileResolutionError::Unavailable));
    assert_eq!(grant.remaining_operations(), 0);
    assert!(supervisor.host().unwrap().storage_failure().is_some());
    assert!(supervisor.owner.borrow().snapshot.is_none());
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    let image = root.files();
    unknown(port.clone().resolve(input));
    unknown(port.next(701, 142));
    assert!(matches!(
        supervisor.resolution_port(1, 1, MAX_RESOLUTION_REPLAY_EVENTS),
        Err(JournalError::Unavailable)
    ));
    assert_eq!(root.files(), image);
}

#[test]
fn a_later_owner_fault_withholds_an_earlier_cached_success() {
    let root = Root::new();
    let host = dispatched(&root, false);
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.resolution_port(2, 2, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
    let input = plan(port.next(701, 151)).request().unwrap();
    let resolved = receipt(port.resolve(input));
    assert_eq!(resolved.stage(), ActionState::Unknown);
    std::fs::write(root.store().join("delivery.pending"), b"occupied stage").unwrap();
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        assert!(host.observe_time(revision, ElapsedTick(2)).is_err());
    }
    let before = supervisor.host().unwrap().inspect();
    let image = root.files();
    unknown(port.resolve(input));
    unknown(port.next(701, 152));
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), image);
}

#[test]
fn irrecoverable_disposition_never_offers_reconciliation_or_releases_its_charge() {
    let root = Root::new();
    let mut host = dispatched(&root, false);
    let inspection = host.inspect();
    host.request_stop(
        inspection.revision,
        StopRequest {
            operation: 161,
            expected_control_sequence: inspection.control.sequence,
            expected_authority_epoch: inspection.control.ledger.epoch,
        },
    ).unwrap();
    let inspection = host.inspect();
    host.abandon_unknown(
        inspection.revision,
        FileAbandonRequest {
            operation: 162,
            attempt: 1,
            expected_control_sequence: inspection.control.sequence,
            expected_authority_epoch: inspection.control.ledger.epoch,
        },
    ).unwrap();
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.resolution_port(1, 1, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
    let before = supervisor.host().unwrap().inspect();
    let image = root.files();
    let offered = plan(port.next(701, 163));
    assert_eq!(offered.stage(), ActionState::IrrecoverablyUnknown);
    assert_eq!(offered.block_reason(), Some(FileResolutionBlockReason::IrrecoverablyUnknown));
    assert_eq!(
        port.resolve(FileResolutionRequest::new(163, offered.basis()).unwrap()),
        Err(FileResolutionError::Unsupported)
    );
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(before.control.ledger.charged, 8);
    assert_eq!(before.executions, 0);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), image);
}
