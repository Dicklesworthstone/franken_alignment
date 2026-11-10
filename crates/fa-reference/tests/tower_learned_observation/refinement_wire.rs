//! The original completed refinement through its sealed Tower projection.
//! Actual learned replay and publication do not qualify the controlled ballots.
use super::refinement_explanation::fixture;
use super::{access, tower};
use fa_reference::action::ActionState;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileRefinementExplanation, FileReviewExplanation, MAX_REVIEW_EXPLANATION_EVENTS,
};
use fa_reference::action::consequence::oversight::actor::{
    ActorError, Knowledge as ActorKnowledge, UnknownReason as ActorUnknown,
};
use fa_reference::address_resolver::{Address, Noun};
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::observation_algebra::Knowledge;
use fa_reference::strict_json::{self, Json};
use fa_reference::tower::publication::{PublicationResponse, PublicationResult, PublicationTower};
use fa_reference::tower::{
    ENVELOPE_FIELDS, ReadArguments, ReasonCode, TowerEncodingError, TowerRequest, Verb,
    encode_request,
};
use fixture::{APPLIED_ROUND, Directory, REFINEMENT_ROUND, REQUEST};
use std::collections::BTreeSet;

fn query(facade: &PublicationTower, round: u64) -> TowerRequest {
    let mut request = TowerRequest::new(
        "explain-original-refinement",
        facade.request_address(REQUEST).unwrap(),
        Verb::Explain,
    );
    request.arguments = ReadArguments::ReviewExplanation { round };
    request
}

fn explain(facade: &mut PublicationTower, round: u64) -> PublicationResponse {
    let request = query(facade, round);
    facade.exchange(&access(true), encode_request(&request).unwrap().as_bytes())
}

fn no_global_claims(response: &PublicationResponse) {
    fn unknown<T>(value: &Knowledge<T>) {
        assert!(matches!(
            value,
            Knowledge::Unknown { .. } | Knowledge::Withheld { .. }
        ));
    }
    unknown(&response.envelope.generation);
    unknown(&response.envelope.control_seq);
    unknown(&response.envelope.source_frontiers);
    unknown(&response.envelope.budgets_remaining);
    unknown(&response.envelope.realized_cost);
    unknown(&response.envelope.affordances);
}

fn refined(response: &PublicationResponse) -> &FileRefinementExplanation {
    let PublicationResult::RefinementExplanation(linked) = &response.result else {
        panic!("expected sealed original refinement: {response:?}");
    };
    let Ok(ActorKnowledge::Known { value, basis }) = linked.native() else {
        panic!("expected Known original refinement: {response:?}");
    };
    assert_eq!(linked.registration(), &response.envelope.address);
    assert_eq!(linked.round(), REFINEMENT_ROUND);
    assert_eq!(value.basis(), *basis);
    assert_eq!(basis.round, REFINEMENT_ROUND);
    assert_eq!(basis.request, REQUEST);
    assert_eq!(response.envelope.reason_code, None);
    no_global_claims(response);
    value
}

fn applied(response: &PublicationResponse) -> &FileReviewExplanation {
    let PublicationResult::ReviewExplanation(linked) = &response.result else {
        panic!("expected existing applied-review result: {response:?}");
    };
    let Ok(ActorKnowledge::Known { value, basis }) = linked.native() else {
        panic!("expected Known fresh application: {response:?}");
    };
    assert_eq!(linked.registration(), &response.envelope.address);
    assert_eq!(linked.round(), APPLIED_ROUND);
    assert_eq!(value.basis(), *basis);
    assert_eq!(response.envelope.reason_code, None);
    no_global_claims(response);
    value
}

fn unavailable(response: &PublicationResponse) {
    let PublicationResult::ReviewExplanation(linked) = &response.result else {
        panic!("unknown completion must retain the existing result: {response:?}");
    };
    assert_eq!(linked.registration(), &response.envelope.address);
    assert_eq!(
        linked.native(),
        &Ok(ActorKnowledge::Unknown {
            reason: ActorUnknown::ControllerUnavailable,
        })
    );
    assert_eq!(
        response.envelope.reason_code,
        Some(ReasonCode::LowerLayerUnverified)
    );
    let wire = document(response);
    assert_eq!(
        field(&wire, &["result", "kind"]).as_str(),
        Some("review-explanation")
    );
    assert_eq!(
        field(&wire, &["result", "value", "schema"]).as_str(),
        Some("fa.publication-review-explanation/1")
    );
    no_global_claims(response);
}

fn document(response: &PublicationResponse) -> Json {
    strict_json::parse(
        response.to_json().unwrap().as_bytes(),
        KNOWLEDGE_JSON_LIMITS,
    )
    .unwrap()
}

fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(value, |value, name| {
        value
            .get(name)
            .unwrap_or_else(|| panic!("missing field {name}"))
    })
}

fn projection(response: &PublicationResponse) -> Json {
    let native = refined(response);
    let encoded = response.to_json().unwrap();
    let secret = b"original salt";
    assert!(!encoded.contains(std::str::from_utf8(secret).unwrap()));
    let hex: String = secret.iter().map(|byte| format!("{byte:02x}")).collect();
    assert!(!encoded.contains(&hex));
    let wire = document(response);
    let envelope = field(&wire, &["envelope"]).as_object().unwrap();
    assert_eq!(
        envelope.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        ENVELOPE_FIELDS.into_iter().collect()
    );
    assert_eq!(
        field(&wire, &["result", "kind"]).as_str(),
        Some("refinement-explanation")
    );
    let value = field(&wire, &["result", "value"]);
    assert_eq!(
        field(value, &["schema"]).as_str(),
        Some("fa.publication-refinement-explanation/1")
    );
    assert_eq!(
        field(value, &["scope"]).as_str(),
        Some("historical_completed_refinement")
    );
    assert_eq!(field(value, &["round"]).as_str(), Some("101"));
    assert_eq!(field(value, &["historical"]).as_bool(), Some(true));
    assert_eq!(
        field(value, &["current_eligibility"]).as_str(),
        Some("not_observed")
    );
    assert_eq!(
        field(value, &["current_hold_reason", "state"]).as_str(),
        Some("unknown")
    );
    for flag in [
        "helpers_rerun",
        "live_evidence_read",
        "effect_authority",
        "payload_disclosed",
        "witness_values_disclosed",
        "helper_inputs_disclosed",
        "commitment_salts_disclosed",
    ] {
        assert_eq!(field(value, &[flag]).as_bool(), Some(false), "{flag}");
    }
    let body = field(value, &["knowledge", "value"]);
    assert_eq!(
        field(body, &["application", "state"]).as_str(),
        Some("not_attempted")
    );
    assert_eq!(field(body, &["input_revision"]).as_str(), Some("1"));
    assert!(field(body, &["evaluation"]).get("witnesses").is_none());
    assert!(field(body, &["action"]).get("payload").is_none());
    assert!(body.get("inputs").is_none());
    let refinement = field(body, &["refinement"]);
    assert_eq!(
        field(refinement, &["kind"]).as_str(),
        Some("kv_exact_residual")
    );
    assert_eq!(field(refinement, &["group", "layer"]).as_str(), Some("1"));
    assert_eq!(
        field(refinement, &["group", "side"]).as_str(),
        Some("value")
    );
    assert_eq!(
        field(refinement, &["group", "position"]).as_str(),
        Some("3")
    );
    assert_eq!(field(refinement, &["group", "head"]).as_str(), Some("0"));
    assert_eq!(
        field(refinement, &["reviewed_input_revision"]).as_str(),
        Some("1")
    );
    assert_eq!(
        field(refinement, &["refined_input_revision"]).as_str(),
        Some("2")
    );
    assert_eq!(
        field(refinement, &["requires_fresh_review"]).as_bool(),
        Some(true)
    );
    let congress = field(body, &["congress"]);
    assert_eq!(field(congress, &["commit_count"]).as_str(), Some("2"));
    assert_eq!(field(congress, &["reveal_count"]).as_str(), Some("2"));
    assert_eq!(field(congress, &["permit_weight"]).as_str(), Some("0"));
    assert_eq!(native.abstained(), &["alpha".to_owned(), "beta".to_owned()]);
    assert!(native.missing().is_empty());
    for member in field(congress, &["members"]).as_array().unwrap() {
        assert_eq!(field(member, &["outcome"]).as_str(), Some("abstained"));
        assert!(member.get("salt").is_none());
        assert!(member.get("inputs").is_none());
    }
    wire
}

