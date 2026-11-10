//! Actual original requests, two-key dispatch and endpoint receipts. No model
//! of a refund or caller-supplied outcome substitutes for native publication.
use super::*;
use super::super::{FileRequestDisposition, JournalError};
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::EndpointOutcome;
use crate::action::consequence::oversight::actor::{ActorError, UnknownReason};

#[path = "../hold_diagnosis/tests/fixture.rs"]
mod fixture;
use fixture::*;

fn plan(result: FileResolutionPlanResult) -> FileResolutionPlan {
    let Ok(Knowledge::Known { value, basis }) = result else {
        panic!("expected native resolution plan: {result:?}");
    };
    assert_eq!(value.basis(), basis);
    match value.request() {
        Some(input) => {
            assert!(value.can_reconcile());
            assert_eq!(input.expected(), basis);
            assert_eq!(input.operation(), value.operation());
            assert_eq!(value.block_reason(), None);
        }
        None => {
            assert!(!value.can_reconcile());
            assert!(value.block_reason().is_some());
        }
    }
    value
}

fn receipt(result: FileResolutionResult) -> FileResolutionReceipt {
    let Ok(Knowledge::Known { value, basis }) = result else {
        panic!("expected original reconciliation acknowledgment: {result:?}");
    };
    assert_eq!(value.after(), basis);
    assert_eq!(value.before().scope, basis.scope);
    assert_eq!(value.before().request, basis.request);
    assert_eq!(value.before().attempt, basis.attempt);
    assert_eq!(value.before().recorded_at, basis.recorded_at);
    assert_eq!(value.before().journal_revision + 1, basis.journal_revision);
    value
}

fn unknown<T: fmt::Debug + PartialEq>(
    result: Result<Knowledge<T, FileHoldDiagnosisBasis>, FileResolutionError>,
) {
    assert_eq!(
        result,
        Ok(Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        })
    );
}

fn dispatched(root: &Root, published: bool) -> FileOversight {
    let (mut host, reviewer) = create(root);
    host.submit_request(host.revision(), 701, spec(b"ok"), snapshot())
        .unwrap();
    dispatch_existing(&mut host, &reviewer, 701);
    if published {
        assert!(matches!(
            host.publish(host.revision(), 1).unwrap(),
            EndpointOutcome::Executed { .. }
        ));
    }
    assert_eq!(
        host.request_status(701).unwrap().disposition,
        FileRequestDisposition::Admitted {
            attempt: 1,
            stage: ActionState::Dispatching,
        }
    );
    host
}

#[test]
fn executable_next_accepts_the_original_published_receipt_without_reexecution() {
    let root = Root::new();
    let host = dispatched(&root, true);
    let (actor, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.resolution_port(3, 1, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
    let before = supervisor.host().unwrap().inspect();
    let image = root.files();
    let offered = plan(port.next(701, 401));
    assert_eq!((offered.basis().request, offered.basis().attempt), (701, 1));
    assert_eq!(offered.stage(), ActionState::Dispatching);
    let input = offered.request().unwrap();
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), image);

    supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
    let resolved = receipt(port.resolve(input));
    assert_eq!(resolved.before(), offered.basis());
    assert_eq!(resolved.stage(), ActionState::Confirmed);
    assert!(matches!(resolved.outcome(), Reconciliation::Resolved(
        EndpointOutcome::Executed { .. }
    )));
    assert_eq!(grant.remaining_operations(), 0);
    let after = supervisor.host().unwrap().inspect();
    assert_eq!(after.executions, before.executions);
    assert_eq!(after.executions, 1);
    assert_eq!(after.payload, before.payload);
    assert_eq!(after.control.ledger.charged, 8);
    assert!(supervisor.owner.borrow().snapshot.is_none());
    assert_eq!(actor.submit(702, &proposal(b"ok")).unwrap_err(), ActorError::Unavailable);
    assert_eq!(supervisor.host().unwrap().inspect(), after);

    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(2)).unwrap();
    }
    let later = supervisor.host().unwrap().inspect();
    supervisor.set_snapshot(later.revision, Some(snapshot())).unwrap();
    let image = root.files();
    assert_eq!(receipt(port.clone().resolve(input)), resolved);
    assert_eq!(supervisor.host().unwrap().inspect(), later);
    assert_eq!(root.files(), image);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    let terminal = plan(port.next(701, 402));
    assert_eq!(terminal.block_reason(), Some(FileResolutionBlockReason::Terminal));

    // Neither a cached acknowledgment nor Next consumes the NEW one-use snapshot.
    let mut fresh = proposal(b"ok");
    fresh.target = later.target;
    actor.submit(702, &fresh).unwrap();
    assert!(matches!(
        supervisor.host().unwrap().request_status(702).unwrap().disposition,
        FileRequestDisposition::Admitted { attempt: 2, .. }
    ));
}

