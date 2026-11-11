//! Original request relationships, paired with disclosure/custody/limit failures.
use super::*;
use crate::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileReviewExplanation, FileReviewExplanationResult,
};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::ReviewWindow;
use crate::action::{ActionState, ElapsedTick, ResolvedTarget};
use crate::round::Verdict;

#[path = "../review_explanation/tests/fixture.rs"]
mod fixture;
use fixture::*;

#[path = "../review_explanation/tests/refinement/model.rs"]
mod learned_model;

fn observed(result: FileRequestHistoryResult) -> FileRequestHistory {
    let Ok(Knowledge::Known { value, basis }) = result else {
        panic!("expected original request relationships: {result:?}");
    };
    assert_eq!(value.basis(), basis);
    value
}

fn grant(
    supervisor: &FileActorSupervisor<FileOversight>,
    queries: usize,
    events: usize,
    reviews: usize,
) -> (
    FileSupervisorRequestHistoryPort,
    FileSupervisorRequestHistoryGrant,
) {
    supervisor
        .request_history_port(
            queries,
            events,
            reviews,
            FileRequestHistoryDisclosure::RecordedRequestRelationships,
        )
        .unwrap()
}

#[test]
fn complete_original_relationships_keep_admission_source_and_private_bytes_unchanged() {
    let root = Root::new();
    let (mut host, source) = source_owner(&root);
    completed(&mut host, 701, 801);
    let original = host.review_replay(801).unwrap();
    let (actor, mut supervisor) = host.into_actor_gateway();
    let before = supervisor.host().unwrap().inspect();
    supervisor
        .set_snapshot(before.revision, Some(snapshot()))
        .unwrap();
    let reads = source.status().read_attempts;
    std::fs::remove_file(root.source()).unwrap();
    let files = root.files();
    let (port, custody) = grant(&supervisor, 2, before.revision as usize, 1);
    let result = observed(port.history(701));
    assert_eq!(result.attempt(), Some(1));
    assert!(result.admitted());
    assert_eq!(result.incomplete_reviews(), 0);
    assert_eq!(result.basis().request_generation, 0);
    assert_eq!(result.basis().scope, profile().delivery.scope);
    assert_eq!(result.basis().journal_revision, before.revision);
    assert_eq!(result.basis().authority_epoch, before.control.ledger.epoch);
    assert_eq!(result.basis().control_sequence, before.control.sequence);
    assert_eq!(result.basis().recorded_at, Some(ElapsedTick(1)));
    assert_eq!(result.reviews().len(), 1);
    let link = &result.reviews()[0];
    assert_eq!(link.round(), 801);
    assert_eq!(link.begin_revision(), original.begin_revision());
    assert_eq!(
        link.completed_revision(),
        original.completed_snapshot().revision
    );
    assert_eq!(
        link.completion_kind(),
        FileRequestReviewCompletion::ApplicationAttempted
    );
    assert_eq!(source.status().read_attempts, reads);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), files);
    assert_eq!(custody.remaining_queries(), 1);
    for secret in [
        "private helper input",
        "private-reveal-salt",
        "unselected private source",
        "approve?",
    ] {
        assert!(!format!("{result:?}").contains(secret));
    }
    // Discovery neither creates nor spends a fresh submission snapshot.
    actor.submit(702, &proposal(b"ok")).unwrap();
    assert!(matches!(
        supervisor
            .host()
            .unwrap()
            .request_status(702)
            .unwrap()
            .disposition,
        FileRequestDisposition::Admitted { attempt: 2, .. }
    ));
}

#[test]
fn pending_starts_unadmitted_requests_and_missing_requests_remain_distinct() {
    let root = Root::new();
    let mut host = owner(&root);
    admit(&mut host, 701, b"ok");
    let input = begin(&mut host, 701, 801);
    let denied = host
        .submit_request(host.revision(), 702, spec(b"denied"), snapshot())
        .unwrap();
    assert!(matches!(
        denied.disposition,
        FileRequestDisposition::NotAdmitted(_)
    ));
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, custody) = grant(&supervisor, 4, MAX_REQUEST_HISTORY_EVENTS, 4);
    let pending = observed(port.history(701));
    assert_eq!(pending.stage(), Some(ActionState::Reviewing));
    assert_eq!(pending.incomplete_reviews(), 1);
    assert!(pending.reviews().is_empty());
    let denied = observed(port.history(702));
    assert!(!denied.admitted());
    assert_eq!(denied.attempt(), None);
    assert_eq!(denied.stage(), None);
    assert_eq!(denied.incomplete_reviews(), 0);
    assert!(denied.reviews().is_empty());
    assert_eq!(port.history(703), Ok(super::unknown()));
    {
        let mut host = supervisor.host_mut().unwrap();
        votes(&mut host, 801, Verdict::Allow);
        let revision = host.revision();
        // A later request may stale the application; completion still happened.
        let _application = host
            .finish_review(revision, 801, Some(&input), snapshot())
            .unwrap();
    }
    let complete = observed(port.history(701));
    assert_eq!(complete.incomplete_reviews(), 0);
    assert_eq!(complete.reviews().len(), 1);
    assert_eq!(
        complete.reviews()[0].completion_kind(),
        FileRequestReviewCompletion::ApplicationAttempted
    );
    assert!(complete.basis().journal_revision > pending.basis().journal_revision);
    assert_eq!(custody.remaining_queries(), 0);
}

