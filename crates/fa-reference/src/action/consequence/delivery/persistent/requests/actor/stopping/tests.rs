//! Original file-backed Stop, two-key publication and conservative liabilities.
//! These cases do not replace native transitions with a mock ledger.
use super::*;
use super::super::{FileActorSupervisor, FileRequestDisposition, JournalError};
use crate::action::consequence::delivery::persistent::{RecoveryReserve, Reconciliation};
use crate::action::consequence::delivery::EndpointOutcome;
use crate::action::consequence::oversight::actor::{ActorError, UnknownReason};
use crate::action::{ActionState, ElapsedTick};
use crate::Error;

#[path = "../hold_diagnosis/tests/fixture.rs"]
mod fixture;
use fixture::*;

fn reserved(root: &Root) -> (
    FileOversight,
    crate::action::consequence::delivery::persistent::observed::FileHumanReviewer,
) {
    let (mut host, reviewer) = create(root);
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    (host, reviewer)
}

fn basis(host: &FileOversight) -> FileFenceBasis {
    let cut = host.inspect();
    FileFenceBasis {
        scope: host.actor_explanation_scope(),
        journal_revision: cut.revision,
        control_sequence: cut.control.sequence,
        authority_epoch: cut.control.ledger.epoch,
    }
}

fn request(supervisor: &FileActorSupervisor<FileOversight>, operation: u64) -> FileFenceRequest {
    FileFenceRequest::new(operation, basis(&supervisor.host().unwrap())).unwrap()
}

fn receipt(result: FileFenceResult) -> FileFenceReceipt {
    let Ok(Knowledge::Known { value, basis }) = result else {
        panic!("expected acknowledged original admission stop: {result:?}");
    };
    assert_eq!(value.after(), basis);
    assert_eq!(value.before().scope, basis.scope);
    assert_eq!(value.before().journal_revision + 1, basis.journal_revision);
    assert_eq!(value.before().authority_epoch + 1, basis.authority_epoch);
    assert_eq!(value.stop().request().operation, value.operation());
    assert_eq!(value.stop().revocation_floor(), basis.authority_epoch);
    assert_eq!(&value.progress().receipt, value.stop());
    assert_eq!(value.progress().dispatcher_epoch, value.stop().dispatcher_epoch());
    assert_eq!(value.progress().reserved_units, 0);
    assert!(!value.progress().endpoint_fenced);
    assert!(!value.progress().drained());
    value
}

fn unknown(result: FileFenceResult) {
    assert_eq!(result, Ok(Knowledge::Unknown {
        reason: UnknownReason::ControllerUnavailable,
    }));
}

#[test]
fn original_fence_cancels_reviewing_and_two_key_authorized_work() {
    for authorized in [false, true] {
        let root = Root::new();
        let (mut host, reviewer) = reserved(&root);
        host.submit_request(host.revision(), 701, spec(b"ok"), snapshot()).unwrap();
        let keys = authorized.then(|| {
            let (action, inputs, automatic, request) = prepare_existing(&mut host, 701);
            let revision = host.revision();
            let human = reviewer.approve(&mut host, revision, &request).unwrap();
            (action, inputs, automatic, human)
        });
        let (actor, mut supervisor) = host.into_actor_gateway();
        let before = supervisor.host().unwrap().inspect();
        assert_eq!(before.control.ledger.reserved, 8);
        supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
        let (port, grant) = supervisor.fence_port(MAX_FENCE_REPLAY_EVENTS).unwrap();
        assert!(port.belongs_to(&actor));
        let input = request(&supervisor, 401);
        let stopped = receipt(port.fence(input));
        assert_eq!(stopped.before(), input.expected());
        assert_eq!(stopped.stop().cancelled(), &[1]);
        assert_eq!(stopped.stop().refunded_units(), 8);
        assert_eq!(stopped.progress().charged_units, 0);
        assert!(stopped.progress().unresolved.is_empty());
        assert_eq!(grant.remaining_operations(), 0);
        assert!(supervisor.owner.borrow().snapshot.is_none());
        assert_eq!(actor.submit(702, &proposal(b"ok")).unwrap_err(), ActorError::Unavailable);
        let after = supervisor.host().unwrap().inspect();
        assert_eq!(after.control.ledger.stages[&1], ActionState::Cancelled);
        assert_eq!(after.control.ledger.available, 100);
        assert_eq!(after.executions, 0);
        assert_eq!(after.payload, before.payload);
        assert_eq!(after.control.ledger.elapsed, before.control.ledger.elapsed);
        if let Some((action, inputs, automatic, human)) = &keys {
            let mut host = supervisor.host_mut().unwrap();
            let revision = host.revision();
            assert!(host.dispatch(
                revision, automatic, human, action, inputs, snapshot(),
            ).is_err());
        }
        assert_eq!(supervisor.host().unwrap().inspect(), after);
        let files = root.files();
        assert_eq!(receipt(port.clone().fence(input)), stopped);
        assert_eq!(root.files(), files);
        assert_eq!(supervisor.host().unwrap().inspect(), after);
    }
}