#[test]
fn original_refinement_wire_retains_judgment_and_attributes_publication_to_the_fresh_review() {
    let root = Directory::new();
    let mut state = fixture::prepared(&root);
    let total = MAX_REVIEW_EXPLANATION_EVENTS * 3;
    let (port, grant) = state
        .supervisor
        .review_explanation_port_with_numerical_budget(3, MAX_REVIEW_EXPLANATION_EVENTS, total)
        .unwrap();
    let mut facade = tower(state.actor.clone());
    facade.set_review_explanation(Some(port)).unwrap();
    let before = state.supervisor.host().unwrap().inspect();
    let numerical = state
        .supervisor
        .host()
        .unwrap()
        .learned_generation_inspection()
        .unwrap();
    let bytes = fixture::image(&root);
    let first = explain(&mut facade, REFINEMENT_ROUND);
    let original = refined(&first);
    assert_eq!(original.group(), fixture::group());
    assert_eq!(
        original.basis().completed_revision,
        state.refinement_revision
    );
    assert_eq!(original.basis().journal_revision, before.revision);
    assert_eq!(original.completed_state(), ActionState::Reviewing);
    assert_eq!(original.ending_state(), ActionState::Reviewing);
    assert!(original.effect().dispatch().is_none());
    let wire = projection(&first);
    let effect = field(&wire, &["result", "value", "knowledge", "value", "effect"]);
    for operation in ["dispatch", "outcome", "reconciliation"] {
        assert_eq!(
            field(effect, &[operation, "state"]).as_str(),
            Some("unknown")
        );
    }
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(
        grant.remaining_numerical_replay_events(),
        Some(total - before.revision as usize)
    );
    assert_eq!(state.supervisor.host().unwrap().inspect(), before);
    assert_eq!(
        state
            .supervisor
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap(),
        numerical
    );
    assert_eq!(fixture::image(&root), bytes);

    // A new congress and the original human reviewer remain necessary after
    // this read; the shared fixture performs the actual guarded publication.
    let applied_revision = fixture::publish(&mut state);
    let ending = state.supervisor.host().unwrap().inspect();
    let numerical = state
        .supervisor
        .host()
        .unwrap()
        .learned_generation_inspection()
        .unwrap();
    let bytes = fixture::image(&root);
    let later_response = explain(&mut facade, REFINEMENT_ROUND);
    let later = refined(&later_response);
    assert_eq!(later.evaluation(), original.evaluation());
    assert_eq!(later.tally(), original.tally());
    assert_eq!(
        later.basis().completed_revision,
        original.basis().completed_revision
    );
    assert_eq!(later.basis().journal_revision, ending.revision);
    assert_eq!(later.ending_state(), ActionState::Confirmed);
    assert!(!later.effect().selected_review_dispatched());
    let wire = projection(&later_response);
    let effect = field(&wire, &["result", "value", "knowledge", "value", "effect"]);
    let dispatch = field(effect, &["dispatch", "value"]);
    assert_eq!(field(dispatch, &["selected_review"]).as_bool(), Some(false));
    assert_eq!(
        field(dispatch, &["automatic", "round"]).as_str(),
        Some("102")
    );
    assert_eq!(
        field(dispatch, &["automatic", "review_revision"]).as_str(),
        Some(applied_revision.to_string().as_str())
    );
    assert_eq!(
        field(dispatch, &["automatic", "input_revision"]).as_str(),
        Some("2")
    );
    assert_eq!(
        field(effect, &["outcome", "value", "outcome", "kind"]).as_str(),
        Some("executed")
    );

    // The same selector preserves the old applied-review schema and application
    // metadata for the independently authorized round at the identical cut.
    let applied_response = explain(&mut facade, APPLIED_ROUND);
    let current = applied(&applied_response);
    assert_eq!(current.application().unwrap().input_revision, 2);
    assert!(current.effect().selected_review_dispatched());
    let wire = document(&applied_response);
    assert_eq!(
        field(&wire, &["result", "kind"]).as_str(),
        Some("review-explanation")
    );
    assert_eq!(
        field(&wire, &["result", "value", "schema"]).as_str(),
        Some("fa.publication-review-explanation/1")
    );
    let body = field(&wire, &["result", "value", "knowledge", "value"]);
    assert_eq!(
        field(body, &["application", "state"]).as_str(),
        Some("applied")
    );
    assert!(body.get("refinement").is_none());
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(
        grant.remaining_numerical_replay_events(),
        Some(total - before.revision as usize - 2 * ending.revision as usize)
    );
    assert_eq!(state.supervisor.host().unwrap().inspect(), ending);
    assert_eq!(
        state
            .supervisor
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap(),
        numerical
    );
    assert_eq!(fixture::image(&root), bytes);
    assert_eq!(original.ending_state(), ActionState::Reviewing);
    assert!(original.effect().dispatch().is_none());
}

