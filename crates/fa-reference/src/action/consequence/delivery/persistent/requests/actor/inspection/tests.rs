//! Original native captures, real ledger histories and finite shared grants.
use super::*;
use crate::action::consequence::delivery::persistent::observed::replay::situation::FileSituationOrigin;
use crate::action::{ActionState, ElapsedTick};

mod fixture;
use fixture::*;

fn known(result: FileInspectionResult) -> FileInspectionPage {
    match result.unwrap() {
        Knowledge::Known { value, basis } => {
            assert_eq!(value.basis(), basis);
            assert_eq!(basis.scope, profile().delivery.scope);
            assert_eq!(basis.origin, FileSituationOrigin::AcknowledgedOwner);
            value
        }
        other => panic!("expected original native situation: {other:?}"),
    }
}

fn unknown(result: FileInspectionResult) {
    assert_eq!(
        result,
        Ok(Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        })
    );
}

#[test]
fn three_pages_enumerate_original_priority_order_with_full_cut_balances_and_no_mutations() {
    let root = Root::new();
    let (mut host, reviewer) = create(&root);
    prepared(&mut host, 1);
    dispatch(&mut host, &reviewer, 2);
    propose(&mut host, 3, false);
    for id in 4..=33 {
        propose(&mut host, id, true);
    }
    let (_actor, mut supervisor) = host.into_actor_gateway();
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
    let before = supervisor.host().unwrap().inspect();
    let image = root.files();
    let (port, grant) = supervisor.inspection_port(4).unwrap();
    let first = known(port.situation());
    let first_cursor = first.continuation().unwrap().clone();
    let second = known(port.clone().situation_page(Some(&first_cursor)));
    let second_cursor = second.continuation().unwrap().clone();
    let third = known(port.situation_page(Some(&second_cursor)));

    assert_eq!(first.attempts().len(), MAX_INSPECTION_ATTEMPTS);
    assert_eq!(second.attempts().len(), MAX_INSPECTION_ATTEMPTS);
    assert_eq!(third.attempts().len(), 1);
    assert_eq!(
        (first.offset(), second.offset(), third.offset()),
        (0, 16, 32)
    );
    assert!(!first.enumeration_complete());
    assert!(!second.enumeration_complete());
    assert!(third.enumeration_complete());
    assert!(third.continuation().is_none());
    assert!(grant.grant.capture.borrow().is_none());
    assert_eq!(grant.remaining_queries(), 1);

    let summary = first.summary();
    assert_eq!(summary.attempts, 33);
    assert_eq!(summary.pending_attempts, 31);
    assert_eq!(summary.unresolved_attempts, 1);
    assert_eq!(summary.terminal_attempts, 1);
    assert_eq!(summary.executions, 0);
    assert_eq!(summary.available_units, 68);
    assert_eq!(summary.reserved_units, 16);
    assert_eq!(summary.charged_units, 16);
    assert_eq!(summary.total_units, 100);
    assert_eq!(summary.retained_requests, 0);
    let mut attempts = Vec::new();
    for page in [&first, &second, &third] {
        assert_eq!(page.basis(), first.basis());
        assert_eq!(page.summary(), summary);
        attempts.extend(page.attempts().iter().map(|row| row.attempt));
        for secret in [
            "secret-payload",
            "private-question",
            "private-profile",
            "initial-secret",
        ] {
            assert!(!format!("{page:?}").contains(secret));
        }
    }
    let expected: Vec<_> = [2, 1].into_iter().chain(4..=33).chain([3]).collect();
    assert_eq!(attempts, expected);
    assert_eq!(first.attempts()[0].stage, ActionState::Dispatching);
    assert_eq!(first.attempts()[1].stage, ActionState::Authorized);
    assert_eq!(third.attempts()[0].stage, ActionState::Denied);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), image);
    assert_eq!(supervisor.owner.borrow().snapshot, Some(snapshot()));
    assert_eq!(
        port.situation_page(Some(&second_cursor)),
        Err(ActorError::IdempotencyConflict)
    );
    assert_eq!(grant.remaining_queries(), 1);
}

