//! Real original congress/application/effect counterparts. Controlled reference
//! ballots establish plumbing and preservation, not helper honesty or quality.
use super::*;
use crate::action::consequence::delivery::persistent::governance::PolicyUpdate;
use crate::action::consequence::delivery::persistent::observed::FileHumanReviewer;
use crate::action::consequence::delivery::{EndpointOutcome, persistent::Reconciliation};
use crate::action::consequence::gate::containment::session::policy::{Predicate, Truth};
use crate::action::consequence::oversight::CommitteeInput;

mod fixture;
use fixture::*;

#[test]
fn original_completed_review_preserves_exact_trace_and_three_native_cuts_without_live_reads() {
    let root = Root::new();
    let (mut host, source) = source_owner(&root);
    let (input, receipt) = completed(&mut host, 901, 101);
    let original = host.review_replay(101).unwrap();
    let (actor, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.review_explanation_port(2).unwrap();
    let before = supervisor.host().unwrap().inspect();
    supervisor
        .set_snapshot(before.revision, Some(snapshot()))
        .unwrap();
    let reads = source.status().read_attempts;
    std::fs::remove_file(root.source()).unwrap();
    let files = root.files();
    let explained = known(port.explain_review(901, 101));
    assert_eq!(explained.policy(), &original.anchor().policy.policy);
    assert_eq!(
        explained.evaluation(),
        &original.archive().policy.evaluation
    );
    assert_eq!(explained.evaluation().result(), Truth::Satisfied);
    assert_eq!(explained.congress(), &original.anchor().policy.congress);
    assert_eq!(explained.decision(), &original.archive().policy.decision);
    assert_eq!(explained.tally(), &original.archive().policy.tally);
    assert_eq!(explained.decision().consequence, Consequence::Continue);
    assert!(explained.missing().is_empty());
    assert!(explained.abstained().is_empty());
    assert_eq!(
        explained.commits(),
        original.archive().policy.transcript.commits
    );
    assert_eq!(explained.commit_times(), original.archive().commit_times);
    assert_eq!(
        explained.reveals().collect::<Vec<_>>(),
        vec![("r", Verdict::Allow, ElapsedTick(1))]
    );
    assert_eq!(explained.window(), original.anchor().window);
    assert_eq!(explained.input_revision(), receipt.input_revision);
    assert_eq!(explained.evidence_root(), ROOT);
    assert_eq!(explained.reveals_opened_at(), ElapsedTick(1));
    assert_eq!(explained.review_completed_at(), receipt.completed_at);
    assert!(!explained.contradiction());
    assert_eq!(
        explained.expected_control_sequence(),
        original.anchor().policy.expected_control_sequence
    );
    assert_eq!(
        explained.proposal_policy_epoch(),
        input.action().spec().policy_epoch
    );
    assert_eq!(explained.semantic_epoch(), 17);
    assert_eq!(explained.target(), input.action().spec().target);
    assert_eq!(explained.units(), 8);
    assert_eq!(explained.payload_bytes(), 2);
    assert_eq!(
        explained.application(),
        Ok(FileReviewApplication {
            control_sequence: receipt.policy.control.sequence,
            input_revision: receipt.input_revision,
            consequence: Consequence::Continue,
        })
    );
    let basis = explained.basis();
    assert_eq!((basis.request, basis.attempt, basis.round), (901, 1, 101));
    assert_eq!(basis.begin_revision, original.begin_revision());
    assert_eq!(
        basis.begin_control_sequence,
        original.anchor().policy.expected_control_sequence
    );
    assert_eq!(basis.started_at, original.anchor().started_at);
    assert_eq!(
        basis.completed_revision,
        original.completed_snapshot().revision
    );
    assert_eq!(
        basis.completed_control_sequence,
        original.completed_snapshot().control.sequence
    );
    assert_eq!(
        basis.completed_at,
        original.completed_snapshot().control.ledger.elapsed
    );
    assert_eq!(basis.journal_revision, before.revision);
    assert_eq!(basis.control_sequence, before.control.sequence);
    assert_eq!(basis.recorded_at, before.control.ledger.elapsed);
    assert_eq!(explained.completed_state(), receipt.policy.control.after);
    assert_eq!(explained.ending_state(), receipt.policy.control.after);
    assert!(!explained.effect().selected_review_dispatched());
    assert!(explained.effect().dispatch().is_none());
    assert!(explained.effect().outcome().is_none());
    let clone = explained.clone();
    assert!(Rc::ptr_eq(&explained.replay, &clone.replay));
    let debug = format!("{explained:?}");
    assert!(!debug.contains("private helper input"));
    assert!(!debug.contains("private-reveal-salt"));
    assert_eq!(known(port.clone().explain_review(901, 101)), explained);
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(source.status().read_attempts, reads);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), files);
    // The read did not consume the independently installed admission snapshot.
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