#[test]
fn refinement_wire_requires_operator_access_and_complete_shared_numerical_allowance() {
    let root = Directory::new();
    let state = fixture::prepared(&root);
    let before = state.supervisor.host().unwrap().inspect();
    let numerical = state
        .supervisor
        .host()
        .unwrap()
        .learned_generation_inspection()
        .unwrap();
    let bytes = fixture::image(&root);
    let events = before.revision as usize;
    let mut facade = tower(state.actor.clone());
    let (default, default_grant) = state.supervisor.review_explanation_port(1).unwrap();
    facade.set_review_explanation(Some(default)).unwrap();
    let request = query(&facade, REFINEMENT_ROUND);
    let denied = facade.dispatch(&access(false), request.clone());
    assert!(matches!(denied.result, PublicationResult::Refused(_)));
    assert_eq!(
        denied.envelope.reason_code,
        Some(ReasonCode::DeniedByPolicy)
    );
    let mut stale = request;
    stale.address.generation = Some(40);
    assert_eq!(
        facade.dispatch(&access(true), stale).envelope.reason_code,
        Some(ReasonCode::StaleGeneration)
    );
    assert_eq!(default_grant.remaining_queries(), 1);
    unavailable(&explain(&mut facade, REFINEMENT_ROUND));
    assert_eq!(default_grant.remaining_queries(), 0);
    assert_eq!(default_grant.remaining_numerical_replay_events(), None);

    let (short, short_grant) = state
        .supervisor
        .review_explanation_port_with_numerical_budget(1, events, events - 1)
        .unwrap();
    facade.set_review_explanation(Some(short)).unwrap();
    unavailable(&explain(&mut facade, REFINEMENT_ROUND));
    assert_eq!(short_grant.remaining_queries(), 0);
    assert_eq!(
        short_grant.remaining_numerical_replay_events(),
        Some(events - 1)
    );

    let (port, grant) = state
        .supervisor
        .review_explanation_port_with_numerical_budget(2, events, events * 2)
        .unwrap();
    let mut clone = tower(state.actor.clone());
    clone.set_review_explanation(Some(port.clone())).unwrap();
    facade.set_review_explanation(Some(port)).unwrap();
    refined(&explain(&mut facade, REFINEMENT_ROUND));
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(events));
    refined(&explain(&mut clone, REFINEMENT_ROUND));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    let exhausted = explain(&mut facade, REFINEMENT_ROUND);
    let PublicationResult::ReviewExplanation(linked) = &exhausted.result else {
        panic!("capacity must retain the existing review result");
    };
    assert_eq!(linked.native(), &Err(ActorError::Capacity));
    assert_eq!(
        exhausted.envelope.reason_code,
        Some(ReasonCode::BudgetExhausted)
    );
    assert_eq!(state.supervisor.host().unwrap().inspect(), before);
    assert_eq!(
        state
            .supervisor
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap(),
        numerical
    );
    assert_eq!(fixture::image(&root), bytes);

    drop(state);
    unavailable(&explain(&mut facade, REFINEMENT_ROUND));
    unavailable(&explain(&mut clone, REFINEMENT_ROUND));
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    grant.revoke();
    let withheld = explain(&mut clone, REFINEMENT_ROUND);
    let PublicationResult::ReviewExplanation(linked) = &withheld.result else {
        panic!("withdrawal must retain the existing review result");
    };
    assert!(matches!(
        linked.native(),
        Ok(ActorKnowledge::Withheld { .. })
    ));
    assert_eq!(
        withheld.envelope.reason_code,
        Some(ReasonCode::DeniedByPolicy)
    );
}

#[test]
fn sealed_refinement_and_unknown_results_cannot_be_rebound_to_another_registration() {
    let root = Directory::new();
    let state = fixture::prepared(&root);
    let events = state.supervisor.host().unwrap().revision() as usize;
    let (port, grant) = state
        .supervisor
        .review_explanation_port_with_numerical_budget(2, events, events * 2)
        .unwrap();
    let mut facade = tower(state.actor.clone());
    facade.set_review_explanation(Some(port)).unwrap();
    let original = explain(&mut facade, REFINEMENT_ROUND);
    refined(&original);
    let unknown = explain(&mut facade, APPLIED_ROUND);
    unavailable(&unknown);
    let exhausted = explain(&mut facade, REFINEMENT_ROUND);
    grant.revoke();
    let withheld = explain(&mut facade, REFINEMENT_ROUND);
    for response in [original, unknown, exhausted, withheld] {
        response.to_json().unwrap();
        for address in [
            facade.request_address(REQUEST + 1).unwrap(),
            Address::new(
                "1",
                Noun::Attempt,
                format!("request-3-2-9-5-{REQUEST}"),
                Some(41),
            )
            .unwrap(),
            Address::new(
                "2",
                Noun::Attempt,
                format!("request-3-2-4-5-{REQUEST}"),
                Some(41),
            )
            .unwrap(),
            Address::new(
                "1",
                Noun::Attempt,
                format!("request-3-2-4-5-{REQUEST}"),
                Some(42),
            )
            .unwrap(),
            facade.mount_address().clone(),
        ] {
            let mut changed = response.clone();
            changed.envelope.address = address;
            assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        }
    }
}
