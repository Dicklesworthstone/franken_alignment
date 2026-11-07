//! Original durable owners and causal counterparts. These tests establish
//! historical exact-policy disclosure, not current permission or model quality.
use super::*;
use crate::ReadWitness;
use crate::action::consequence::delivery::persistent::governance::PolicyUpdate;
use crate::action::consequence::gate::containment::session::policy::{Predicate, Truth};

mod fixture;
use fixture::*;

#[test]
fn admitted_and_denied_requests_preserve_original_predicates_without_live_reads() {
    for (payload, truth, state) in [
        (b"ok".as_slice(), Truth::Satisfied, ActionState::Reviewing),
        (b"blocked".as_slice(), Truth::Violated, ActionState::Denied),
    ] {
        let root = Root::new();
        let (host, source) = source_owner(&root);
        let (actor, mut supervisor) = host.into_actor_gateway();
        let revision = supervisor.host().unwrap().revision();
        supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
        actor.submit(901, &proposal(payload)).unwrap();
        let expected = supervisor
            .host()
            .unwrap()
            .investigate_proposal(
                1,
                InterventionScope::new(901, false, false, false, &[]).unwrap(),
            )
            .unwrap();
        let original = expected.origin();
        let (port, grant) = supervisor.explanation_port(3).unwrap();
        let reads = source.status().read_attempts;
        // The historical read still succeeds when the actual required source
        // is unavailable; it cannot silently refresh or replace the old facts.
        std::fs::remove_file(root.source()).unwrap();
        let before = supervisor.host().unwrap().inspect();
        let files = root.files();
        supervisor
            .set_snapshot(before.revision, Some(snapshot()))
            .unwrap();
        let explained = known(port.explain_request(901));
        assert_eq!(explained.policy(), original.proposal().policy.as_ref());
        assert_eq!(explained.evaluation(), &original.proposal().evaluation);
        assert_eq!(explained.evaluation().result(), truth);
        assert_eq!(explained.initial_state(), state);
        assert_eq!(explained.ending_state(), state);
        assert_eq!(explained.proposal_policy_epoch(), 0);
        assert_eq!(explained.semantic_epoch(), 17);
        assert_eq!(explained.target(), Some(profile().delivery.target));
        assert_eq!(explained.units(), 8);
        assert_eq!(explained.payload_bytes(), payload.len());
        assert_eq!(
            explained.evaluation().witnesses(),
            &[ReadWitness::Exact {
                key: 7,
                value: Some(b"ok".to_vec()),
            }]
        );
        let basis = explained.basis();
        assert_eq!((basis.request, basis.attempt), (901, 1));
        assert_eq!(basis.scope, profile().delivery.scope);
        assert_eq!(
            basis.admission_revision,
            original.admission_snapshot().revision
        );
        assert_eq!(
            basis.admission_control_sequence,
            original.admission_snapshot().control.sequence
        );
        assert_eq!(basis.admitted_at, Some(ElapsedTick(1)));
        assert_eq!(basis.journal_revision, original.journal_snapshot().revision);
        assert_eq!(
            basis.control_sequence,
            original.journal_snapshot().control.sequence
        );
        assert_eq!(basis.recorded_at, Some(ElapsedTick(1)));
        assert_eq!(known(port.clone().explain_request(901)), explained);
        assert_eq!(grant.remaining_queries(), 1);
        assert_eq!(source.status().read_attempts, reads);
        assert_eq!(supervisor.host().unwrap().inspect(), before);
        assert_eq!(root.files(), files);
        // Explanation did not consume the original one-use admission snapshot.
        actor.submit(902, &proposal(b"ok")).unwrap();
        assert!(matches!(
            supervisor
                .host()
                .unwrap()
                .request_status(902)
                .unwrap()
                .disposition,
            FileRequestDisposition::Admitted { attempt: 2, .. }
        ));
    }
}