#[test]
fn replayable_continue_keeps_real_application_success_and_stale_refusal_distinct() {
    for stale in [false, true] {
        let root = Root::new();
        let mut host = owner(&root);
        admit(&mut host, 901, b"ok");
        let input = begin(&mut host, 901, 101);
        votes(&mut host, 101, Verdict::Allow);
        let changed = inputs(input.action(), b"changed actual helper input");
        let applied = host
            .finish_review(
                host.revision(),
                101,
                Some(if stale { &changed } else { &input }),
                snapshot(),
            )
            .unwrap();
        if stale {
            assert_eq!(applied, Err(Error::Stale));
        } else {
            assert!(applied.is_ok());
        }
        let (_, supervisor) = host.into_actor_gateway();
        let (port, _grant) = supervisor.review_explanation_port(1).unwrap();
        let before = supervisor.host().unwrap().inspect();
        let files = root.files();
        let explained = known(port.explain_review(901, 101));
        assert_eq!(explained.decision().consequence, Consequence::Continue);
        if stale {
            assert_eq!(explained.application(), Err(Error::Stale));
            assert_eq!(before.control.ledger.reserved, 0);
        } else {
            assert_eq!(
                explained.application().unwrap().consequence,
                Consequence::Continue
            );
            assert_eq!(before.control.ledger.reserved, 8);
        }
        assert_eq!(
            explained.completed_state(),
            before.control.ledger.stages[&1]
        );
        assert!(!explained.effect().selected_review_dispatched());
        assert!(explained.effect().dispatch().is_none());
        assert_eq!(supervisor.host().unwrap().inspect(), before);
        assert_eq!(root.files(), files);
    }
}

#[test]
fn missing_and_abstaining_members_remain_distinct_original_hold_reasons() {
    for abstain in [false, true] {
        let root = Root::new();
        let mut host = owner(&root);
        admit(&mut host, 901, b"ok");
        let input = begin(&mut host, 901, 101);
        if abstain {
            votes(&mut host, 101, Verdict::Abstain);
        } else {
            host.observe_time(host.revision(), ElapsedTick(6)).unwrap();
            host.open_reveals(host.revision(), 101).unwrap();
            host.observe_time(host.revision(), ElapsedTick(11)).unwrap();
        }
        let receipt = host
            .finish_review(host.revision(), 101, Some(&input), snapshot())
            .unwrap()
            .unwrap();
        assert_eq!(
            receipt.policy.control.decision.consequence,
            Consequence::HoldEffect
        );
        let (_, supervisor) = host.into_actor_gateway();
        let (port, _grant) = supervisor.review_explanation_port(1).unwrap();
        let explained = known(port.explain_review(901, 101));
        assert_eq!(explained.decision().consequence, Consequence::HoldEffect);
        assert_eq!(explained.tally().permit_weight, 0);
        assert_eq!(explained.missing().len(), usize::from(!abstain));
        assert_eq!(explained.abstained().len(), usize::from(abstain));
        assert_eq!(explained.reveals().len(), usize::from(abstain));
        assert_eq!(
            explained.application().unwrap().consequence,
            Consequence::HoldEffect
        );
        assert!(explained.effect().outcome().is_none());
    }
}