#[test]
fn continuation_uses_original_capture_when_live_owner_changes_order_and_count() {
    let root = Root::new();
    let (mut host, _reviewer) = create(&root);
    populate(&mut host, 17);
    let (_actor, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.inspection_port(4).unwrap();
    let first = known(port.situation());
    let cursor = first.continuation().unwrap().clone();
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.cancel(revision, 1).unwrap();
        propose(&mut host, 18, true);
    }
    let changed = supervisor.host().unwrap().inspect();
    let image = root.files();
    let original_tail = known(port.situation_page(Some(&cursor)));
    assert_eq!(original_tail.basis(), first.basis());
    assert_eq!(original_tail.summary(), first.summary());
    assert_eq!(original_tail.summary().attempts, 17);
    assert_eq!(original_tail.summary().terminal_attempts, 0);
    assert_eq!(original_tail.attempts()[0].attempt, 17);
    assert!(original_tail.enumeration_complete());

    let fresh = known(port.situation());
    assert_ne!(fresh.basis(), first.basis());
    assert_eq!(fresh.summary().attempts, 18);
    assert_eq!(fresh.summary().terminal_attempts, 1);
    assert_eq!(fresh.attempts()[0].attempt, 2);
    let fresh_tail = known(port.situation_page(fresh.continuation()));
    assert_eq!(fresh_tail.offset(), 16);
    assert_eq!(fresh_tail.attempts()[0].attempt, 18);
    assert_eq!(fresh_tail.attempts()[1].attempt, 1);
    assert_eq!(fresh_tail.attempts()[1].stage, ActionState::Cancelled);
    assert_eq!(fresh_tail.basis(), fresh.basis());
    assert_eq!(fresh_tail.summary(), fresh.summary());
    assert_eq!(supervisor.host().unwrap().inspect(), changed);
    assert_eq!(root.files(), image);
    assert_eq!(grant.remaining_queries(), 0);
}

