//! Actual durable requests and two-key publication, with paired unavailable cases.
use super::*;
use crate::action::consequence::delivery::StopRequest;
use crate::action::consequence::delivery::persistent::FileAbandonRequest;
use crate::action::consequence::delivery::persistent::governance::PolicyUpdate;
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};

#[path = "tests/fixture.rs"]
mod fixture;
use fixture::*;

fn known(result: FileHoldDiagnosisResult) -> FileHoldDiagnosis {
    let Ok(Knowledge::Known { value, basis }) = result else {
        panic!("expected original recorded-state diagnosis: {result:?}");
    };
    assert_eq!(value.basis(), basis);
    assert_eq!(value.predicates().len(), 10);
    for row in value.predicates() {
        if let Knowledge::Known { basis: row_basis, .. } = row.satisfied() {
            assert_eq!(*row_basis, basis);
        }
    }
    for condition in [
        FileHoldCondition::CurrentSourceEvidence,
        FileHoldCondition::CurrentApprovals,
        FileHoldCondition::CurrentClock,
        FileHoldCondition::FinalPublicationEligibility,
    ] {
        assert_eq!(
            predicate(&value, condition),
            &Knowledge::Unknown {
                reason: UnknownReason::ControllerUnavailable,
            }
        );
    }
    value
}

fn predicate(
    value: &FileHoldDiagnosis,
    condition: FileHoldCondition,
) -> &Knowledge<bool, FileHoldDiagnosisBasis> {
    let matching: Vec<_> = value
        .predicates()
        .iter()
        .filter(|row| row.condition() == condition)
        .collect();
    assert_eq!(matching.len(), 1);
    matching[0].satisfied()
}

fn recorded(value: &FileHoldDiagnosis, condition: FileHoldCondition, expected: bool) {
    assert_eq!(
        predicate(value, condition),
        &Knowledge::Known {
            value: expected,
            basis: value.basis(),
        }
    );
}

fn unknown(result: FileHoldDiagnosisResult) {
    assert_eq!(
        result,
        Ok(Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        })
    );
}

#[test]
fn live_cut_preserves_source_work_and_an_unused_real_admission_snapshot() {
    let root = Root::new();
    let (host, source) = source_owner(&root);
    let (actor, mut supervisor) = host.into_actor_gateway();
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
    actor.submit(701, &proposal(b"ok")).unwrap();
    let before = supervisor.host().unwrap().inspect();
    supervisor
        .set_snapshot(before.revision, Some(snapshot()))
        .unwrap();
    let reads = source.status().read_attempts;
    std::fs::remove_file(root.source()).unwrap();
    let files = root.files();
    let (port, grant) = supervisor.hold_diagnosis_port(2).unwrap();
    let value = known(port.why_held(701));
    assert_eq!(value.stage(), ActionState::Reviewing);
    assert_eq!(value.basis().scope, profile().delivery.scope);
    assert_eq!((value.basis().request, value.basis().attempt), (701, 1));
    assert_eq!(value.basis().request_generation, 0);
    assert_eq!(value.basis().journal_revision, before.revision);
    assert_eq!(value.basis().control_sequence, before.control.sequence);
    assert_eq!(value.basis().recorded_at, Some(ElapsedTick(1)));
    recorded(&value, FileHoldCondition::RecordedAdmissionOpen, true);
    recorded(&value, FileHoldCondition::RecordedNotSuspended, true);
    recorded(&value, FileHoldCondition::RecordedPolicyEpochMatches, true);
    recorded(&value, FileHoldCondition::RecordedDeadlineOpen, true);
    recorded(&value, FileHoldCondition::RecordedStageAuthorized, false);
    recorded(&value, FileHoldCondition::DispatchNotRecorded, true);
    assert_eq!(source.status().read_attempts, reads);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), files);
    assert_eq!(grant.remaining_queries(), 1);
    for secret in ["unselected private source", "private helper input", "approve?"] {
        assert!(!format!("{value:?}").contains(secret));
    }
    // A real subsequent admission consumes the snapshot left untouched by diagnosis.
    actor.submit(702, &proposal(b"ok")).unwrap();
    assert!(matches!(
        supervisor.host().unwrap().request_status(702).unwrap().disposition,
        FileRequestDisposition::Admitted { attempt: 2, .. }
    ));
    assert_eq!(source.status().read_attempts, reads);
}