fn publish(host: &mut FileOversight, reviewer: &FileHumanReviewer, input: &CommitteeInput) {
    let automatic = host
        .authorize(host.revision(), 1, input, snapshot())
        .unwrap();
    let human_request = host
        .request_human_approval(host.revision(), 1001, 1, input, ElapsedTick(31))
        .unwrap();
    let revision = host.revision();
    let human = reviewer.approve(host, revision, &human_request).unwrap();
    host.dispatch(
        host.revision(),
        &automatic,
        &human,
        input.action(),
        input,
        snapshot(),
    )
    .unwrap();
    assert_eq!(
        host.publish(host.revision(), 1).unwrap(),
        EndpointOutcome::Executed {
            resulting_version: 2
        }
    );
    host.reconcile(host.revision(), 1).unwrap();
}

#[test]
fn original_publication_keys_survive_explanation_and_effect_linkage_names_the_selected_review() {
    let root = Root::new();
    let (mut host, reviewer) = FileOversight::create(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let (input, _) = completed(&mut host, 901, 101);
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.review_explanation_port(3).unwrap();
    let first = known(port.explain_review(901, 101));
    assert!(first.effect().dispatch().is_none());
    {
        let mut host = supervisor.host_mut().unwrap();
        publish(&mut host, &reviewer, &input);
    }
    let before = supervisor.host().unwrap().inspect();
    let files = root.files();
    let later = known(port.explain_review(901, 101));
    assert_eq!(first.policy(), later.policy());
    assert_eq!(first.evaluation(), later.evaluation());
    assert_eq!(
        first.basis().completed_revision,
        later.basis().completed_revision
    );
    assert!(later.basis().completed_revision < later.basis().journal_revision);
    assert_eq!(later.ending_state(), ActionState::Confirmed);
    assert!(later.effect().selected_review_dispatched());
    let dispatch = later.effect().dispatch().unwrap();
    assert_eq!(dispatch.authorization().round(), 101);
    assert_eq!(
        dispatch.authorization().review_revision(),
        first.basis().completed_revision
    );
    assert_eq!(dispatch.human().request, 1001);
    assert_eq!(
        later.effect().outcome().unwrap().outcome(),
        EndpointOutcome::Executed {
            resulting_version: 2
        }
    );
    assert_eq!(
        later.effect().reconciliation().unwrap().result(),
        Ok(Reconciliation::Resolved(EndpointOutcome::Executed {
            resulting_version: 2
        }))
    );
    assert_eq!(known(port.clone().explain_review(901, 101)), later);
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), files);
    assert_eq!(before.executions, 1);
}

#[test]
fn later_review_of_same_attempt_does_not_attribute_its_dispatched_key_to_earlier_round() {
    let root = Root::new();
    let (mut host, reviewer) = FileOversight::create(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    completed(&mut host, 901, 101);
    let input = begin(&mut host, 901, 102);
    votes(&mut host, 102, Verdict::Allow);
    host.finish_review(host.revision(), 102, Some(&input), snapshot())
        .unwrap()
        .unwrap();
    publish(&mut host, &reviewer, &input);
    let (_, supervisor) = host.into_actor_gateway();
    let (port, _grant) = supervisor.review_explanation_port(2).unwrap();
    let first = known(port.explain_review(901, 101));
    let second = known(port.explain_review(901, 102));
    assert_eq!(first.effect().dispatch(), second.effect().dispatch());
    assert_eq!(
        first.effect().dispatch().unwrap().authorization().round(),
        102
    );
    assert!(!first.effect().selected_review_dispatched());
    assert!(second.effect().selected_review_dispatched());
    assert_eq!(first.effect().outcome(), second.effect().outcome());
}

#[test]
fn later_policy_and_clock_changes_preserve_original_congress_and_application() {
    let root = Root::new();
    let mut host = owner(&root);
    completed(&mut host, 901, 101);
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, _grant) = supervisor.review_explanation_port(2).unwrap();
    let first = known(port.explain_review(901, 101));
    let changed = Policy::new(2, vec![Predicate::PayloadAtMost(1)]).unwrap();
    {
        let mut host = supervisor.host_mut().unwrap();
        let state = host.inspect();
        let update = PolicyUpdate::new(
            44,
            state.control.sequence,
            state.control.ledger.epoch,
            changed.clone(),
        )
        .unwrap();
        let revision = host.revision();
        host.replace_policy(revision, &update).unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(5)).unwrap();
    }
    let later = known(port.explain_review(901, 101));
    assert_eq!(later.policy(), first.policy());
    assert_ne!(later.policy(), &changed);
    assert_eq!(later.congress(), first.congress());
    assert_eq!(later.evaluation(), first.evaluation());
    assert_eq!(later.application(), first.application());
    assert_eq!(
        later.basis().completed_revision,
        first.basis().completed_revision
    );
    assert_eq!(later.basis().started_at, ElapsedTick(1));
    assert_eq!(later.basis().recorded_at, Some(ElapsedTick(5)));
    assert!(later.basis().journal_revision > first.basis().journal_revision);
}