#[test]
fn sent_and_executed_effects_stay_charged_until_the_original_separate_drain() {
    let root = Root::new();
    let (mut host, reviewer) = reserved(&root);
    for external_request in [701, 702] {
        host.submit_request(host.revision(), external_request, spec(b"ok"), snapshot()).unwrap();
        dispatch_existing(&mut host, &reviewer, external_request);
    }
    host.submit_request(host.revision(), 703, spec(b"ok"), snapshot()).unwrap();
    assert!(matches!(host.publish(host.revision(), 2).unwrap(), EndpointOutcome::Executed { .. }));
    let (_, mut supervisor) = host.into_actor_gateway();
    let before = supervisor.host().unwrap().inspect();
    let (port, grant) = supervisor.fence_port(MAX_FENCE_REPLAY_EVENTS).unwrap();
    let input = request(&supervisor, 501);
    let stopped = receipt(port.fence(input));
    assert_eq!(stopped.stop().cancelled(), &[3]);
    assert_eq!(stopped.stop().refunded_units(), 8);
    assert_eq!(stopped.progress().unresolved, vec![1, 2]);
    assert!(stopped.progress().irrecoverable.is_empty());
    assert_eq!(stopped.progress().charged_units, 16);
    let after = supervisor.host().unwrap().inspect();
    assert_eq!(after.executions, 1);
    assert_eq!(after.executions, before.executions);
    assert_eq!(after.payload, before.payload);
    assert_eq!(after.control.ledger.available, 84);
    assert_eq!(after.control.ledger.stages[&1], ActionState::Unknown);
    assert_eq!(after.control.ledger.stages[&2], ActionState::Unknown);
    assert_eq!(after.control.ledger.stages[&3], ActionState::Cancelled);
    assert_eq!(after.control.ledger.elapsed, before.control.ledger.elapsed);

    // A distinct original supervisor action supplies current time and settles
    // the endpoint. The fence port has no such method or clock argument.
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        let drained = host.progress_stop(revision, ElapsedTick(2)).unwrap();
        assert!(drained.progress.drained());
        assert_eq!(drained.progress.charged_units, 8);
        assert!(matches!(drained.outcomes[&1], Ok(Reconciliation::Resolved(
            EndpointOutcome::NotExecuted { .. }
        ))));
        assert!(matches!(drained.outcomes[&2], Ok(Reconciliation::Resolved(
            EndpointOutcome::Executed { .. }
        ))));
    }
    let current = supervisor.host().unwrap().inspect();
    let files = root.files();
    supervisor.set_snapshot(current.revision, Some(snapshot())).unwrap();
    let retried = receipt(port.clone().fence(input));
    assert_eq!(retried, stopped);
    assert!(!retried.progress().endpoint_fenced);
    assert_eq!(retried.progress().charged_units, 16);
    assert_eq!(grant.remaining_operations(), 0);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    assert_eq!(supervisor.host().unwrap().inspect(), current);
    assert_eq!(root.files(), files);
}