#[test]
fn actual_authorization_changes_predicates_without_changing_request_generation() {
    let root = Root::new();
    let mut host = owner(&root);
    host.submit_request(host.revision(), 701, spec(b"ok"), snapshot())
        .unwrap();
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.hold_diagnosis_port(3).unwrap();
    let before = known(port.why_held(701));
    assert_eq!(before.stage(), ActionState::Reviewing);
    recorded(&before, FileHoldCondition::RecordedStageAuthorized, false);
    {
        let mut host = supervisor.host_mut().unwrap();
        let _reviewed = prepare_existing(&mut host, 701);
    }
    let inspection = supervisor.host().unwrap().inspect();
    let files = root.files();
    let after = known(port.clone().why_held(701));
    assert_eq!(after.stage(), ActionState::Authorized);
    assert_eq!(after.basis().request_generation, before.basis().request_generation);
    assert!(after.basis().journal_revision > before.basis().journal_revision);
    assert_eq!(after.basis().journal_revision, inspection.revision);
    assert_eq!(inspection.control.ledger.reserved, 8);
    for condition in [
        FileHoldCondition::RecordedAdmissionOpen,
        FileHoldCondition::RecordedNotSuspended,
        FileHoldCondition::RecordedPolicyEpochMatches,
        FileHoldCondition::RecordedDeadlineOpen,
        FileHoldCondition::RecordedStageAuthorized,
        FileHoldCondition::DispatchNotRecorded,
    ] {
        recorded(&after, condition, true);
    }
    // The helper is real and authorization reserved funds; current keys/source/
    // time and final eligibility nevertheless remain Unknown (checked by known).
    assert_eq!(before.stage(), ActionState::Reviewing);
    recorded(&before, FileHoldCondition::RecordedStageAuthorized, false);
    assert_eq!(known(port.why_held(701)), after);
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(supervisor.host().unwrap().inspect(), inspection);
    assert_eq!(root.files(), files);
}

#[test]
fn exact_recorded_deadline_is_closed_while_the_preceding_tick_is_open() {
    let root = Root::new();
    let mut host = owner(&root);
    host.submit_request(host.revision(), 701, spec(b"ok"), snapshot())
        .unwrap();
    host.observe_time(host.revision(), ElapsedTick(99)).unwrap();
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, _grant) = supervisor.hold_diagnosis_port(2).unwrap();
    let before = known(port.why_held(701));
    assert_eq!(before.deadline(), ElapsedTick(100));
    recorded(&before, FileHoldCondition::RecordedDeadlineOpen, true);
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(100)).unwrap();
    }
    let inspection = supervisor.host().unwrap().inspect();
    let files = root.files();
    let after = known(port.why_held(701));
    assert_eq!(after.basis().recorded_at, Some(ElapsedTick(100)));
    recorded(&after, FileHoldCondition::RecordedDeadlineOpen, false);
    recorded(&before, FileHoldCondition::RecordedDeadlineOpen, true);
    assert_eq!(supervisor.host().unwrap().inspect(), inspection);
    assert_eq!(root.files(), files);
}