#[test]
fn exact_request_and_round_binding_never_substitutes_equal_numbered_attempts_or_other_reviews() {
    let root = Root::new();
    let mut host = owner(&root);
    host.propose(host.revision(), 1, spec(b"ok"), snapshot())
        .unwrap();
    completed(&mut host, 901, 101);
    assert_eq!(admit(&mut host, 2, b"ok"), 3);
    let input = begin(&mut host, 2, 102);
    votes(&mut host, 102, Verdict::Allow);
    host.finish_review(host.revision(), 102, Some(&input), snapshot())
        .unwrap()
        .unwrap();
    admit(&mut host, 903, b"blocked");
    let mut stale = spec(b"ok");
    stale.target.as_mut().unwrap().expected_version = 99;
    assert!(matches!(
        host.submit_request(host.revision(), 904, stale, snapshot())
            .unwrap()
            .disposition,
        FileRequestDisposition::NotAdmitted(_)
    ));
    admit(&mut host, 905, b"ok");
    begin(&mut host, 905, 105); // Deliberately unfinished original review.
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.review_explanation_port(10).unwrap();
    assert_eq!(known(port.explain_review(901, 101)).basis().attempt, 2);
    assert_eq!(known(port.explain_review(2, 102)).basis().attempt, 3);
    for (request, round) in [
        (1, 101),
        (901, 102),
        (2, 101),
        (903, 101),
        (904, 101),
        (999, 101),
        (905, 105),
        (901, 999),
    ] {
        unknown(port.explain_review(request, round));
    }
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(port.explain_review(901, 101), Err(ActorError::Capacity));
}

#[test]
fn shared_budget_revocation_foreign_owner_and_exclusive_borrow_keep_original_custody() {
    let root = Root::new();
    let mut host = owner(&root);
    completed(&mut host, 901, 101);
    let (actor, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.review_explanation_port(3).unwrap();
    let clone = port.clone();
    assert!(port.belongs_to(&actor));
    for (request, round) in [(0, 101), (901, 0)] {
        assert_eq!(
            port.explain_review(request, round),
            Err(ActorError::MalformedProposal)
        );
    }
    assert_eq!(grant.remaining_queries(), 3);
    {
        let _borrow = supervisor.owner.borrow_mut();
        unknown(port.explain_review(901, 101));
        assert_eq!(grant.remaining_queries(), 3);
        assert!(matches!(
            supervisor.review_explanation_port(1),
            Err(JournalError::Unavailable)
        ));
    }
    let other = Root::new();
    let (other_actor, _other_supervisor) = owner(&other).into_actor_gateway();
    assert!(!port.belongs_to(&other_actor));
    let mut different_scope = port.clone();
    different_scope.scope.branch += 1;
    assert!(!different_scope.belongs_to(&actor));
    unknown(different_scope.explain_review(901, 101));
    known(clone.explain_review(901, 101));
    assert_eq!(grant.remaining_queries(), 1);
    grant.revoke();
    for selected in [&port, &clone] {
        assert_eq!(
            selected.explain_review(901, 101),
            Ok(Knowledge::Withheld {
                authority_required: "supervisor_review_explanation",
            })
        );
    }
}

#[test]
fn whole_later_suffix_must_fit_without_refund_or_prefix_explanation() {
    let root = Root::new();
    let mut host = owner(&root);
    completed(&mut host, 901, 101);
    let (_, mut supervisor) = host.into_actor_gateway();
    let events = supervisor.host().unwrap().revision() as usize;
    let (port, grant) = supervisor
        .review_explanation_port_with_budget(2, events)
        .unwrap();
    assert_eq!(grant.max_replay_events(), events);
    known(port.explain_review(901, 101));
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(2)).unwrap();
    }
    let before = supervisor.host().unwrap().inspect();
    let files = root.files();
    unknown(port.explain_review(901, 101));
    assert_eq!(grant.remaining_queries(), 0);
    let (permitted, _grant) = supervisor
        .review_explanation_port_with_budget(1, events + 1)
        .unwrap();
    assert_eq!(
        known(permitted.explain_review(901, 101))
            .basis()
            .journal_revision,
        before.revision
    );
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), files);
}

