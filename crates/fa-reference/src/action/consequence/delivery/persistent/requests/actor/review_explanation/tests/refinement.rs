//! Real retained residual promotion and later independent authorization. The
//! synthetic coefficients/ballots test plumbing, not learned detector quality.
use super::*;
use crate::action::consequence::activation::probe::learned::KvRow;
use crate::action::consequence::activation::tensor::kv::experiment::KvSide;
use crate::action::consequence::delivery::persistent::observed::decoder::learned::sidecar::{
    FileLearnedSidecar, FileLearnedSidecarFinish,
};
use crate::action::consequence::delivery::persistent::observed::replay::review_replay_event_attempts;
use crate::action::consequence::oversight::{
    learned_host::sidecar::LearnedSidecarRequest,
    sidecar::{SidecarCongressBudget, SidecarIdentity},
};

mod model;

fn group() -> KvGroup {
    KvGroup {
        row: KvRow {
            layer: 1,
            side: KvSide::Value,
            position: 1,
        },
        head: 0,
    }
}

fn prepared() -> (Root, FileOversight, FileHumanReviewer, FileLearnedSidecar) {
    let root = Root::new();
    let (mut host, reviewer) = FileOversight::create(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.enable_learned_generation(host.revision(), model::config(false, 1))
        .unwrap();
    for _ in 0..2 {
        let n = host.learned_generation_inspection().unwrap().numerical;
        host.advance_learned_generation(host.revision(), n.actor_revision, n.position)
            .unwrap()
            .unwrap();
    }
    let attempt = admit(&mut host, 901, b"ok");
    let actor_revision = host
        .learned_generation_inspection()
        .unwrap()
        .numerical
        .actor_revision;
    let handle = host
        .begin_learned_sidecar_plan(
            host.revision(),
            attempt,
            actor_revision,
            LearnedSidecarRequest {
                identity: SidecarIdentity {
                    object_id: 91,
                    generation: 1,
                    transform_id: 7,
                },
                priority: vec![group()],
                budget: SidecarCongressBudget {
                    rounds: 2,
                    ..SidecarCongressBudget::default()
                },
            },
        )
        .unwrap();
    (root, host, reviewer, handle)
}

fn ballot(host: &mut FileOversight, handle: &FileLearnedSidecar, round: u64, vote: Verdict) {
    host.begin_review(
        host.revision(),
        handle.attempt(),
        round,
        ROOT,
        ReviewWindow {
            commit_by: ElapsedTick(5),
            reveal_by: ElapsedTick(10),
        },
        snapshot(),
    )
    .unwrap();
    votes(host, round, vote);
}

fn finish(
    host: &mut FileOversight,
    handle: &mut FileLearnedSidecar,
    round: u64,
    vote: Verdict,
) -> FileLearnedSidecarFinish {
    ballot(host, handle, round, vote);
    host.finish_learned_sidecar_review(host.revision(), handle, round, snapshot())
        .unwrap()
}

fn refined(result: FileCompletedReviewExplanationResult) -> FileRefinementExplanation {
    let Ok(Knowledge::Known {
        value: FileCompletedReviewExplanation::Refined(value),
        basis,
    }) = result
    else {
        panic!("expected original completed refinement: {result:?}");
    };
    assert_eq!(value.basis(), basis);
    value
}

fn unknown_completion(result: FileCompletedReviewExplanationResult) {
    assert_eq!(
        result,
        Ok(Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        })
    );
}