#[test]
fn exact_request_mapping_never_substitutes_a_direct_or_unadmitted_attempt() {
    let root = Root::new();
    let mut host = owner(&root);
    host.propose(host.revision(), 1, spec(b"ok"), snapshot())
        .unwrap();
    let denied = host
        .submit_request(host.revision(), 901, spec(b"blocked"), snapshot())
        .unwrap();
    assert!(matches!(
        denied.disposition,
        FileRequestDisposition::Admitted { attempt: 2, .. }
    ));
    host.submit_request(host.revision(), 2, spec(b"ok"), snapshot())
        .unwrap();
    let mut invalid_target = spec(b"ok");
    invalid_target.target.as_mut().unwrap().expected_version = 2;
    assert!(matches!(
        host.submit_request(host.revision(), 903, invalid_target, snapshot())
            .unwrap().disposition,
        FileRequestDisposition::NotAdmitted(_)
    ));
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.hold_diagnosis_port(5).unwrap();
    let inspection = supervisor.host().unwrap().inspect();
    let files = root.files();
    let denied = known(port.why_held(901));
    assert_eq!((denied.basis().request, denied.basis().attempt), (901, 2));
    assert_eq!(denied.stage(), ActionState::Denied);
    recorded(&denied, FileHoldCondition::RecordedStageAuthorized, false);
    recorded(&denied, FileHoldCondition::DispatchNotRecorded, true);
    let admitted = known(port.why_held(2));
    assert_eq!((admitted.basis().request, admitted.basis().attempt), (2, 3));
    assert_eq!(admitted.stage(), ActionState::Reviewing);
    unknown(port.why_held(1));
    unknown(port.why_held(903));
    unknown(port.why_held(999));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(port.why_held(901), Err(ActorError::Capacity));
    assert_eq!(supervisor.host().unwrap().inspect(), inspection);
    assert_eq!(root.files(), files);
}

#[test]
fn policy_change_and_stop_advance_the_cut_without_rewriting_the_old_report() {
    let root = Root::new();
    let mut host = owner(&root);
    host.submit_request(host.revision(), 701, spec(b"ok"), snapshot())
        .unwrap();
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, _grant) = supervisor.hold_diagnosis_port(3).unwrap();
    let original = known(port.why_held(701));
    {
        let mut host = supervisor.host_mut().unwrap();
        let inspection = host.inspect();
        let update = PolicyUpdate::new(
            61,
            inspection.control.sequence,
            inspection.control.ledger.epoch,
            Policy::new(2, vec![Predicate::PayloadAtMost(1)]).unwrap(),
        )
        .unwrap();
        let revision = host.revision();
        host.replace_policy(revision, &update).unwrap();
    }
    let changed = known(port.why_held(701));
    assert_eq!(changed.proposal_policy_epoch(), original.proposal_policy_epoch());
    assert!(changed.current_policy_epoch() > original.current_policy_epoch());
    recorded(&changed, FileHoldCondition::RecordedPolicyEpochMatches, false);
    {
        let mut host = supervisor.host_mut().unwrap();
        let inspection = host.inspect();
        host.request_stop(
            inspection.revision,
            StopRequest {
                operation: 81,
                expected_control_sequence: inspection.control.sequence,
                expected_authority_epoch: inspection.control.ledger.epoch,
            },
        )
        .unwrap();
    }
    let inspection = supervisor.host().unwrap().inspect();
    let files = root.files();
    let stopped = known(port.why_held(701));
    recorded(&stopped, FileHoldCondition::RecordedAdmissionOpen, false);
    recorded(
        &stopped,
        FileHoldCondition::RecordedNotSuspended,
        !inspection.control.suspended,
    );
    recorded(&stopped, FileHoldCondition::RecordedStageAuthorized, false);
    recorded(&original, FileHoldCondition::RecordedAdmissionOpen, true);
    recorded(&original, FileHoldCondition::RecordedPolicyEpochMatches, true);
    assert_eq!(supervisor.host().unwrap().inspect(), inspection);
    assert_eq!(root.files(), files);
}