#[test]
fn each_predecessor_coordinate_is_checked_before_work_or_snapshot_consumption() {
    let root = Root::new();
    let (mut host, _) = reserved(&root);
    host.submit_request(host.revision(), 701, spec(b"ok"), snapshot()).unwrap();
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.fence_port(MAX_FENCE_REPLAY_EVENTS).unwrap();
    let input = request(&supervisor, 601);
    let original = input.expected();
    let mut changes = Vec::new();
    let mut changed = original; changed.scope.run += 1; changes.push(changed);
    let mut changed = original; changed.scope.principal += 1; changes.push(changed);
    let mut changed = original; changed.journal_revision += 1; changes.push(changed);
    let mut changed = original; changed.control_sequence += 1; changes.push(changed);
    let mut changed = original; changed.authority_epoch += 1; changes.push(changed);
    supervisor.set_snapshot(original.journal_revision, Some(snapshot())).unwrap();
    let before = supervisor.host().unwrap().inspect();
    let files = root.files();
    assert_eq!(FileFenceRequest::new(0, original), Err(FileFenceError::MalformedRequest));
    for expected in &changes {
        assert_eq!(port.fence(FileFenceRequest::new(601, *expected).unwrap()),
            Err(FileFenceError::StalePredecessor));
    }
    {
        let _borrow = supervisor.owner.borrow_mut();
        unknown(port.fence(input));
        assert!(matches!(supervisor.fence_port(MAX_FENCE_REPLAY_EVENTS),
            Err(JournalError::Unavailable)));
    }
    assert_eq!(grant.remaining_operations(), 1);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), files);
    let stopped = receipt(port.fence(input));
    for expected in changes {
        assert_eq!(port.clone().fence(FileFenceRequest::new(601, expected).unwrap()),
            Err(FileFenceError::IdempotencyConflict));
    }
    assert_eq!(port.fence(request(&supervisor, 602)), Err(FileFenceError::Capacity));
    assert_eq!(receipt(port.fence(input)), stopped);
}

#[test]
fn reserve_and_full_original_event_allowance_are_required_without_replay() {
    let missing = Root::new();
    let host = owner(&missing);
    let (_, supervisor) = host.into_actor_gateway();
    assert!(matches!(supervisor.fence_port(MAX_FENCE_REPLAY_EVENTS),
        Err(JournalError::Contract(Error::Incomplete))));

    let root = Root::new();
    let (host, _) = reserved(&root);
    let (_, mut supervisor) = host.into_actor_gateway();
    for events in [0, MAX_FENCE_REPLAY_EVENTS + 1] {
        assert!(supervisor.fence_port(events).is_err());
    }
    for (events, numerical) in [(0, 1), (1, 0), (1, 2), (MAX_FENCE_REPLAY_EVENTS + 1, 1)] {
        assert!(supervisor.fence_port_with_numerical_budget(events, numerical).is_err());
    }
    let input = request(&supervisor, 701);
    let events = usize::try_from(input.expected().journal_revision).unwrap();
    supervisor.set_snapshot(input.expected().journal_revision, Some(snapshot())).unwrap();
    let (limited, limit) = supervisor.fence_port(events).unwrap();
    let before = supervisor.host().unwrap().inspect();
    let files = root.files();
    assert_eq!(limited.fence(input), Err(FileFenceError::Capacity));
    assert_eq!(limit.remaining_operations(), 1);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), files);
    let (exact, grant) = supervisor.fence_port_with_numerical_budget(events + 1, 1).unwrap();
    receipt(exact.fence(input));
    assert_eq!(grant.max_replay_events(), events + 1);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(1));
    assert_eq!(grant.remaining_operations(), 0);
}

