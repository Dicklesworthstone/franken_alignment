//! Actual learned text, original congress and later publication through the
//! existing sealed explanation/wire. Synthetic votes test custody, not quality.
use super::*;
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileReviewExplanation, MAX_REVIEW_EXPLANATION_EVENTS,
};

const REQUEST: u64 = 701;
const ROUND: u64 = REQUEST + 100;

fn completed(
    root: &Directory,
) -> (FileLearnedTextStreamActorPort, Supervisor, oversight::Keys) {
    let (actor, mut supervisor, reviewer) = create(root);
    generate(&mut supervisor);
    observe(&mut supervisor);
    let ticket = actor
        .submit(REQUEST, LearnedTextRelease::Message, ElapsedTick(100))
        .unwrap();
    assert_eq!(actor.poll(&ticket), ActorKnowledge::Pending { request: REQUEST });
    let keys = support::ready(&mut supervisor, &reviewer, REQUEST);
    let generated = supervisor.host().unwrap().learned_generation_inspection().unwrap();
    assert!(generated.numerical.work.admitted_tokens > 0);
    assert!(generated.numerical.work.reserved_decoder_products > 0);
    assert!(generated.numerical.telemetry.compression_work_units > 0);
    (actor, supervisor, keys)
}

fn query(facade: &PublicationTower) -> TowerRequest {
    let mut query = TowerRequest::new(
        "explain-original-learned-review",
        facade.request_address(REQUEST).unwrap(),
        Verb::Explain,
    );
    query.arguments = ReadArguments::ReviewExplanation { round: ROUND };
    query
}

fn explain(facade: &mut PublicationTower) -> PublicationResponse {
    let query = query(facade);
    facade.exchange(&access(true), encode_request(&query).unwrap().as_bytes())
}

fn explained(response: &PublicationResponse) -> &FileReviewExplanation {
    let PublicationResult::ReviewExplanation(linked) = &response.result else {
        panic!("expected original review result: {response:?}");
    };
    let Ok(ActorKnowledge::Known { value, basis }) = linked.native() else {
        panic!("expected completed original learned review: {response:?}");
    };
    assert_eq!(value.basis(), *basis);
    assert_eq!(basis.request, REQUEST);
    assert_eq!(basis.round, ROUND);
    assert_eq!(linked.registration(), &response.envelope.address);
    assert_eq!(linked.round(), ROUND);
    assert_eq!(response.envelope.reason_code, None);
    assert!(matches!(response.envelope.control_seq, Knowledge::Unknown { .. }));
    assert!(matches!(response.envelope.affordances, Knowledge::Unknown { .. }));
    value
}

fn unknown(response: &PublicationResponse) {
    let PublicationResult::ReviewExplanation(linked) = &response.result else {
        panic!("expected charged original review result: {response:?}");
    };
    assert!(matches!(linked.native(), Ok(ActorKnowledge::Unknown { .. })));
    assert_eq!(response.envelope.reason_code, Some(ReasonCode::LowerLayerUnverified));
}