#[test]
fn original_admission_survives_later_cancellation_policy_and_clock_changes() {
    let root = Root::new();
    let mut host = owner(&root);
    host.submit_request(host.revision(), 901, spec(b"ok"), snapshot())
        .unwrap();
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, _grant) = supervisor.explanation_port(2).unwrap();
    let admission = known(port.explain_request(901));
    let new_policy = Policy::new(2, vec![Predicate::PayloadAtMost(1)]).unwrap();
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.cancel_request(revision, 901).unwrap();
        let inspection = host.inspect();
        let update = PolicyUpdate::new(
            61,
            inspection.control.sequence,
            inspection.control.ledger.epoch,
            new_policy.clone(),
        )
        .unwrap();
        let revision = host.revision();
        host.replace_policy(revision, &update).unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(5)).unwrap();
    }
    let before = supervisor.host().unwrap().inspect();
    let files = root.files();
    let explained = known(port.explain_request(901));
    assert_eq!(explained.policy(), admission.policy());
    assert_eq!(explained.evaluation(), admission.evaluation());
    assert_ne!(explained.policy(), &new_policy);
    assert_eq!(explained.initial_state(), ActionState::Reviewing);
    assert_eq!(explained.ending_state(), ActionState::Cancelled);
    assert_eq!(explained.evaluation().result(), Truth::Satisfied);
    assert_eq!(
        explained.basis().admission_revision,
        admission.basis().admission_revision
    );
    assert_eq!(explained.basis().admitted_at, Some(ElapsedTick(1)));
    assert_eq!(explained.basis().recorded_at, Some(ElapsedTick(5)));
    assert!(explained.basis().admission_revision < explained.basis().journal_revision);
    assert_eq!(explained.basis().journal_revision, before.revision);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), files);
}

#[test]
fn exact_request_mapping_never_substitutes_an_equal_numbered_attempt() {
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
    let admitted = host
        .submit_request(host.revision(), 2, spec(b"ok"), snapshot())
        .unwrap();
    assert!(matches!(
        admitted.disposition,
        FileRequestDisposition::Admitted { attempt: 3, .. }
    ));
    let mut stale_target = spec(b"ok");
    stale_target.target.as_mut().unwrap().expected_version = 2;
    assert!(matches!(
        host.submit_request(host.revision(), 903, stale_target, snapshot())
            .unwrap()
            .disposition,
        FileRequestDisposition::NotAdmitted(_)
    ));
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.explanation_port(5).unwrap();
    let before = supervisor.host().unwrap().inspect();
    let files = root.files();
    let denied = known(port.explain_request(901));
    assert_eq!((denied.basis().request, denied.basis().attempt), (901, 2));
    assert_eq!(denied.evaluation().result(), Truth::Violated);
    let admitted = known(port.explain_request(2));
    assert_eq!((admitted.basis().request, admitted.basis().attempt), (2, 3));
    assert_eq!(admitted.evaluation().result(), Truth::Satisfied);
    unknown(port.explain_request(1)); // A direct attempt with this ID exists.
    unknown(port.explain_request(903)); // Retained refusal, no frozen proposal.
    unknown(port.explain_request(999));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(port.explain_request(901), Err(ActorError::Capacity));
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), files);
}

#[test]
fn equal_denial_states_retain_different_original_failing_predicates() {
    let root = Root::new();
    let mut host = owner(&root);
    host.submit_request(host.revision(), 901, spec(b"blocked"), snapshot())
        .unwrap();
    let mut changed_source = snapshot();
    changed_source.values.insert(7, b"changed".to_vec());
    host.submit_request(host.revision(), 902, spec(b"ok"), changed_source)
        .unwrap();
    let (_, supervisor) = host.into_actor_gateway();
    let (port, _grant) = supervisor.explanation_port(2).unwrap();
    let payload_denial = known(port.explain_request(901));
    let source_denial = known(port.explain_request(902));
    assert_eq!(payload_denial.initial_state(), ActionState::Denied);
    assert_eq!(source_denial.initial_state(), ActionState::Denied);
    assert_eq!(payload_denial.policy(), source_denial.policy());
    let truths = |value: &FileAdmissionExplanation| {
        value
            .evaluation()
            .trace()
            .iter()
            .map(|step| step.result)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        truths(&payload_denial),
        [Truth::Satisfied, Truth::Violated, Truth::Violated]
    );
    assert_eq!(
        truths(&source_denial),
        [Truth::Violated, Truth::Satisfied, Truth::Violated]
    );
    assert_ne!(
        payload_denial.evaluation().witnesses(),
        source_denial.evaluation().witnesses()
    );
}

#[test]
fn acknowledged_owner_explanation_never_reopens_a_changed_canonical_image() {
    let root = Root::new();
    let mut host = owner(&root);
    host.submit_request(host.revision(), 901, spec(b"ok"), snapshot())
        .unwrap();
    let (_, supervisor) = host.into_actor_gateway();
    let (port, _grant) = supervisor.explanation_port(1).unwrap();
    let before = supervisor.host().unwrap().inspect();
    let canonical = root.store().join("delivery.bin");
    let external_bytes = b"external corruption, never a historical owner input";
    std::fs::write(&canonical, external_bytes).unwrap();
    let explained = known(port.explain_request(901));
    assert_eq!(explained.evaluation().result(), Truth::Satisfied);
    assert_eq!(explained.basis().journal_revision, before.revision);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(std::fs::read(canonical).unwrap(), external_bytes);
}