#[test]
fn completed_refinement_preserves_original_archive_and_never_creates_an_application() {
    let (root, mut host, _reviewer, mut handle) = prepared();
    let sequence = host.inspect().control.sequence;
    let original = finish(&mut host, &mut handle, 101, Verdict::Abstain);
    let FileLearnedSidecarFinish::Refined {
        group: promoted,
        input_revision,
        archive,
    } = original
    else {
        panic!("fixture must complete a real refinement");
    };
    assert_eq!(promoted, group());
    assert_eq!(input_revision, 2);
    assert_eq!(host.inspect().control.sequence, sequence);
    let inputs = host.current_learned_sidecar(&handle).unwrap().clone();
    assert!(
        host.authorize(host.revision(), handle.attempt(), &inputs, snapshot())
            .is_err()
    );
    let events = host.revision() as usize;
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .review_explanation_port_with_numerical_budget(1, events, events)
        .unwrap();
    let before = supervisor.host().unwrap().inspect();
    let numerical = supervisor
        .host()
        .unwrap()
        .learned_generation_inspection()
        .unwrap();
    let files = root.files();
    let attempts = review_replay_event_attempts();
    let value = refined(port.explain_review_completion(901, 101));
    assert_eq!(review_replay_event_attempts() - attempts, events);
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(value.group(), promoted);
    assert_eq!(value.reviewed_input_revision(), archive.input_revision);
    assert_eq!(value.input_revision(), 1);
    assert_eq!(value.refined_input_revision(), input_revision);
    assert_eq!(value.evaluation(), &archive.policy.evaluation);
    assert_eq!(value.decision(), &archive.policy.decision);
    assert_eq!(value.tally(), &archive.policy.tally);
    assert_eq!(value.commits(), archive.policy.transcript.commits);
    assert_eq!(value.commit_times(), archive.commit_times);
    assert_eq!(value.abstained(), &["r".to_owned()]);
    assert!(value.missing().is_empty());
    assert_eq!(
        value.reveals().collect::<Vec<_>>(),
        vec![("r", Verdict::Abstain, ElapsedTick(1))]
    );
    assert_eq!(value.completed_state(), ActionState::Reviewing);
    assert_eq!(value.ending_state(), ActionState::Reviewing);
    assert_eq!(value.basis().begin_control_sequence, sequence);
    assert_eq!(value.basis().completed_control_sequence, sequence);
    assert_eq!(value.basis().journal_revision, events as u64);
    assert!(!value.effect().selected_review_dispatched());
    assert!(value.effect().dispatch().is_none());
    assert!(value.effect().outcome().is_none());
    assert!(value.effect().reconciliation().is_none());
    let clone = value.clone();
    assert!(Rc::ptr_eq(&value.replay, &clone.replay));
    let debug = format!("{value:?}");
    assert!(!debug.contains("private-reveal-salt"));
    assert!(!debug.contains("unselected private source"));
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(
        supervisor
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap(),
        numerical
    );
    assert_eq!(root.files(), files);
}

#[test]
fn refinement_uses_same_precharged_allowance_and_legacy_selector_remains_applied_only() {
    let (root, mut host, _, mut handle) = prepared();
    finish(&mut host, &mut handle, 101, Verdict::Abstain);
    let events = host.revision() as usize;
    let (_, supervisor) = host.into_actor_gateway();
    let (ordinary, ordinary_grant) = supervisor.review_explanation_port(1).unwrap();
    let (short, short_grant) = supervisor
        .review_explanation_port_with_numerical_budget(1, events, events - 1)
        .unwrap();
    let (port, grant) = supervisor
        .review_explanation_port_with_numerical_budget(3, events, events * 2)
        .unwrap();
    let files = root.files();
    let attempts = review_replay_event_attempts();
    unknown_completion(ordinary.explain_review_completion(901, 101));
    unknown_completion(short.explain_review_completion(901, 101));
    assert_eq!(review_replay_event_attempts(), attempts);
    assert_eq!(ordinary_grant.remaining_queries(), 0);
    assert_eq!(short_grant.remaining_queries(), 0);
    assert_eq!(
        short_grant.remaining_numerical_replay_events(),
        Some(events - 1)
    );
    unknown(port.explain_review(901, 101));
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(events));
    let attempted = review_replay_event_attempts();
    refined(port.clone().explain_review_completion(901, 101));
    assert_eq!(review_replay_event_attempts() - attempted, events);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    unknown_completion(port.explain_review_completion(901, 101));
    assert_eq!(review_replay_event_attempts() - attempted, events);
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(
        port.explain_review_completion(901, 101),
        Err(ActorError::Capacity)
    );
    assert_eq!(root.files(), files);
}