#[test]
fn missing_receipt_stays_charged_and_a_later_original_publication_can_be_resolved() {
    let root = Root::new();
    let host = dispatched(&root, false);
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.resolution_port(4, 2, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
    let first_plan = plan(port.next(701, 501));
    let first_input = first_plan.request().unwrap();
    let first = receipt(port.resolve(first_input));
    assert_eq!(first.outcome(), Reconciliation::AwaitingResolution);
    assert_eq!(first.stage(), ActionState::Unknown);
    assert_eq!(first.after().request_generation, first.before().request_generation);
    let unresolved = supervisor.host().unwrap().inspect();
    assert_eq!(unresolved.control.ledger.charged, 8);
    assert_eq!(unresolved.executions, 0);
    assert_eq!(unresolved.payload, b"initial");
    assert_eq!(grant.remaining_operations(), 1);

    // Only the original supervisor publishes this retained envelope. Resolve
    // has neither sent it nor created a replacement dispatch/key.
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.publish(revision, 1).unwrap();
    }
    let used = plan(port.next(701, 501));
    assert_eq!(used.block_reason(), Some(FileResolutionBlockReason::OperationAlreadyUsed));
    let second_plan = plan(port.next(701, 502));
    assert!(second_plan.basis().journal_revision > first.after().journal_revision);
    let second = receipt(port.resolve(second_plan.request().unwrap()));
    assert_eq!(second.stage(), ActionState::Confirmed);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 1);
    assert_eq!(supervisor.host().unwrap().inspect().control.ledger.charged, 8);
    let current = supervisor.host().unwrap().inspect();
    let image = root.files();
    assert_eq!(receipt(port.clone().resolve(first_input)), first);
    assert_eq!(first.stage(), ActionState::Unknown);
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(supervisor.host().unwrap().inspect(), current);
    assert_eq!(root.files(), image);
}