#[test]
fn foreign_forged_replaced_and_replayed_cursors_preserve_valid_shared_progress() {
    let root = Root::new();
    let (mut host, _reviewer) = create(&root);
    populate(&mut host, 33);
    let (_actor, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.inspection_port(6).unwrap();
    let (foreign, foreign_grant) = supervisor.inspection_port(6).unwrap();
    let replaced = known(port.situation()).continuation().unwrap().clone();
    let current = known(port.clone().situation())
        .continuation()
        .unwrap()
        .clone();
    let other_grant = known(foreign.situation()).continuation().unwrap().clone();
    assert_ne!(current, replaced);
    assert_ne!(current.id(), replaced.id());
    assert_ne!(current.id(), other_grant.id());
    assert_ne!(replaced.id(), other_grant.id());
    assert_ne!(current.id(), 0);
    let forged = FileInspectionCursor {
        id: current.id(),
        grant: current.grant.clone(),
        page: Weak::new(),
    };
    for invalid in [&replaced, &other_grant, &forged] {
        assert_eq!(
            port.situation_page(Some(invalid)),
            Err(ActorError::IdempotencyConflict)
        );
        assert_eq!(grant.remaining_queries(), 4);
    }
    assert_eq!(
        foreign.situation_page(Some(&current)),
        Err(ActorError::IdempotencyConflict)
    );
    assert_eq!(foreign_grant.remaining_queries(), 5);

    let second = known(port.clone().situation_page(Some(&current)));
    let next = second.continuation().unwrap().clone();
    assert_ne!(next.id(), current.id());
    assert_eq!(current.clone(), current);
    assert_eq!(
        port.situation_page(Some(&current)),
        Err(ActorError::IdempotencyConflict)
    );
    assert_eq!(grant.remaining_queries(), 3);
    let final_page = known(port.situation_page(Some(&next)));
    assert_eq!(final_page.attempts()[0].attempt, 33);
    assert!(final_page.enumeration_complete());
    assert_eq!(grant.remaining_queries(), 2);
    assert!(grant.grant.capture.borrow().is_none());
    assert_eq!(
        port.situation_page(Some(&next)),
        Err(ActorError::IdempotencyConflict)
    );
    let foreign_second = known(foreign.situation_page(Some(&other_grant)));
    assert_eq!(foreign_second.offset(), 16);
    assert_eq!(foreign_grant.remaining_queries(), 4);
}

#[test]
fn exhausted_shared_allowance_discards_capture_without_claiming_complete_enumeration() {
    let root = Root::new();
    let (mut host, _reviewer) = create(&root);
    populate(&mut host, 33);
    let (_actor, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.inspection_port(2).unwrap();
    let first = known(port.situation());
    let cursor = first.continuation().unwrap();
    let truncated = known(port.clone().situation_page(Some(cursor)));
    assert_eq!(truncated.offset(), 16);
    assert_eq!(truncated.attempts().len(), 16);
    assert_eq!(truncated.summary().attempts, 33);
    assert!(!truncated.enumeration_complete());
    assert!(truncated.continuation().is_none());
    assert_eq!(grant.remaining_queries(), 0);
    assert!(grant.grant.capture.borrow().is_none());
    assert!(cursor.page.upgrade().is_none());
    assert_eq!(port.situation(), Err(ActorError::Capacity));
    assert_eq!(
        port.clone().situation_page(Some(cursor)),
        Err(ActorError::Capacity)
    );

    let (single, _single_grant) = supervisor.inspection_port(1).unwrap();
    let first = known(single.situation());
    assert_eq!(first.offset(), 0);
    assert!(!first.enumeration_complete());
    assert!(first.continuation().is_none());
}

#[test]
fn revocation_and_custodian_drop_destroy_pending_capture_for_all_clones() {
    let root = Root::new();
    let (mut host, _reviewer) = create(&root);
    populate(&mut host, 17);
    let (_actor, supervisor) = host.into_actor_gateway();
    for explicit in [false, true] {
        let (port, grant) = supervisor.inspection_port(3).unwrap();
        let clone = port.clone();
        let first = known(port.situation());
        let cursor = first.continuation().unwrap();
        assert!(cursor.page.upgrade().is_some());
        if explicit {
            grant.revoke();
        } else {
            drop(grant);
        }
        assert!(cursor.page.upgrade().is_none());
        assert!(cursor.grant.upgrade().is_none());
        for revoked in [&port, &clone] {
            assert_eq!(
                revoked.situation_page(Some(cursor)),
                Ok(Knowledge::Withheld {
                    authority_required: "supervisor_inspection",
                })
            );
        }
    }
    assert!(supervisor.host().is_ok());
}

#[test]
fn owner_loss_clears_pending_pages_without_retaining_file_authority_or_reviving_on_reopen() {
    let root = Root::new();
    let (mut host, _reviewer) = create(&root);
    populate(&mut host, 17);
    let (_actor, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.inspection_port(3).unwrap();
    let first = known(port.situation());
    let cursor = first.continuation().unwrap();
    drop(supervisor);
    assert!(port.owner.upgrade().is_none());
    let image = root.files();
    unknown(port.situation_page(Some(cursor)));
    assert!(grant.grant.capture.borrow().is_none());
    assert!(cursor.page.upgrade().is_none());
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(root.files(), image);

    let (host, _reviewer) = FileOversight::open(root.store(), profile()).unwrap();
    let (_actor, supervisor) = host.into_actor_gateway();
    let (fresh, _grant) = supervisor.inspection_port(3).unwrap();
    let fresh_first = known(fresh.situation());
    assert_ne!(fresh_first.continuation().unwrap().id(), cursor.id());
    unknown(port.situation_page(Some(cursor)));
    assert_eq!(
        fresh.situation_page(Some(cursor)),
        Err(ActorError::IdempotencyConflict)
    );
    assert_eq!(
        known(fresh.situation_page(fresh_first.continuation())).offset(),
        16
    );
}

#[test]
fn conflicting_owner_borrow_invalidates_pending_capture_without_spending() {
    let root = Root::new();
    let (mut host, _reviewer) = create(&root);
    populate(&mut host, 17);
    let (_actor, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.inspection_port(4).unwrap();
    let first = known(port.situation());
    let cursor = first.continuation().unwrap();
    {
        let _borrow = supervisor.host_mut().unwrap();
        unknown(port.situation_page(Some(cursor)));
        assert_eq!(grant.remaining_queries(), 3);
        assert!(grant.grant.capture.borrow().is_none());
    }
    assert_eq!(
        port.situation_page(Some(cursor)),
        Err(ActorError::IdempotencyConflict)
    );
    assert_eq!(grant.remaining_queries(), 3);
    let fresh = known(port.situation());
    let tail = known(port.situation_page(fresh.continuation()));
    assert!(tail.enumeration_complete());
    assert_eq!(grant.remaining_queries(), 1);
}

#[test]
fn original_storage_fault_invalidates_known_pending_capture_without_disk_fallback() {
    let root = Root::new();
    let (mut host, _reviewer) = create(&root);
    populate(&mut host, 17);
    let (_actor, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.inspection_port(3).unwrap();
    let first = known(port.situation());
    let cursor = first.continuation().unwrap();
    std::fs::write(root.store().join("delivery.pending"), b"occupied stage").unwrap();
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        assert!(host.observe_time(revision, ElapsedTick(2)).is_err());
        assert!(host.storage_failure().is_some());
    }
    let image = root.files();
    unknown(port.situation_page(Some(cursor)));
    assert!(grant.grant.capture.borrow().is_none());
    assert!(cursor.page.upgrade().is_none());
    assert_eq!(grant.remaining_queries(), 2);
    unknown(port.situation());
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(root.files(), image);
}

#[test]
fn original_native_token_binding_failure_spends_once_and_cannot_recapture_by_offset() {
    let root = Root::new();
    let (mut host, _reviewer) = create(&root);
    populate(&mut host, 17);
    let (_actor, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.inspection_port(4).unwrap();
    let first = known(port.situation());
    let cursor = first.continuation().unwrap();

    // Substitute an actual token from another equal-revision native capture.
    // This probes the lower boundary: pagination must pass its original token
    // to its original report, never recover by manufacturing a fresh offset.
    let separate = supervisor.host().unwrap().situation().unwrap();
    let Knowledge::Known { value, basis } = separate.page(None, 16).unwrap() else {
        panic!("expected separately captured native page");
    };
    assert_eq!(basis, first.basis());
    grant.grant.capture.borrow_mut().as_mut().unwrap().next = value.next.unwrap();
    let before = supervisor.host().unwrap().inspect();
    let image = root.files();
    assert_eq!(
        port.situation_page(Some(cursor)),
        Err(ActorError::Unavailable)
    );
    assert_eq!(grant.remaining_queries(), 2);
    assert!(grant.grant.capture.borrow().is_none());
    assert!(cursor.page.upgrade().is_none());
    assert_eq!(
        port.situation_page(Some(cursor)),
        Err(ActorError::IdempotencyConflict)
    );
    assert_eq!(grant.remaining_queries(), 2);
    let fresh = known(port.situation());
    let tail = known(port.situation_page(fresh.continuation()));
    assert!(tail.enumeration_complete());
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), image);
}

#[test]
fn opaque_id_allocator_refuses_zero_and_exhaustion_instead_of_rebinding_a_page() {
    let counter = AtomicU64::new(u64::MAX - 1);
    assert_eq!(next_cursor_id(&counter), Some(u64::MAX - 1));
    assert_eq!(next_cursor_id(&counter), None);
    assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    assert_eq!(next_cursor_id(&counter), None);
    let zero = AtomicU64::new(0);
    assert_eq!(next_cursor_id(&zero), None);
    assert_eq!(zero.load(Ordering::Relaxed), 0);
}