#[test]
fn numerical_allowance_explains_original_learned_congress_and_later_effect_without_owning_either() {
    let root = Directory::new();
    let (actor, mut supervisor, keys) = completed(&root);
    let original = supervisor.host().unwrap().review_replay(ROUND).unwrap();
    let state = supervisor.host().unwrap().inspect();
    let numerical = supervisor.host().unwrap().learned_generation_inspection().unwrap();
    let events = state.revision as usize;
    let total_allowance = 2 * MAX_REVIEW_EXPLANATION_EVENTS;
    let (port, grant) = supervisor
        .review_explanation_port_with_numerical_budget(
            2,
            MAX_REVIEW_EXPLANATION_EVENTS,
            total_allowance,
        )
        .unwrap();
    let mut facade = tower(actor);
    facade.set_review_explanation(Some(port)).unwrap();
    assert_eq!(
        facade.dispatch(&access(false), query(&facade)).envelope.reason_code,
        Some(ReasonCode::DeniedByPolicy)
    );
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(total_allowance));

    // Reconstruct exclusively from the original owner and its immutable recipe.
    // The test's foreign canonical bytes are neither parsed nor repaired.
    let acknowledged = image(&root);
    let canonical = root.store().join("delivery.bin");
    std::fs::write(&canonical, b"unacknowledged foreign journal").unwrap();
    let response = explain(&mut facade);
    let historical = explained(&response);
    assert_eq!(historical.policy(), &original.anchor().policy.policy);
    assert_eq!(historical.evaluation(), &original.archive().policy.evaluation);
    assert_eq!(historical.congress(), &original.anchor().policy.congress);
    assert_eq!(historical.decision(), &original.archive().policy.decision);
    assert_eq!(historical.tally(), &original.archive().policy.tally);
    assert_eq!(historical.application().unwrap().consequence, Consequence::Continue);
    assert_eq!(historical.basis().completed_revision, original.completed_snapshot().revision);
    assert_eq!(historical.basis().journal_revision, state.revision);
    assert_eq!(historical.ending_state(), ActionState::Authorized);
    assert!(!historical.effect().selected_review_dispatched());
    assert!(historical.effect().dispatch().is_none());
    assert_eq!(grant.remaining_numerical_replay_events(), Some(total_allowance - events));
    assert_eq!(supervisor.host().unwrap().inspect(), state);
    assert_eq!(
        supervisor.host().unwrap().learned_generation_inspection().unwrap(),
        numerical
    );
    assert_eq!(image(&root), b"unacknowledged foreign journal");
    let wire = response.to_json().unwrap();
    assert!(!wire.contains("PRIVATE-SOURCE-BYTES"));
    std::fs::write(&canonical, &acknowledged).unwrap();

    // Original independent approval keys still drive the real effect after the
    // historical read. The read supplied no approval and consumed neither key.
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &keys);
        let revision = host.revision();
        let result = host
            .publish_checked(
                revision,
                keys.automatic.attempt(),
                Some(&keys.inputs),
                snapshot(),
                ElapsedTick(2),
            )
            .unwrap();
        assert!(matches!(result.outcome, EndpointOutcome::Executed { .. }));
        let revision = host.revision();
        host.reconcile(revision, keys.automatic.attempt()).unwrap();
    }
    let ending = supervisor.host().unwrap().inspect();
    let numerical = supervisor.host().unwrap().learned_generation_inspection().unwrap();
    let acknowledged = image(&root);
    let later_response = explain(&mut facade);
    let later = explained(&later_response);
    assert_eq!(later.policy(), historical.policy());
    assert_eq!(later.application(), historical.application());
    assert_eq!(later.basis().completed_revision, historical.basis().completed_revision);
    assert!(later.basis().journal_revision > historical.basis().journal_revision);
    assert_eq!(later.ending_state(), ActionState::Confirmed);
    assert!(later.effect().selected_review_dispatched());
    assert!(later.effect().outcome().is_some());
    assert_eq!(
        grant.remaining_numerical_replay_events(),
        Some(total_allowance - events - ending.revision as usize)
    );
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(supervisor.host().unwrap().inspect(), ending);
    assert_eq!(
        supervisor.host().unwrap().learned_generation_inspection().unwrap(),
        numerical
    );
    assert_eq!(image(&root), acknowledged);
}

#[test]
fn learned_review_default_and_one_short_stay_unknown_while_exact_cut_succeeds() {
    let root = Directory::new();
    let (actor, supervisor, _keys) = completed(&root);
    let events = supervisor.host().unwrap().revision() as usize;
    let state = supervisor.host().unwrap().inspect();
    let numerical = supervisor.host().unwrap().learned_generation_inspection().unwrap();
    let acknowledged = image(&root);
    let mut facade = tower(actor);

    let (default, default_grant) = supervisor.review_explanation_port_with_budget(1, events).unwrap();
    facade.set_review_explanation(Some(default)).unwrap();
    unknown(&explain(&mut facade));
    assert_eq!(default_grant.remaining_queries(), 0);
    assert_eq!(default_grant.remaining_numerical_replay_events(), None);

    let (short, short_grant) = supervisor
        .review_explanation_port_with_numerical_budget(1, events, events - 1)
        .unwrap();
    facade.set_review_explanation(Some(short)).unwrap();
    unknown(&explain(&mut facade));
    assert_eq!(short_grant.remaining_queries(), 0);
    assert_eq!(short_grant.remaining_numerical_replay_events(), Some(events - 1));

    let (exact, exact_grant) = supervisor
        .review_explanation_port_with_numerical_budget(1, events, events)
        .unwrap();
    facade.set_review_explanation(Some(exact)).unwrap();
    assert_eq!(explained(&explain(&mut facade)).basis().journal_revision, events as u64);
    assert_eq!(exact_grant.remaining_queries(), 0);
    assert_eq!(exact_grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(supervisor.host().unwrap().inspect(), state);
    assert_eq!(
        supervisor.host().unwrap().learned_generation_inspection().unwrap(),
        numerical
    );
    assert_eq!(image(&root), acknowledged);
}