#[test]
fn whole_history_and_review_limits_refuse_without_returning_a_favorable_prefix() {
    let root = Root::new();
    let mut host = owner(&root);
    completed(&mut host, 701, 801);
    let input = begin(&mut host, 701, 799);
    votes(&mut host, 799, Verdict::Allow);
    host.finish_review(host.revision(), 799, Some(&input), snapshot())
        .unwrap()
        .unwrap();
    completed(&mut host, 702, 802);
    let (_, supervisor) = host.into_actor_gateway();
    let before = supervisor.host().unwrap().inspect();
    let files = root.files();
    let (short, short_custody) = grant(&supervisor, 1, before.revision as usize - 1, 3);
    let (narrow, narrow_custody) = grant(&supervisor, 1, before.revision as usize, 1);
    let (full, full_custody) = grant(&supervisor, 2, before.revision as usize, 2);
    assert_eq!(short.history(701), Err(ActorError::Capacity));
    assert_eq!(narrow.history(701), Err(ActorError::Capacity));
    assert_eq!(short_custody.remaining_queries(), 0);
    assert_eq!(narrow_custody.remaining_queries(), 0);
    let first = observed(full.history(701));
    assert_eq!(
        first
            .reviews()
            .iter()
            .map(FileRequestReviewLink::round)
            .collect::<Vec<_>>(),
        vec![801, 799]
    );
    assert!(first.reviews()[0].completed_revision() < first.reviews()[1].completed_revision());
    let second = observed(full.clone().history(702));
    assert_eq!(second.attempt(), Some(2));
    assert_eq!(second.reviews().len(), 1);
    assert_eq!(second.reviews()[0].round(), 802);
    assert_eq!(full_custody.remaining_queries(), 0);
    assert_eq!(full.history(701), Err(ActorError::Capacity));
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), files);
}

#[test]
fn weak_custody_foreign_owners_and_replaced_incarnations_cannot_renew_disclosure() {
    let root = Root::new();
    let mut host = owner(&root);
    admit(&mut host, 701, b"ok");
    let (actor, mut supervisor) = host.into_actor_gateway();
    let (port, custody) = grant(&supervisor, 2, MAX_REQUEST_HISTORY_EVENTS, 2);
    assert!(port.belongs_to(&actor));
    assert_eq!(port.history(0), Err(ActorError::MalformedProposal));
    assert_eq!(custody.remaining_queries(), 2);
    {
        let _borrow = supervisor.host_mut().unwrap();
        assert_eq!(port.history(701), Ok(super::unknown()));
    }
    assert_eq!(custody.remaining_queries(), 2);
    observed(port.history(701));
    let foreign_root = Root::new();
    let mut foreign = owner(&foreign_root);
    admit(&mut foreign, 701, b"ok");
    let (foreign_actor, foreign_supervisor) = foreign.into_actor_gateway();
    assert!(!port.belongs_to(&foreign_actor));
    drop(foreign_supervisor);
    let replacement_root = Root::new();
    let mut replacement = owner(&replacement_root);
    admit(&mut replacement, 701, b"ok");
    assert_eq!(replacement.inspect(), supervisor.host().unwrap().inspect());
    *supervisor.host_mut().unwrap() = replacement;
    assert_eq!(port.history(701), Ok(super::unknown()));
    assert_eq!(custody.remaining_queries(), 1);
    let (fresh, fresh_custody) = grant(&supervisor, 2, MAX_REQUEST_HISTORY_EVENTS, 2);
    observed(fresh.history(701));
    fresh_custody.revoke();
    assert_eq!(
        fresh.history(701),
        Ok(Knowledge::Withheld {
            authority_required: "supervisor_request_history",
        })
    );
    drop(supervisor);
    assert_eq!(port.history(701), Ok(super::unknown()));
}