#[test]
fn elapsed_deadline_alone_never_seals_or_refunds_a_missing_receipt() {
    let root = Root::new();
    let mut host = dispatched(&root, false);
    host.observe_time(host.revision(), ElapsedTick(100)).unwrap();
    let (_, supervisor) = host.into_actor_gateway();
    let (port, _grant) = supervisor.resolution_port(1, 1, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
    let offered = plan(port.next(701, 601));
    assert!(offered.can_reconcile());
    assert_eq!(offered.basis().recorded_at, Some(ElapsedTick(100)));
    let resolved = receipt(port.resolve(offered.request().unwrap()));
    assert_eq!(resolved.outcome(), Reconciliation::AwaitingResolution);
    assert_eq!(resolved.stage(), ActionState::Unknown);
    let after = supervisor.host().unwrap().inspect();
    assert_eq!(after.executions, 0);
    assert_eq!(after.payload, b"initial");
    assert_eq!(after.control.ledger.available, 92);
    assert_eq!(after.control.ledger.charged, 8);
}

#[test]
fn receipt_retention_expiry_preserves_uncertainty_even_after_actual_execution() {
    let root = Root::new();
    let mut host = dispatched(&root, true);
    host.observe_time(host.revision(), ElapsedTick(1001)).unwrap();
    let (_, supervisor) = host.into_actor_gateway();
    let (port, _grant) = supervisor.resolution_port(1, 1, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
    let offered = plan(port.next(701, 602));
    let resolved = receipt(port.resolve(offered.request().unwrap()));
    assert_eq!(resolved.outcome(), Reconciliation::RetentionExpired);
    assert_eq!(resolved.stage(), ActionState::Unknown);
    let after = supervisor.host().unwrap().inspect();
    assert_eq!(after.executions, 1);
    assert_eq!(after.payload, b"ok");
    assert_eq!(after.control.ledger.charged, 8);
    assert_eq!(after.control.ledger.available, 92);
}

#[test]
fn only_a_real_nonexecution_receipt_refunds_its_original_charge_once() {
    let root = Root::new();
    let (mut host, reviewer) = create(&root);
    for request in [701, 702] {
        host.submit_request(host.revision(), request, spec(b"ok"), snapshot()).unwrap();
        dispatch_existing(&mut host, &reviewer, request);
    }
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert!(matches!(
        host.publish(host.revision(), 1).unwrap(),
        EndpointOutcome::Executed { .. }
    ));
    assert!(matches!(
        host.publish(host.revision(), 2).unwrap(),
        EndpointOutcome::NotExecuted { .. }
    ));
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.resolution_port(2, 2, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
    let offered = plan(port.next(702, 7010));
    let input = offered.request().unwrap();
    let resolved = receipt(port.resolve(input));
    assert_eq!(resolved.stage(), ActionState::ConfirmedNotExecuted);
    assert!(matches!(resolved.outcome(), Reconciliation::Resolved(
        EndpointOutcome::NotExecuted { .. }
    )));
    let after = supervisor.host().unwrap().inspect();
    assert_eq!(after.control.ledger.charged, 8);
    assert_eq!(after.control.ledger.available, 92);
    assert_eq!(after.executions, 1);
    let image = root.files();
    assert_eq!(receipt(port.clone().resolve(input)), resolved);
    assert_eq!(supervisor.host().unwrap().inspect(), after);
    assert_eq!(root.files(), image);
    assert_eq!(grant.remaining_operations(), 1);

    let first = plan(port.next(701, 7011));
    let executed = receipt(port.resolve(first.request().unwrap()));
    assert_eq!(executed.stage(), ActionState::Confirmed);
    assert_eq!(supervisor.host().unwrap().inspect().control.ledger.charged, 8);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 1);
}

#[test]
fn pending_authorized_denied_and_cancelled_requests_do_not_offer_reconciliation() {
    let root = Root::new();
    let mut host = owner(&root);
    for (request, payload) in [
        (701, b"ok".as_slice()),
        (702, b"blocked".as_slice()),
        (703, b"ok".as_slice()),
    ] {
        host.submit_request(host.revision(), request, spec(payload), snapshot()).unwrap();
    }
    host.cancel_request(host.revision(), 703).unwrap();
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.resolution_port(4, 1, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
    for (request, expected) in [
        (701, FileResolutionBlockReason::NotDispatched),
        (702, FileResolutionBlockReason::Terminal),
        (703, FileResolutionBlockReason::Terminal),
    ] {
        let offered = plan(port.next(request, 801));
        assert_eq!(offered.block_reason(), Some(expected));
        assert_eq!(
            port.resolve(FileResolutionRequest::new(801, offered.basis()).unwrap()),
            Err(FileResolutionError::Unsupported)
        );
    }
    {
        let mut host = supervisor.host_mut().unwrap();
        let _reviewed = prepare_existing(&mut host, 701);
    }
    let inspection = supervisor.host().unwrap().inspect();
    let image = root.files();
    let authorized = plan(port.next(701, 801));
    assert_eq!(authorized.stage(), ActionState::Authorized);
    assert_eq!(authorized.basis().request_generation, 0);
    assert_eq!(authorized.block_reason(), Some(FileResolutionBlockReason::NotDispatched));
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(supervisor.host().unwrap().inspect(), inspection);
    assert_eq!(root.files(), image);
}

#[test]
fn request_mapping_never_resolves_a_direct_attempt_or_unadmitted_request() {
    let root = Root::new();
    let (mut host, reviewer) = create(&root);
    host.propose(host.revision(), 1, spec(b"ok"), snapshot()).unwrap();
    host.submit_request(host.revision(), 901, spec(b"ok"), snapshot()).unwrap();
    let mut invalid = spec(b"ok");
    invalid.target.as_mut().unwrap().expected_version = 99;
    assert!(matches!(
        host.submit_request(host.revision(), 903, invalid, snapshot()).unwrap().disposition,
        FileRequestDisposition::NotAdmitted(_)
    ));
    dispatch_existing(&mut host, &reviewer, 901);
    host.publish(host.revision(), 2).unwrap();
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.resolution_port(3, 1, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
    let before = supervisor.host().unwrap().inspect();
    let image = root.files();
    unknown(port.next(1, 9010));
    unknown(port.next(903, 9010));
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), image);
    let offered = plan(port.next(901, 9010));
    assert_eq!((offered.basis().request, offered.basis().attempt), (901, 2));
    let mut alias = offered.basis();
    alias.request = 1;
    alias.attempt = 1;
    unknown(port.resolve(FileResolutionRequest::new(9010, alias).unwrap()));
    assert_eq!(grant.remaining_operations(), 1);
    let resolved = receipt(port.resolve(offered.request().unwrap()));
    assert_eq!(resolved.after().attempt, 2);
    assert_eq!(resolved.stage(), ActionState::Confirmed);
    let after = supervisor.host().unwrap().inspect();
    assert_eq!(after.control.ledger.stages[&1], ActionState::Reviewing);
    assert_eq!(after.executions, 1);
}

mod boundaries;
mod numerical;