#[test]
fn dispatched_unknown_and_irrecoverable_liabilities_remain_charged() {
    let root = Root::new();
    let (mut host, reviewer) = create(&root);
    host.submit_request(host.revision(), 701, spec(b"ok"), snapshot())
        .unwrap();
    dispatch_existing(&mut host, &reviewer, 701);
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, _grant) = supervisor.hold_diagnosis_port(3).unwrap();
    let dispatched = known(port.why_held(701));
    assert_eq!(dispatched.stage(), ActionState::Dispatching);
    recorded(&dispatched, FileHoldCondition::DispatchNotRecorded, false);
    assert_eq!(supervisor.host().unwrap().inspect().control.ledger.charged, 8);
    {
        let mut host = supervisor.host_mut().unwrap();
        let inspection = host.inspect();
        host.request_stop(
            inspection.revision,
            StopRequest {
                operation: 81,
                expected_control_sequence: inspection.control.sequence,
                expected_authority_epoch: inspection.control.ledger.epoch,
            },
        )
        .unwrap();
    }
    let unresolved = known(port.why_held(701));
    assert_eq!(unresolved.stage(), ActionState::Unknown);
    recorded(&unresolved, FileHoldCondition::DispatchNotRecorded, false);
    assert_eq!(supervisor.host().unwrap().inspect().control.ledger.charged, 8);
    {
        let mut host = supervisor.host_mut().unwrap();
        let inspection = host.inspect();
        host.abandon_unknown(
            inspection.revision,
            FileAbandonRequest {
                operation: 82,
                attempt: unresolved.basis().attempt,
                expected_control_sequence: inspection.control.sequence,
                expected_authority_epoch: inspection.control.ledger.epoch,
            },
        )
        .unwrap();
    }
    let inspection = supervisor.host().unwrap().inspect();
    let files = root.files();
    let abandoned = known(port.why_held(701));
    assert_eq!(abandoned.stage(), ActionState::IrrecoverablyUnknown);
    recorded(&abandoned, FileHoldCondition::DispatchNotRecorded, false);
    recorded(&abandoned, FileHoldCondition::RecordedStageAuthorized, false);
    assert_eq!(inspection.control.ledger.charged, 8);
    assert_eq!(supervisor.host().unwrap().inspect(), inspection);
    assert_eq!(root.files(), files);
}

#[test]
fn actual_confirmed_publication_and_cancelled_request_remain_terminal() {
    let root = Root::new();
    let (mut host, reviewer) = create(&root);
    host.submit_request(host.revision(), 701, spec(b"ok"), snapshot())
        .unwrap();
    dispatch_existing(&mut host, &reviewer, 701);
    assert!(matches!(
        host.submit_request(host.revision(), 702, spec(b"ok"), snapshot())
            .unwrap()
            .disposition,
        FileRequestDisposition::Admitted { attempt: 2, .. }
    ));
    host.publish(host.revision(), 1).unwrap();
    host.reconcile(host.revision(), 1).unwrap();
    host.cancel_request(host.revision(), 702).unwrap();
    let (_, supervisor) = host.into_actor_gateway();
    let (port, _grant) = supervisor.hold_diagnosis_port(2).unwrap();
    let inspection = supervisor.host().unwrap().inspect();
    let files = root.files();
    let confirmed = known(port.why_held(701));
    assert_eq!(confirmed.stage(), ActionState::Confirmed);
    recorded(&confirmed, FileHoldCondition::RecordedStageAuthorized, false);
    recorded(&confirmed, FileHoldCondition::DispatchNotRecorded, false);
    let cancelled = known(port.why_held(702));
    assert_eq!(cancelled.stage(), ActionState::Cancelled);
    recorded(&cancelled, FileHoldCondition::RecordedStageAuthorized, false);
    recorded(&cancelled, FileHoldCondition::DispatchNotRecorded, true);
    assert_eq!(inspection.executions, 1);
    assert_eq!(inspection.payload, b"ok");
    assert_eq!(inspection.control.ledger.charged, 8);
    assert_eq!(supervisor.host().unwrap().inspect(), inspection);
    assert_eq!(root.files(), files);
}

#[test]
fn shared_grants_charge_lookups_and_preserve_owner_scope_and_revocation() {
    let root = Root::new();
    let mut host = owner(&root);
    host.submit_request(host.revision(), 701, spec(b"ok"), snapshot())
        .unwrap();
    let (actor, supervisor) = host.into_actor_gateway();
    for queries in [0, MAX_HOLD_DIAGNOSIS_QUERIES + 1] {
        assert!(supervisor.hold_diagnosis_port(queries).is_err());
    }
    let (port, grant) = supervisor.hold_diagnosis_port(3).unwrap();
    assert!(port.belongs_to(&actor));
    assert_eq!(port.why_held(0), Err(ActorError::MalformedProposal));
    assert_eq!(grant.remaining_queries(), 3);
    let mut wrong_scope = port.clone();
    wrong_scope.scope.branch += 1;
    assert!(!wrong_scope.belongs_to(&actor));
    unknown(wrong_scope.why_held(701));
    assert_eq!(grant.remaining_queries(), 2);
    known(port.clone().why_held(701));
    unknown(port.why_held(999));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(port.clone().why_held(701), Err(ActorError::Capacity));
    let (revoked, custody) = supervisor.hold_diagnosis_port(1).unwrap();
    let clone = revoked.clone();
    custody.revoke();
    for closed in [revoked, clone] {
        assert_eq!(
            closed.why_held(701),
            Ok(Knowledge::Withheld {
                authority_required: "supervisor_hold_diagnosis",
            })
        );
    }
    let other = Root::new();
    let (foreign, _supervisor) = owner(&other).into_actor_gateway();
    assert!(!port.belongs_to(&foreign));
}