#[test]
fn storage_failure_and_invalid_provisioning_never_fall_back_to_an_old_file() {
    let root = Root::new();
    let mut host = owner(&root);
    admit(&mut host, 701, b"ok");
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, custody) = grant(&supervisor, 2, MAX_REQUEST_HISTORY_EVENTS, 2);
    observed(port.history(701));
    for (queries, events, reviews) in [
        (0, 1, 1),
        (1, 0, 1),
        (1, 1, 0),
        (MAX_REQUEST_HISTORY_QUERIES + 1, 1, 1),
        (1, MAX_REQUEST_HISTORY_EVENTS + 1, 1),
        (1, 1, MAX_REQUEST_HISTORY_REVIEWS + 1),
    ] {
        assert!(
            supervisor
                .request_history_port(
                    queries,
                    events,
                    reviews,
                    FileRequestHistoryDisclosure::RecordedRequestRelationships
                )
                .is_err()
        );
    }
    std::fs::write(root.store().join("delivery.pending"), b"occupied stage").unwrap();
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        assert!(host.observe_time(revision, ElapsedTick(2)).is_err());
        assert!(host.storage_failure().is_some());
    }
    let files = root.files();
    assert_eq!(port.history(701), Ok(super::unknown()));
    assert_eq!(custody.remaining_queries(), 1);
    assert_eq!(root.files(), files);
}

#[test]
fn learned_refinement_and_application_are_discovered_without_numerical_replay() {
    use crate::action::consequence::activation::probe::learned::{KvGroup, KvRow};
    use crate::action::consequence::activation::tensor::kv::experiment::KvSide;
    use crate::action::consequence::delivery::persistent::observed::{
        decoder::learned::sidecar::FileLearnedSidecarFinish,
        guarded::investigation::proposal_replay_event_attempts,
        replay::review_replay_event_attempts,
    };
    use crate::action::consequence::oversight::{
        learned_host::sidecar::LearnedSidecarRequest,
        sidecar::{SidecarCongressBudget, SidecarIdentity},
    };
    let root = Root::new();
    let mut host = owner(&root);
    host.enable_learned_generation(host.revision(), learned_model::config(false, 1))
        .unwrap();
    for _ in 0..2 {
        let native = host.learned_generation_inspection().unwrap().numerical;
        host.advance_learned_generation(host.revision(), native.actor_revision, native.position)
            .unwrap()
            .unwrap();
    }
    let attempt = admit(&mut host, 901, b"ok");
    let actor_revision = host
        .learned_generation_inspection()
        .unwrap()
        .numerical
        .actor_revision;
    let mut sidecar = host
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
                priority: vec![KvGroup {
                    row: KvRow {
                        layer: 1,
                        side: KvSide::Value,
                        position: 1,
                    },
                    head: 0,
                }],
                budget: SidecarCongressBudget {
                    rounds: 2,
                    ..SidecarCongressBudget::default()
                },
            },
        )
        .unwrap();
    let mut revisions = Vec::new();
    for (round, vote) in [(101, Verdict::Abstain), (102, Verdict::Allow)] {
        host.begin_review(
            host.revision(),
            attempt,
            round,
            ROOT,
            ReviewWindow {
                commit_by: ElapsedTick(5),
                reveal_by: ElapsedTick(10),
            },
            snapshot(),
        )
        .unwrap();
        let begin = host.revision();
        votes(&mut host, round, vote);
        let result = host
            .finish_learned_sidecar_review(host.revision(), &mut sidecar, round, snapshot())
            .unwrap();
        match round {
            101 => assert!(matches!(result, FileLearnedSidecarFinish::Refined { .. })),
            102 => assert!(matches!(
                result,
                FileLearnedSidecarFinish::Applied { receipt: Ok(_), .. }
            )),
            _ => unreachable!(),
        }
        revisions.push((begin, host.revision()));
    }
    host.clear_learned_replay_cache();
    let numerical = host.learned_generation_inspection().unwrap();
    let (_, supervisor) = host.into_actor_gateway();
    let before = supervisor.host().unwrap().inspect();
    let files = root.files();
    let proposal_attempts = proposal_replay_event_attempts();
    let review_attempts = review_replay_event_attempts();
    let (port, custody) = grant(&supervisor, 1, before.revision as usize, 2);
    let history = observed(port.history(901));
    assert_eq!(history.reviews().len(), 2);
    assert_eq!(
        history.reviews()[0].completion_kind(),
        FileRequestReviewCompletion::EvidenceRefined
    );
    assert_eq!(
        history.reviews()[1].completion_kind(),
        FileRequestReviewCompletion::ApplicationAttempted
    );
    for (link, (begin, end)) in history.reviews().iter().zip(revisions) {
        assert_eq!(
            (link.begin_revision(), link.completed_revision()),
            (begin, end)
        );
    }
    assert_eq!(history.incomplete_reviews(), 0);
    assert_eq!(custody.remaining_queries(), 0);
    assert_eq!(proposal_replay_event_attempts(), proposal_attempts);
    assert_eq!(review_replay_event_attempts(), review_attempts);
    assert_eq!(
        supervisor
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap(),
        numerical
    );
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), files);
}