#[test]
fn terminal_reserve_stops_at_ordinary_capacity_and_exact_retry_writes_nothing() {
    let root = Root::new();
    let mut selected = profile();
    selected.delivery.limits.events = 8;
    let (mut host, _) = FileOversight::create(root.store(), selected).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    while host.journal_capacity().unwrap().ordinary_remaining().events != 0 {
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    }
    assert_eq!(host.revision(), 5);
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.fence_port(6).unwrap();
    let input = request(&supervisor, 801);
    let stopped = receipt(port.fence(input));
    assert_eq!(stopped.after().journal_revision, 6);
    assert_eq!(supervisor.host().unwrap().journal_capacity().unwrap().remaining().events, 2);
    let files = root.files();
    for _ in 0..3 {
        assert_eq!(receipt(port.clone().fence(input)), stopped);
    }
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(root.files(), files);
}

#[test]
fn revocation_and_owner_loss_suppress_even_a_cached_acknowledgment() {
    let root = Root::new();
    let (host, _) = reserved(&root);
    let (actor, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.fence_port(MAX_FENCE_REPLAY_EVENTS).unwrap();
    let input = request(&supervisor, 901);
    receipt(port.fence(input));
    let foreign_root = Root::new();
    let (foreign_host, _) = reserved(&foreign_root);
    let (foreign, _foreign_supervisor) = foreign_host.into_actor_gateway();
    assert!(port.belongs_to(&actor));
    assert!(!port.belongs_to(&foreign));
    let mut wrong = port.clone();
    wrong.scope.branch += 1;
    unknown(wrong.fence(input));
    let files = root.files();
    grant.revoke();
    assert_eq!(port.clone().fence(input), Ok(Knowledge::Withheld {
        authority_required: "supervisor_fence",
    }));
    assert_eq!(root.files(), files);

    let (orphan, _custody) = supervisor.fence_port(MAX_FENCE_REPLAY_EVENTS).unwrap();
    let current = request(&supervisor, 902);
    drop(supervisor);
    assert!(orphan.owner.upgrade().is_none());
    unknown(orphan.fence(current));
    let (reopened, _) = FileOversight::open(root.store(), profile()).unwrap();
    let (replacement_actor, replacement) = reopened.into_actor_gateway();
    assert!(!orphan.belongs_to(&replacement_actor));
    assert!(orphan.belongs_to(&actor));
    let (fresh, _fresh_grant) = replacement.fence_port(MAX_FENCE_REPLAY_EVENTS).unwrap();
    assert_eq!(fresh.fence(input), Err(FileFenceError::StalePredecessor));
    assert_eq!(fresh.fence(request(&replacement, 903)), Err(FileFenceError::Unsupported));
}

#[test]
fn fresh_grants_cannot_adopt_or_append_an_already_acknowledged_stop() {
    let root = Root::new();
    let (host, _) = reserved(&root);
    let (_, supervisor) = host.into_actor_gateway();
    let (port, _grant) = supervisor.fence_port(MAX_FENCE_REPLAY_EVENTS).unwrap();
    let input = request(&supervisor, 1001);
    let acknowledged = receipt(port.fence(input));
    let (fresh, fresh_grant) = supervisor.fence_port(MAX_FENCE_REPLAY_EVENTS).unwrap();
    let before = supervisor.host().unwrap().inspect();
    let files = root.files();
    assert_eq!(fresh.fence(input), Err(FileFenceError::StalePredecessor));
    assert_eq!(fresh.fence(request(&supervisor, 1001)), Err(FileFenceError::Unsupported));
    assert_eq!(fresh.fence(request(&supervisor, 1002)), Err(FileFenceError::Unsupported));
    assert_eq!(fresh_grant.remaining_operations(), 1);
    assert_eq!(receipt(port.fence(input)), acknowledged);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), files);
}

