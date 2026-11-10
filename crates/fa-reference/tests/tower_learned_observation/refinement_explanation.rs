//! Public native grant over actual learned refinement and later publication.
//! This child deliberately uses no Tower refinement variant, so the native
//! completion API and its old applied selector remain independently testable.
#[path = "../support/learned_refinement_explanation.rs"]
pub(super) mod fixture;

use fa_reference::action::ActionState;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileCompletedReviewExplanation, FileCompletedReviewExplanationResult,
    FileRefinementExplanation, MAX_REVIEW_EXPLANATION_EVENTS,
};
use fa_reference::action::consequence::oversight::actor::{ActorError, Knowledge, UnknownReason};
use fixture::{APPLIED_ROUND, Directory, REFINEMENT_ROUND, REQUEST};

fn refined(result: FileCompletedReviewExplanationResult) -> FileRefinementExplanation {
    let Ok(Knowledge::Known {
        value: FileCompletedReviewExplanation::Refined(value),
        basis,
    }) = result
    else {
        panic!("expected native original refinement: {result:?}");
    };
    assert_eq!(value.basis(), basis);
    value
}

#[test]
fn original_learned_owner_exposes_refinement_and_attributes_later_actual_effect_to_fresh_review() {
    let root = Directory::new();
    let mut state = fixture::prepared(&root);
    let (port, grant) = state
        .supervisor
        .review_explanation_port_with_numerical_budget(
            3,
            MAX_REVIEW_EXPLANATION_EVENTS,
            MAX_REVIEW_EXPLANATION_EVENTS * 3,
        )
        .unwrap();
    let before = state.supervisor.host().unwrap().inspect();
    let generation = state
        .supervisor
        .host()
        .unwrap()
        .learned_generation_inspection()
        .unwrap();
    let bytes = fixture::image(&root);
    let value = refined(port.explain_review_completion(REQUEST, REFINEMENT_ROUND));
    assert_eq!(value.basis().scope, fixture::profile().delivery.scope);
    assert_eq!(value.basis().request, REQUEST);
    assert_eq!(value.basis().attempt, state.sidecar.attempt());
    assert_eq!(value.basis().round, REFINEMENT_ROUND);
    assert_eq!(value.basis().completed_revision, state.refinement_revision);
    assert_eq!(value.basis().journal_revision, before.revision);
    assert_eq!(value.group(), fixture::group());
    assert_eq!(value.reviewed_input_revision(), 1);
    assert_eq!(value.refined_input_revision(), 2);
    assert_eq!(value.abstained(), &["alpha".to_owned(), "beta".to_owned()]);
    assert!(value.missing().is_empty());
    assert_eq!(value.reveals().len(), 2);
    assert_eq!(value.completed_state(), ActionState::Reviewing);
    assert_eq!(value.ending_state(), ActionState::Reviewing);
    assert!(value.effect().dispatch().is_none());
    assert!(value.effect().outcome().is_none());
    assert!(!value.effect().selected_review_dispatched());
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(state.supervisor.host().unwrap().inspect(), before);
    assert_eq!(
        state
            .supervisor
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap(),
        generation
    );
    assert_eq!(fixture::image(&root), bytes);

    let applied_revision = fixture::publish(&mut state);
    let before = state.supervisor.host().unwrap().inspect();
    let generation = state
        .supervisor
        .host()
        .unwrap()
        .learned_generation_inspection()
        .unwrap();
    let bytes = fixture::image(&root);
    let after = refined(port.explain_review_completion(REQUEST, REFINEMENT_ROUND));
    assert_eq!(
        after.basis().completed_revision,
        value.basis().completed_revision
    );
    assert_eq!(after.evaluation(), value.evaluation());
    assert_eq!(after.tally(), value.tally());
    assert_eq!(after.group(), value.group());
    assert_eq!(
        after.reviewed_input_revision(),
        value.reviewed_input_revision()
    );
    assert_eq!(
        after.refined_input_revision(),
        value.refined_input_revision()
    );
    assert_eq!(after.ending_state(), ActionState::Confirmed);
    assert!(!after.effect().selected_review_dispatched());
    let dispatch = after.effect().dispatch().unwrap();
    assert_eq!(dispatch.authorization().round(), APPLIED_ROUND);
    assert_eq!(dispatch.authorization().review_revision(), applied_revision);
    assert_eq!(dispatch.authorization().input_revision(), 2);
    assert!(matches!(
        after.effect().outcome().unwrap().outcome(),
        EndpointOutcome::Executed { .. }
    ));
    assert!(after.effect().reconciliation().unwrap().result().is_ok());
    let Ok(Knowledge::Known {
        value: FileCompletedReviewExplanation::Applied(applied),
        ..
    }) = port.explain_review_completion(REQUEST, APPLIED_ROUND)
    else {
        panic!("fresh successful review must remain an actual application");
    };
    assert_eq!(applied.application().unwrap().input_revision, 2);
    assert!(applied.effect().selected_review_dispatched());
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(state.supervisor.host().unwrap().inspect(), before);
    assert_eq!(
        state
            .supervisor
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap(),
        generation
    );
    assert_eq!(fixture::image(&root), bytes);
    // The earlier retained observation stays at its own immutable historical cut.
    assert_eq!(value.ending_state(), ActionState::Reviewing);
    assert!(value.effect().dispatch().is_none());
}

#[test]
fn public_refinement_disclosure_requires_numerical_consent_and_shared_custody() {
    let root = Directory::new();
    let state = fixture::prepared(&root);
    let events = state.supervisor.host().unwrap().revision() as usize;
    let (ordinary, ordinary_grant) = state.supervisor.review_explanation_port(1).unwrap();
    let (short, short_grant) = state
        .supervisor
        .review_explanation_port_with_numerical_budget(1, events, events - 1)
        .unwrap();
    let (port, grant) = state
        .supervisor
        .review_explanation_port_with_numerical_budget(2, events, events * 2)
        .unwrap();
    let before = state.supervisor.host().unwrap().inspect();
    let generation = state
        .supervisor
        .host()
        .unwrap()
        .learned_generation_inspection()
        .unwrap();
    let bytes = fixture::image(&root);
    for result in [
        ordinary.explain_review_completion(REQUEST, REFINEMENT_ROUND),
        short.explain_review_completion(REQUEST, REFINEMENT_ROUND),
    ] {
        assert_eq!(
            result,
            Ok(Knowledge::Unknown {
                reason: UnknownReason::ControllerUnavailable
            })
        );
    }
    assert_eq!(ordinary_grant.remaining_queries(), 0);
    assert_eq!(short_grant.remaining_queries(), 0);
    assert_eq!(
        short_grant.remaining_numerical_replay_events(),
        Some(events - 1)
    );
    // The legacy selector never rebrands a refinement as a refused application.
    assert!(matches!(
        port.explain_review(REQUEST, REFINEMENT_ROUND),
        Ok(Knowledge::Unknown { .. })
    ));
    assert_eq!(grant.remaining_numerical_replay_events(), Some(events));
    let clone = port.clone();
    refined(clone.explain_review_completion(REQUEST, REFINEMENT_ROUND));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(
        port.explain_review_completion(REQUEST, REFINEMENT_ROUND),
        Err(ActorError::Capacity)
    );
    assert_eq!(state.supervisor.host().unwrap().inspect(), before);
    assert_eq!(
        state
            .supervisor
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap(),
        generation
    );
    assert_eq!(fixture::image(&root), bytes);
    grant.revoke();
    assert!(matches!(
        clone.explain_review_completion(REQUEST, REFINEMENT_ROUND),
        Ok(Knowledge::Withheld { .. })
    ));
}