#[test]
fn lost_owner_releases_authority_and_cannot_be_replaced_by_equal_scope_reopen() {
    let root = Root::new();
    let mut host = owner(&root);
    host.submit_request(host.revision(), 701, spec(b"ok"), snapshot())
        .unwrap();
    let (actor, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.hold_diagnosis_port(2).unwrap();
    known(port.why_held(701));
    drop(supervisor);
    assert!(port.owner.upgrade().is_none());
    let files = root.files();
    unknown(port.why_held(701));
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(root.files(), files);
    let (reopened, _reviewer) = FileOversight::open(root.store(), profile()).unwrap();
    let (new_actor, new_supervisor) = reopened.into_actor_gateway();
    assert!(port.belongs_to(&actor));
    assert!(!port.belongs_to(&new_actor));
    let (fresh, _custody) = new_supervisor.hold_diagnosis_port(1).unwrap();
    let value = known(fresh.why_held(701));
    assert_eq!(value.stage(), ActionState::Cancelled);
    recorded(&value, FileHoldCondition::RecordedPolicyEpochMatches, false);
    unknown(port.why_held(701));
    assert_eq!(grant.remaining_queries(), 1);
}

#[test]
fn borrow_conflict_and_original_storage_failure_never_return_cached_known_data() {
    let root = Root::new();
    let mut host = owner(&root);
    host.submit_request(host.revision(), 701, spec(b"ok"), snapshot())
        .unwrap();
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.hold_diagnosis_port(3).unwrap();
    known(port.why_held(701));
    {
        let _exclusive = supervisor.owner.borrow_mut();
        unknown(port.why_held(701));
        assert_eq!(grant.remaining_queries(), 2);
        assert!(matches!(
            supervisor.hold_diagnosis_port(1),
            Err(JournalError::Unavailable)
        ));
    }
    known(port.why_held(701));
    std::fs::write(root.store().join("delivery.pending"), b"occupied stage").unwrap();
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        assert!(host.observe_time(revision, ElapsedTick(2)).is_err());
        assert!(host.storage_failure().is_some());
    }
    let files = root.files();
    unknown(port.clone().why_held(701));
    assert_eq!(grant.remaining_queries(), 1);
    assert!(matches!(
        supervisor.hold_diagnosis_port(1),
        Err(JournalError::Unavailable)
    ));
    assert_eq!(root.files(), files);
}

#[test]
fn acknowledged_diagnosis_never_reads_or_repairs_changed_canonical_bytes() {
    let root = Root::new();
    let mut host = owner(&root);
    host.submit_request(host.revision(), 701, spec(b"ok"), snapshot())
        .unwrap();
    let (_, supervisor) = host.into_actor_gateway();
    let (port, _grant) = supervisor.hold_diagnosis_port(1).unwrap();
    let inspection = supervisor.host().unwrap().inspect();
    let canonical = root.store().join("delivery.bin");
    let external_bytes = b"changed outside the acknowledged original owner";
    std::fs::write(&canonical, external_bytes).unwrap();
    let value = known(port.why_held(701));
    assert_eq!(value.basis().journal_revision, inspection.revision);
    assert_eq!(value.stage(), ActionState::Reviewing);
    assert_eq!(supervisor.host().unwrap().inspect(), inspection);
    assert_eq!(std::fs::read(canonical).unwrap(), external_bytes);
}

mod numerical;