#[test]
fn storage_failure_latches_spent_work_and_returns_no_candidate_stop_or_refund() {
    let root = Root::new();
    let (mut host, _) = reserved(&root);
    host.submit_request(host.revision(), 701, spec(b"ok"), snapshot()).unwrap();
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.fence_port(MAX_FENCE_REPLAY_EVENTS).unwrap();
    let input = request(&supervisor, 1101);
    let before = supervisor.host().unwrap().inspect();
    supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
    std::fs::write(root.store().join("delivery.pending"), b"occupied original stage").unwrap();
    assert_eq!(port.fence(input), Err(FileFenceError::Unavailable));
    assert_eq!(grant.remaining_operations(), 0);
    assert!(supervisor.owner.borrow().snapshot.is_none());
    assert!(supervisor.host().unwrap().storage_failure().is_some());
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(before.control.ledger.reserved, 8);
    let files = root.files();
    unknown(port.clone().fence(input));
    assert!(matches!(supervisor.fence_port(MAX_FENCE_REPLAY_EVENTS), Err(JournalError::Unavailable)));
    assert_eq!(root.files(), files);
}

#[test]
fn no_fresh_clock_is_required_after_reopening_an_original_uncertain_dispatch() {
    let root = Root::new();
    let (mut host, reviewer) = reserved(&root);
    host.submit_request(host.revision(), 701, spec(b"ok"), snapshot()).unwrap();
    dispatch_existing(&mut host, &reviewer, 701);
    drop(host);
    drop(reviewer);
    let (host, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert!(!host.clock_ready());
    let (_, supervisor) = host.into_actor_gateway();
    let before = supervisor.host().unwrap().inspect();
    let (port, _grant) = supervisor.fence_port(MAX_FENCE_REPLAY_EVENTS).unwrap();
    let stopped = receipt(port.fence(request(&supervisor, 1201)));
    assert_eq!(stopped.progress().charged_units, 8);
    assert_eq!(stopped.progress().unresolved, vec![1]);
    assert_eq!(supervisor.host().unwrap().inspect().control.ledger.elapsed,
        before.control.ledger.elapsed);
    assert!(!supervisor.host().unwrap().clock_ready());
}

#[test]
fn missing_live_source_cannot_block_an_explicit_reserved_admission_stop() {
    use crate::action::consequence::delivery::persistent::observed::source::FileSourcePolicy;
    use crate::action::consequence::oversight::evidence_source::{
        EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource,
    };
    use crate::action::consequence::oversight::policy_state::{
        StateFreshness, StateLimits, StateSource,
    };
    use std::collections::BTreeMap;

    let unreserved = Root::new();
    let (host, _) = source_owner(&unreserved);
    let (_, unreserved_supervisor) = host.into_actor_gateway();
    assert!(matches!(unreserved_supervisor.fence_port(MAX_FENCE_REPLAY_EVENTS),
        Err(JournalError::Contract(Error::Incomplete))));

    let root = Root::new();
    let (mut host, _) = reserved(&root);
    let scope = profile().delivery.scope;
    host.enable_file_source(host.revision(), FileSourcePolicy {
        source: StateSource { scope, source: 51, generation: 1 },
        limits: StateLimits::default(),
        freshness: StateFreshness::new(5).unwrap(),
    }).unwrap();
    let evidence = EvidenceSnapshot::new(
        EvidenceIdentity { source: 51, generation: 1, scope },
        snapshot(),
        BTreeMap::from([("r".into(), b"private helper input".to_vec())]),
    ).unwrap();
    std::fs::write(root.source(), evidence.encode()).unwrap();
    let mut source = FileEvidenceSource::new(root.source(), 51, scope, 1048576).unwrap();
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap();
    host.submit_request(host.revision(), 701, spec(b"ok"), snapshot()).unwrap();
    let reads = source.status().read_attempts;
    std::fs::remove_file(root.source()).unwrap();
    let (_, supervisor) = host.into_actor_gateway();
    let (port, _grant) = supervisor.fence_port(MAX_FENCE_REPLAY_EVENTS).unwrap();
    let stopped = receipt(port.fence(request(&supervisor, 1301)));
    assert_eq!(stopped.stop().cancelled(), &[1]);
    assert_eq!(stopped.stop().refunded_units(), 8);
    assert_eq!(source.status().read_attempts, reads);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
}

mod numerical;