#[test]
fn later_applied_review_owns_actual_publication_without_relabeling_refinement() {
    let (root, mut host, reviewer, mut handle) = prepared();
    finish(&mut host, &mut handle, 101, Verdict::Abstain);
    let refinement_revision = host.revision();
    let action = host.request_action(901).unwrap().clone();
    let inputs = host.current_learned_sidecar(&handle).unwrap().clone();
    assert!(
        host.authorize(host.revision(), handle.attempt(), &inputs, snapshot())
            .is_err()
    );
    assert!(matches!(
        finish(&mut host, &mut handle, 102, Verdict::Allow),
        FileLearnedSidecarFinish::Applied { receipt: Ok(_), .. }
    ));
    let applied_revision = host.revision();
    let automatic = host
        .authorize(host.revision(), handle.attempt(), &inputs, snapshot())
        .unwrap();
    let request = host
        .request_human_approval(
            host.revision(),
            1001,
            handle.attempt(),
            &inputs,
            ElapsedTick(10),
        )
        .unwrap();
    let revision = host.revision();
    let human = reviewer.approve(&mut host, revision, &request).unwrap();
    host.dispatch(
        host.revision(),
        &automatic,
        &human,
        &action,
        &inputs,
        snapshot(),
    )
    .unwrap();
    let result = host
        .publish_checked(
            host.revision(),
            handle.attempt(),
            Some(&inputs),
            snapshot(),
            ElapsedTick(2),
        )
        .unwrap();
    assert!(matches!(result.outcome, EndpointOutcome::Executed { .. }));
    host.reconcile(host.revision(), handle.attempt()).unwrap();
    let events = host.revision() as usize;
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .review_explanation_port_with_numerical_budget(3, events, events * 3)
        .unwrap();
    let before = supervisor.host().unwrap().inspect();
    let numerical = supervisor
        .host()
        .unwrap()
        .learned_generation_inspection()
        .unwrap();
    let files = root.files();
    let value = refined(port.explain_review_completion(901, 101));
    assert_eq!(value.basis().completed_revision, refinement_revision);
    assert_eq!(value.completed_state(), ActionState::Reviewing);
    assert_eq!(value.ending_state(), ActionState::Confirmed);
    assert_eq!(value.reviewed_input_revision(), 1);
    assert_eq!(value.refined_input_revision(), 2);
    assert!(!value.effect().selected_review_dispatched());
    let dispatch = value.effect().dispatch().unwrap();
    assert_eq!(dispatch.authorization().round(), 102);
    assert_eq!(dispatch.authorization().review_revision(), applied_revision);
    assert_eq!(dispatch.authorization().input_revision(), 2);
    assert!(matches!(
        value.effect().outcome().unwrap().outcome(),
        EndpointOutcome::Executed { .. }
    ));
    assert!(value.effect().reconciliation().unwrap().result().is_ok());
    let legacy = known(port.explain_review(901, 102));
    let Ok(Knowledge::Known {
        value: FileCompletedReviewExplanation::Applied(applied),
        ..
    }) = port.explain_review_completion(901, 102)
    else {
        panic!("original later application must retain its completion kind");
    };
    assert_eq!(applied, legacy);
    assert!(applied.effect().selected_review_dispatched());
    assert_eq!(applied.application().unwrap().input_revision, 2);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(
        supervisor
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap(),
        numerical
    );
    assert_eq!(root.files(), files);
}

#[test]
fn refinement_rejects_wrong_request_after_one_charged_replay_and_never_uses_a_cached_cut() {
    let (root, mut host, _, mut handle) = prepared();
    finish(&mut host, &mut handle, 101, Verdict::Abstain);
    admit(&mut host, 902, b"ok");
    let events = host.revision() as usize;
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .review_explanation_port_with_numerical_budget(4, events, events * 2)
        .unwrap();
    let attempts = review_replay_event_attempts();
    unknown_completion(port.explain_review_completion(902, 101));
    assert_eq!(review_replay_event_attempts() - attempts, events);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(events));
    refined(port.explain_review_completion(901, 101));
    assert_eq!(review_replay_event_attempts() - attempts, events * 2);
    assert_eq!(grant.remaining_queries(), 2);
    std::fs::write(root.store().join("delivery.pending"), b"occupied stage").unwrap();
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        assert!(host.observe_time(revision, ElapsedTick(2)).is_err());
        assert!(host.storage_failure().is_some());
    }
    let files = root.files();
    unknown_completion(port.explain_review_completion(901, 101));
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(review_replay_event_attempts() - attempts, events * 2);
    assert_eq!(root.files(), files);
}

#[test]
fn completion_selection_preserves_malformed_busy_and_revoked_custody_boundaries() {
    let (_root, mut host, _, mut handle) = prepared();
    finish(&mut host, &mut handle, 101, Verdict::Abstain);
    let events = host.revision() as usize;
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .review_explanation_port_with_numerical_budget(2, events, events * 2)
        .unwrap();
    let attempts = review_replay_event_attempts();
    for (request, round) in [(0, 101), (901, 0)] {
        assert_eq!(
            port.explain_review_completion(request, round),
            Err(ActorError::MalformedProposal)
        );
    }
    {
        let _busy = supervisor.host_mut().unwrap();
        unknown_completion(port.explain_review_completion(901, 101));
    }
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(events * 2));
    assert_eq!(review_replay_event_attempts(), attempts);
    unknown_completion(port.explain_review_completion(901, 999));
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(events * 2));
    assert_eq!(review_replay_event_attempts(), attempts);
    let clone = port.clone();
    grant.revoke();
    assert!(matches!(
        clone.explain_review_completion(901, 101),
        Ok(Knowledge::Withheld {
            authority_required: "supervisor_review_explanation"
        })
    ));
    assert_eq!(review_replay_event_attempts(), attempts);
}