#[test]
fn shared_queries_revocation_scope_and_borrow_conflicts_keep_exact_custody() {
    let root = Root::new();
    let mut host = owner(&root);
    host.submit_request(host.revision(), 901, spec(b"ok"), snapshot())
        .unwrap();
    let (actor, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.explanation_port(3).unwrap();
    let clone = port.clone();
    assert!(port.belongs_to(&actor));
    assert_eq!(port.explain_request(0), Err(ActorError::MalformedProposal));
    assert_eq!(grant.remaining_queries(), 3);
    {
        let _exclusive = supervisor.owner.borrow_mut();
        unknown(port.explain_request(901));
        assert_eq!(grant.remaining_queries(), 3);
        assert!(matches!(
            supervisor.explanation_port(1),
            Err(JournalError::Unavailable)
        ));
    }
    let other_root = Root::new();
    let (other_actor, _other_supervisor) = owner(&other_root).into_actor_gateway();
    assert!(!port.belongs_to(&other_actor)); // Identical numeric scope, different original owner.
    let mut wrong_scope = port.clone();
    wrong_scope.scope.branch += 1;
    assert!(!wrong_scope.belongs_to(&actor));
    unknown(wrong_scope.explain_request(901));
    assert_eq!(grant.remaining_queries(), 2);
    known(clone.explain_request(901));
    assert_eq!(grant.remaining_queries(), 1);
    grant.revoke();
    for port in [&port, &clone] {
        assert_eq!(
            port.explain_request(901),
            Ok(Knowledge::Withheld {
                authority_required: "supervisor_explanation"
            })
        );
    }
}

#[test]
fn event_budget_applies_to_whole_current_suffix_and_is_spent_on_unknown() {
    let root = Root::new();
    let mut host = owner(&root);
    host.submit_request(host.revision(), 901, spec(b"ok"), snapshot())
        .unwrap();
    let (_, mut supervisor) = host.into_actor_gateway();
    let admitted_events = supervisor.host().unwrap().revision() as usize;
    let (port, grant) = supervisor
        .explanation_port_with_budget(2, admitted_events)
        .unwrap();
    assert_eq!(grant.max_replay_events(), admitted_events);
    known(port.explain_request(901));
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(2)).unwrap();
    }
    let before = supervisor.host().unwrap().inspect();
    let files = root.files();
    unknown(port.explain_request(901));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(port.explain_request(901), Err(ActorError::Capacity));
    let (permitted, _grant) = supervisor
        .explanation_port_with_budget(1, admitted_events + 1)
        .unwrap();
    assert_eq!(
        known(permitted.explain_request(901))
            .basis()
            .journal_revision,
        before.revision
    );
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), files);
}

#[test]
fn dropped_original_owner_releases_lock_and_cannot_adopt_a_reopened_owner() {
    let root = Root::new();
    let mut host = owner(&root);
    host.submit_request(host.revision(), 901, spec(b"ok"), snapshot())
        .unwrap();
    let (actor, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.explanation_port(2).unwrap();
    known(port.explain_request(901));
    drop(supervisor);
    unknown(port.explain_request(901));
    assert_eq!(grant.remaining_queries(), 1);
    let (reopened, _reviewer) = FileOversight::open(root.store(), profile()).unwrap();
    let (new_actor, new_supervisor) = reopened.into_actor_gateway();
    assert!(!port.belongs_to(&new_actor));
    assert!(port.belongs_to(&actor)); // Identity alone never upgrades the dead Weak owner.
    let (fresh, _fresh_grant) = new_supervisor.explanation_port(1).unwrap();
    let explained = known(fresh.explain_request(901));
    assert_eq!(explained.initial_state(), ActionState::Reviewing);
    assert_eq!(explained.ending_state(), ActionState::Cancelled);
    unknown(port.explain_request(901));
    assert_eq!(grant.remaining_queries(), 1);
}

#[test]
fn invalid_grants_do_not_provision_unbounded_or_empty_allowances() {
    let root = Root::new();
    let (_, supervisor) = owner(&root).into_actor_gateway();
    for (queries, events) in [(0, 1), (1, 0)] {
        assert!(matches!(
            supervisor.explanation_port_with_budget(queries, events),
            Err(JournalError::Contract(Error::InvalidInput))
        ));
    }
    for (queries, events) in [
        (MAX_EXPLANATION_QUERIES + 1, 1),
        (1, MAX_EXPLANATION_EVENTS + 1),
    ] {
        assert!(matches!(
            supervisor.explanation_port_with_budget(queries, events),
            Err(JournalError::Contract(Error::Limit))
        ));
    }
    let (port, grant) = supervisor.explanation_port(1).unwrap();
    drop(grant);
    assert!(matches!(
        port.explain_request(901),
        Ok(Knowledge::Withheld { .. })
    ));
}

mod numerical;