#[test]
fn dead_owner_cannot_adopt_reopened_history_or_keep_the_original_writer_lock() {
    let root = Root::new();
    let mut host = owner(&root);
    completed(&mut host, 901, 101);
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.review_explanation_port(2).unwrap();
    let original = known(port.explain_review(901, 101));
    drop(supervisor);
    unknown(port.explain_review(901, 101));
    assert_eq!(grant.remaining_queries(), 1);
    let (reopened, _) = FileOversight::open(root.store(), profile()).unwrap();
    let (actor, supervisor) = reopened.into_actor_gateway();
    assert!(!port.belongs_to(&actor));
    let (fresh, _custody) = supervisor.review_explanation_port(1).unwrap();
    let explained = known(fresh.explain_review(901, 101));
    assert_eq!(explained.policy(), original.policy());
    assert_eq!(explained.application(), original.application());
    assert_eq!(
        explained.basis().completed_revision,
        original.basis().completed_revision
    );
    assert_eq!(explained.ending_state(), ActionState::Cancelled);
    unknown(port.explain_review(901, 101));
    assert_eq!(grant.remaining_queries(), 1);
}

#[test]
fn acknowledged_ram_cut_never_falls_back_to_disk_and_faulted_owner_cannot_disclose_it() {
    let root = Root::new();
    let mut host = owner(&root);
    completed(&mut host, 901, 101);
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.review_explanation_port(3).unwrap();
    let canonical = root.store().join("delivery.bin");
    let original = std::fs::read(&canonical).unwrap();
    std::fs::write(&canonical, b"external bytes not acknowledged by this owner").unwrap();
    known(port.explain_review(901, 101));
    assert_eq!(
        std::fs::read(&canonical).unwrap(),
        b"external bytes not acknowledged by this owner"
    );
    // Restore only the test's own canonical mutation before causing a real
    // storage barrier failure, which must suppress the previously valid cut.
    std::fs::write(&canonical, original).unwrap();
    std::fs::write(root.store().join("delivery.pending"), b"occupied stage").unwrap();
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        assert!(host.observe_time(revision, ElapsedTick(2)).is_err());
        assert!(host.storage_failure().is_some());
    }
    let files = root.files();
    unknown(port.explain_review(901, 101));
    assert_eq!(grant.remaining_queries(), 2);
    assert!(matches!(
        supervisor.review_explanation_port(1),
        Err(JournalError::Unavailable)
    ));
    assert_eq!(root.files(), files);
}

#[test]
fn grant_bounds_reject_empty_and_unbounded_allowances() {
    let root = Root::new();
    let (_, supervisor) = owner(&root).into_actor_gateway();
    for (queries, events) in [(0, 1), (1, 0)] {
        assert!(matches!(
            supervisor.review_explanation_port_with_budget(queries, events),
            Err(JournalError::Contract(Error::InvalidInput))
        ));
    }
    for (queries, events) in [
        (MAX_REVIEW_EXPLANATION_QUERIES + 1, 1),
        (1, MAX_REVIEW_EXPLANATION_EVENTS + 1),
    ] {
        assert!(matches!(
            supervisor.review_explanation_port_with_budget(queries, events),
            Err(JournalError::Contract(Error::Limit))
        ));
    }
    let (port, grant) = supervisor.review_explanation_port(1).unwrap();
    drop(grant);
    assert!(matches!(
        port.explain_review(901, 101),
        Ok(Knowledge::Withheld { .. })
    ));
}

mod numerical;
mod refinement;
