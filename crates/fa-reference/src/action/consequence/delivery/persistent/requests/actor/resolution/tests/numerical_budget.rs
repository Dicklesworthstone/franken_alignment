//! Explicit numerical reservations around the original receipt transaction.
//! These tests use real numerical histories, original congress/human approval,
//! endpoint receipts and storage failure; no caller outcome replaces them.
use super::*;
mod fixture;
use fixture::{learned_owner, raw_owner};

fn reserved_cut(host: &FileOversight) -> usize {
    usize::try_from(host.revision()).unwrap() + 1
}

#[test]
fn actual_learned_publication_reconciles_with_the_same_whole_cut_charge_warm_or_cold() {
    for cold in [false, true] {
        let root = Root::new();
        let host = learned_owner(&root, true, cold);
        let events = reserved_cut(&host);
        let (_, mut supervisor) = host.into_actor_gateway();
        let before = supervisor.host().unwrap().inspect();
        let numerical = supervisor.host().unwrap().learned_generation_inspection().unwrap();
        supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
        let image = root.files();
        let (default, default_grant) = supervisor.resolution_port(1, 1, events).unwrap();
        let blocked = plan(default.next(701, 201));
        assert_eq!(blocked.block_reason(), Some(FileResolutionBlockReason::NumericalReplayUnsupported));
        let input = FileResolutionRequest::new(201, blocked.basis()).unwrap();
        assert_eq!(default.resolve(input), Err(FileResolutionError::Unsupported));
        assert_eq!(default_grant.remaining_numerical_replay_events(), None);

        let (short, short_grant) = supervisor
            .resolution_port_with_numerical_budget(2, 1, events, events - 1).unwrap();
        let denied = plan(short.next(701, 201));
        assert_eq!(denied.block_reason(), Some(FileResolutionBlockReason::NumericalReplayLimit));
        assert_eq!(short.resolve(input), Err(FileResolutionError::Capacity));
        assert_eq!(short_grant.remaining_operations(), 1);
        assert_eq!(short_grant.remaining_numerical_replay_events(), Some(events - 1));
        assert!(supervisor.owner.borrow().snapshot.is_some());
        assert_eq!(supervisor.host().unwrap().inspect(), before);
        assert_eq!(supervisor.host().unwrap().learned_generation_inspection().unwrap(), numerical);
        assert_eq!(root.files(), image);

        let (exact, exact_grant) = supervisor
            .resolution_port_with_numerical_budget(4, 1, events, events).unwrap();
        let offered = plan(exact.next(701, 201));
        assert_eq!(plan(exact.clone().next(701, 201)), offered);
        assert_eq!(exact_grant.remaining_numerical_replay_events(), Some(events));
        assert_eq!(exact_grant.remaining_operations(), 1);
        assert!(supervisor.owner.borrow().snapshot.is_some());
        let result = receipt(exact.resolve(offered.request().unwrap()));
        assert_eq!(result.stage(), ActionState::Confirmed);
        assert!(matches!(result.outcome(), Reconciliation::Resolved(EndpointOutcome::Executed { .. })));
        assert_eq!(result.before(), offered.basis());
        assert_eq!(result.after().recorded_at, result.before().recorded_at);
        assert_eq!(exact_grant.remaining_numerical_replay_events(), Some(0));
        assert_eq!(exact_grant.remaining_operations(), 0);
        assert!(supervisor.owner.borrow().snapshot.is_none());
        let after = supervisor.host().unwrap().inspect();
        let after_numerical = supervisor.host().unwrap().learned_generation_inspection().unwrap();
        assert_eq!(after.executions, before.executions);
        assert_eq!(after.executions, 1);
        assert_eq!(after.control.ledger.charged, 8);
        assert_eq!(after.payload, before.payload);
        assert_eq!(after_numerical.numerical, numerical.numerical);
        assert_eq!(after_numerical.paused, numerical.paused);
        assert_eq!(after_numerical.pending, numerical.pending);

        // The exact cache remains historical after unrelated trusted progress,
        // and cannot consume a newly supplied one-use admission observation.
        {
            let mut host = supervisor.host_mut().unwrap();
            let revision = host.revision();
            host.observe_time(revision, ElapsedTick(2)).unwrap();
        }
        let later = supervisor.host().unwrap().inspect();
        supervisor.set_snapshot(later.revision, Some(snapshot())).unwrap();
        let image = root.files();
        assert_eq!(receipt(exact.clone().resolve(input)), result);
        assert_eq!(exact_grant.remaining_numerical_replay_events(), Some(0));
        assert_eq!(supervisor.host().unwrap().inspect(), later);
        assert!(supervisor.owner.borrow().snapshot.is_some());
        assert_eq!(root.files(), image);
    }
}

#[test]
fn raw_only_predictor_requires_explicit_budget_and_reconciles_its_real_receipt_without_reforecasting() {
    let root = Root::new();
    let host = raw_owner(&root, true);
    let events = reserved_cut(&host);
    let (_, mut supervisor) = host.into_actor_gateway();
    let before = supervisor.host().unwrap().inspect();
    let forecast = supervisor.host().unwrap().action_consistency_snapshot().unwrap();
    let observation = supervisor.host().unwrap().action_consistency_observation(1).unwrap().clone();
    supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
    let image = root.files();
    let (default, grant) = supervisor.resolution_port(1, 1, events).unwrap();
    let offered = plan(default.next(701, 211));
    assert_eq!(offered.block_reason(), Some(FileResolutionBlockReason::NumericalReplayUnsupported));
    let input = FileResolutionRequest::new(211, offered.basis()).unwrap();
    assert_eq!(default.resolve(input), Err(FileResolutionError::Unsupported));
    assert_eq!(grant.remaining_operations(), 1);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), image);
    let (explicit, grant) = supervisor
        .resolution_port_with_numerical_budget(1, 1, events, events).unwrap();
    let result = receipt(explicit.resolve(input));
    assert_eq!(result.stage(), ActionState::Confirmed);
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    let after = supervisor.host().unwrap().inspect();
    let after_forecast = supervisor.host().unwrap().action_consistency_snapshot().unwrap();
    assert_eq!(after.executions, before.executions);
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(after_forecast.evidence, forecast.evidence);
    assert_eq!(after_forecast.pending_attempt, forecast.pending_attempt);
    assert_eq!(after_forecast.coverage_lost, forecast.coverage_lost);
    assert_eq!(supervisor.host().unwrap().action_consistency_observation(1).unwrap(), &observation);
    let image = root.files();
    assert_eq!(receipt(explicit.clone().resolve(input)), result);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(root.files(), image);
}

#[test]
fn numerical_missing_and_expired_receipts_remain_unknown_charged_and_without_resend() {
    for expired in [false, true] {
        let root = Root::new();
        let mut host = learned_owner(&root, expired, true);
        if expired {
            host.observe_time(host.revision(), ElapsedTick(1001)).unwrap();
        }
        let events = reserved_cut(&host);
        let (_, supervisor) = host.into_actor_gateway();
        let before = supervisor.host().unwrap().inspect();
        let numerical = supervisor.host().unwrap().learned_generation_inspection().unwrap().numerical;
        let (port, grant) = supervisor
            .resolution_port_with_numerical_budget(1, 1, events, events).unwrap();
        let input = plan(port.next(701, 221)).request().unwrap();
        let result = receipt(port.resolve(input));
        assert_eq!(result.outcome(), if expired { Reconciliation::RetentionExpired } else { Reconciliation::AwaitingResolution });
        assert_eq!(result.stage(), ActionState::Unknown);
        let after = supervisor.host().unwrap().inspect();
        assert_eq!(after.executions, u64::from(expired));
        assert_eq!(after.executions, before.executions);
        assert_eq!(after.payload, before.payload);
        assert_eq!(after.control.ledger.charged, 8);
        assert_eq!(after.control.ledger.available, 92);
        assert_eq!(after.control.ledger.elapsed, before.control.ledger.elapsed);
        assert_eq!(supervisor.host().unwrap().learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
        let image = root.files();
        assert_eq!(receipt(port.resolve(input)), result);
        assert_eq!(root.files(), image);
    }
}

#[test]
fn cumulative_reservations_share_across_clones_and_exact_retry_precedes_exhaustion() {
    let root = Root::new();
    let host = learned_owner(&root, false, false);
    let first_events = reserved_cut(&host);
    let second_events = first_events + 1;
    let budget = first_events + second_events;
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .resolution_port_with_numerical_budget(4, 3, second_events + 1, budget).unwrap();
    let first = plan(port.next(701, 231)).request().unwrap();
    let one = receipt(port.clone().resolve(first));
    assert_eq!(one.outcome(), Reconciliation::AwaitingResolution);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(second_events));
    assert_eq!(receipt(port.resolve(first)), one);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(second_events));
    let second = plan(port.clone().next(701, 232)).request().unwrap();
    let two = receipt(port.resolve(second));
    assert_eq!(two.outcome(), Reconciliation::AwaitingResolution);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(grant.remaining_operations(), 1);
    let third = plan(port.next(701, 233));
    assert_eq!(third.block_reason(), Some(FileResolutionBlockReason::NumericalReplayLimit));
    let before = supervisor.host().unwrap().inspect();
    supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
    let image = root.files();
    assert_eq!(port.resolve(FileResolutionRequest::new(233, third.basis()).unwrap()), Err(FileResolutionError::Capacity));
    assert_eq!(receipt(port.clone().resolve(first)), one);
    assert_eq!(receipt(port.resolve(second)), two);
    assert_eq!(port.resolve(FileResolutionRequest::new(231, third.basis()).unwrap()), Err(FileResolutionError::IdempotencyConflict));
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(grant.remaining_operations(), 1);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), image);
    grant.revoke();
    assert_eq!(port.resolve(first), Ok(Knowledge::Withheld { authority_required: "supervisor_resolution" }));
    assert_eq!(root.files(), image);
}

#[test]
fn entered_numerical_storage_failure_keeps_the_full_reservation_and_suppresses_cached_retry() {
    let root = Root::new();
    let host = learned_owner(&root, true, true);
    let events = reserved_cut(&host);
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .resolution_port_with_numerical_budget(2, 2, events, events * 2).unwrap();
    let input = plan(port.next(701, 241)).request().unwrap();
    let before = supervisor.host().unwrap().inspect();
    supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
    std::fs::write(root.store().join("delivery.pending"), b"occupied native stage").unwrap();
    assert_eq!(port.resolve(input), Err(FileResolutionError::Unavailable));
    assert_eq!(grant.remaining_numerical_replay_events(), Some(events));
    assert_eq!(grant.remaining_operations(), 1);
    assert!(supervisor.owner.borrow().snapshot.is_none());
    assert!(supervisor.host().unwrap().storage_failure().is_some());
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    let image = root.files();
    unknown(port.clone().resolve(input));
    unknown(port.next(701, 242));
    assert_eq!(grant.remaining_numerical_replay_events(), Some(events));
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(root.files(), image);
    assert!(matches!(supervisor.resolution_port_with_numerical_budget(1, 1, events, events), Err(JournalError::Unavailable)));
}

#[test]
fn numerical_allowance_never_widens_per_operation_limit_or_charges_plain_owners() {
    let root = Root::new();
    let host = learned_owner(&root, true, false);
    let events = reserved_cut(&host);
    let (_, mut supervisor) = host.into_actor_gateway();
    let (limited, grant) = supervisor
        .resolution_port_with_numerical_budget(1, 2, events - 1, events).unwrap();
    let denied = plan(limited.next(701, 251));
    assert_eq!(denied.block_reason(), Some(FileResolutionBlockReason::ReplayLimit));
    let before = supervisor.host().unwrap().inspect();
    supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
    let image = root.files();
    assert_eq!(limited.resolve(FileResolutionRequest::new(251, denied.basis()).unwrap()), Err(FileResolutionError::Capacity));
    assert_eq!(grant.remaining_numerical_replay_events(), Some(events));
    assert_eq!(grant.remaining_operations(), 2);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    assert_eq!(root.files(), image);

    for args in [(0, 1, 1, 1), (1, 0, 1, 1), (1, 1, 0, 1), (1, 1, 1, 0)] {
        assert!(matches!(supervisor.resolution_port_with_numerical_budget(args.0, args.1, args.2, args.3), Err(JournalError::Contract(crate::Error::InvalidInput))));
    }
    for args in [
        (MAX_RESOLUTION_QUERIES + 1, 1, 1, 1),
        (1, MAX_RESOLUTION_OPERATIONS + 1, 1, 1),
        (1, 1, MAX_RESOLUTION_REPLAY_EVENTS + 1, 1),
        (1, 2, 3, 7),
        (1, 1, 1, usize::MAX),
    ] {
        assert!(matches!(supervisor.resolution_port_with_numerical_budget(args.0, args.1, args.2, args.3), Err(JournalError::Contract(crate::Error::Limit))));
    }
    let (_, ceiling) = supervisor.resolution_port_with_numerical_budget(1, 2, 3, 6).unwrap();
    assert_eq!(ceiling.remaining_numerical_replay_events(), Some(6));

    let ordinary_root = Root::new();
    let host = dispatched(&ordinary_root, true);
    let events = reserved_cut(&host);
    let (_, ordinary) = host.into_actor_gateway();
    let (port, grant) = ordinary.resolution_port_with_numerical_budget(1, 1, events, 1).unwrap();
    let input = plan(port.next(701, 252)).request().unwrap();
    assert_eq!(receipt(port.resolve(input)).stage(), ActionState::Confirmed);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(1));
    assert_eq!(grant.remaining_operations(), 0);
}
