//! Original learned admission and exact-policy denial through the existing wire.
//! Numerical generation is real; congress ballots are controls, not quality evidence.
use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::{
    FileRequestDisposition,
    actor::{FileAdmissionExplanation, MAX_EXPLANATION_EVENTS},
};
use fa_reference::action::consequence::experiment::InterventionScope;
use fa_reference::action::consequence::gate::containment::session::policy::{
    Policy, Predicate, Truth,
};

const REQUEST: u64 = 701;

fn query(facade: &PublicationTower) -> TowerRequest {
    TowerRequest::new(
        "explain-original-learned-admission",
        facade.request_address(REQUEST).unwrap(),
        Verb::Explain,
    )
}

fn explain(facade: &mut PublicationTower) -> PublicationResponse {
    let request = query(facade);
    facade.exchange(&access(true), encode_request(&request).unwrap().as_bytes())
}

fn explained(response: &PublicationResponse) -> &FileAdmissionExplanation {
    let PublicationResult::Explanation(Ok(ActorKnowledge::Known { value, basis })) =
        &response.result
    else {
        panic!("expected original learned admission: {response:?}");
    };
    assert_eq!(value.basis(), *basis);
    assert_eq!(basis.request, REQUEST);
    assert_ne!(basis.attempt, REQUEST);
    assert_eq!(response.envelope.reason_code, None);
    assert!(matches!(response.envelope.control_seq, Knowledge::Unknown { .. }));
    assert!(matches!(response.envelope.affordances, Knowledge::Unknown { .. }));
    value
}

fn unknown(response: &PublicationResponse) {
    assert!(matches!(
        response.result,
        PublicationResult::Explanation(Ok(ActorKnowledge::Unknown { .. }))
    ));
    assert_eq!(
        response.envelope.reason_code,
        Some(ReasonCode::LowerLayerUnverified)
    );
}

#[test]
fn learned_admission_and_actual_policy_denial_require_the_complete_explicit_cut() {
    for denied in [false, true] {
        let root = Directory::new();
        let mut selected = profile();
        if denied {
            selected.delivery.policy = Policy::new(
                1,
                vec![
                    Predicate::ExactValue {
                        key: 7,
                        value: b"ok".to_vec(),
                    },
                    Predicate::PayloadAtMost(1),
                    Predicate::All(vec![0, 1]),
                ],
            )
            .unwrap();
        }
        let (mut host, _reviewer) =
            FileOversight::create_with_learned_text_stream(root.store(), selected, recipe())
                .unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let (actor, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        generate(&mut supervisor);
        observe(&mut supervisor);
        let ticket = actor
            .submit(REQUEST, LearnedTextRelease::Message, ElapsedTick(100))
            .unwrap();
        let stage = if denied { ActionState::Denied } else { ActionState::Reviewing };
        let original = {
            let host = supervisor.host().unwrap();
            let attempt = support::attempt(&host, REQUEST);
            assert_eq!(
                host.request_status(REQUEST).unwrap().disposition,
                FileRequestDisposition::Admitted { attempt, stage }
            );
            host.investigate_proposal(
                attempt,
                InterventionScope::new(REQUEST, false, false, false, &[]).unwrap(),
            )
            .unwrap()
        };
        let outcome = actor.poll(&ticket);
        if denied {
            assert!(matches!(
                outcome,
                ActorKnowledge::Known { value: ActorOutcome::Denied, .. }
            ));
        } else {
            assert_eq!(outcome, ActorKnowledge::Pending { request: REQUEST });
        }
        let state = supervisor.host().unwrap().inspect();
        let numerical = supervisor.host().unwrap().learned_generation_inspection().unwrap();
        assert!(numerical.numerical.work.admitted_tokens > 0);
        assert!(numerical.numerical.work.reserved_decoder_products > 0);
        assert!(numerical.numerical.telemetry.compression_work_units > 0);
        let events = state.revision as usize;
        assert!(events > 1);
        let acknowledged = image(&root);
        let mut facade = tower(actor.clone());

        let (default, default_grant) =
            supervisor.explanation_port_with_budget(1, events).unwrap();
        facade.set_explanation(Some(default)).unwrap();
        unknown(&explain(&mut facade));
        assert_eq!(default_grant.remaining_queries(), 0);
        assert_eq!(default_grant.remaining_numerical_replay_events(), None);

        let (short, short_grant) = supervisor
            .explanation_port_with_numerical_budget(1, events, events - 1)
            .unwrap();
        facade.set_explanation(Some(short)).unwrap();
        unknown(&explain(&mut facade));
        assert_eq!(short_grant.remaining_queries(), 0);
        assert_eq!(short_grant.remaining_numerical_replay_events(), Some(events - 1));

        let (exact, exact_grant) = supervisor
            .explanation_port_with_numerical_budget(1, events, events)
            .unwrap();
        facade.set_explanation(Some(exact)).unwrap();
        let response = explain(&mut facade);
        let historical = explained(&response);
        assert_eq!(historical.policy(), original.origin().proposal().policy.as_ref());
        assert_eq!(historical.evaluation(), &original.origin().proposal().evaluation);
        assert_eq!(
            historical.evaluation().result(),
            if denied { Truth::Violated } else { Truth::Satisfied }
        );
        assert_eq!(historical.initial_state(), stage);
        assert_eq!(historical.ending_state(), stage);
        assert_eq!(
            historical.basis().admission_revision,
            original.origin().admission_snapshot().revision
        );
        assert_eq!(historical.basis().journal_revision, state.revision);
        assert_eq!(exact_grant.remaining_queries(), 0);
        assert_eq!(exact_grant.remaining_numerical_replay_events(), Some(0));
        assert_eq!(supervisor.host().unwrap().inspect(), state);
        assert_eq!(
            supervisor.host().unwrap().learned_generation_inspection().unwrap(),
            numerical
        );
        assert_eq!(actor.poll(&ticket), outcome);
        assert_eq!(image(&root), acknowledged);
        let wire = response.to_json().unwrap();
        assert!(wire.contains("fa.publication-explanation/1"));
        assert!(wire.contains(r#""current_eligibility":"not_observed""#));
        assert!(wire.contains(r#""effect_authority":false"#));
    }
}

#[test]
fn learned_admission_reads_preserve_original_intake_then_independent_two_key_publication() {
    let root = Directory::new();
    let (actor, mut supervisor, reviewer) = create(&root);
    generate(&mut supervisor);
    observe(&mut supervisor);
    let ticket = actor
        .submit(REQUEST, LearnedTextRelease::Message, ElapsedTick(100))
        .unwrap();
    let state = supervisor.host().unwrap().inspect();
    let numerical = supervisor.host().unwrap().learned_generation_inspection().unwrap();
    let events = state.revision as usize;
    let total = 2 * MAX_EXPLANATION_EVENTS;
    let (port, grant) = supervisor
        .explanation_port_with_numerical_budget(2, MAX_EXPLANATION_EVENTS, total)
        .unwrap();
    let mut facade = tower(actor.clone());
    facade.set_explanation(Some(port)).unwrap();
    assert_eq!(
        facade.dispatch(&access(false), query(&facade)).envelope.reason_code,
        Some(ReasonCode::DeniedByPolicy)
    );
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(total));

    // Only acknowledged in-memory events and their original immutable model are
    // available to this capability. Foreign canonical bytes remain untouched.
    let canonical = root.store().join("delivery.bin");
    let acknowledged = image(&root);
    std::fs::write(&canonical, b"unacknowledged foreign journal").unwrap();
    let response = explain(&mut facade);
    let original = explained(&response);
    assert_eq!(original.policy(), &profile().delivery.policy);
    assert_eq!(original.evaluation().result(), Truth::Satisfied);
    assert_eq!(original.initial_state(), ActionState::Reviewing);
    assert_eq!(original.ending_state(), ActionState::Reviewing);
    assert_eq!(original.basis().journal_revision, state.revision);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(total - events));
    assert_eq!(supervisor.host().unwrap().inspect(), state);
    assert_eq!(
        supervisor.host().unwrap().learned_generation_inspection().unwrap(),
        numerical
    );
    assert_eq!(actor.poll(&ticket), ActorKnowledge::Pending { request: REQUEST });
    assert_eq!(image(&root), b"unacknowledged foreign journal");
    std::fs::write(&canonical, acknowledged).unwrap();

    // The historical read supplied no approval. The original congress and human
    // reviewer must still authorize and dispatch the source-derived release.
    let keys = support::ready(&mut supervisor, &reviewer, REQUEST);
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
    assert_eq!(later.policy(), original.policy());
    assert_eq!(later.evaluation(), original.evaluation());
    assert_eq!(later.basis().admission_revision, original.basis().admission_revision);
    assert!(later.basis().journal_revision > original.basis().journal_revision);
    assert_eq!(later.basis().journal_revision, ending.revision);
    assert_eq!(later.initial_state(), ActionState::Reviewing);
    assert_eq!(later.ending_state(), ActionState::Confirmed);
    assert_eq!(
        grant.remaining_numerical_replay_events(),
        Some(total - events - ending.revision as usize)
    );
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(supervisor.host().unwrap().inspect(), ending);
    assert_eq!(
        supervisor.host().unwrap().learned_generation_inspection().unwrap(),
        numerical
    );
    assert_eq!(image(&root), acknowledged);
    assert!(matches!(
        actor.poll(&ticket),
        ActorKnowledge::Known { value: ActorOutcome::Executed, .. }
    ));
    assert!(!later_response.to_json().unwrap().contains("PRIVATE-SOURCE-BYTES"));
}

#[test]
fn learned_admission_output_loss_keeps_the_original_numerical_reservation_spent() {
    let root = Directory::new();
    let mut selected = profile();
    selected.delivery.policy =
        Policy::new(1, vec![Predicate::PayloadIs(vec![0xaa; 20_000])]).unwrap();
    let original_policy = selected.delivery.policy.clone();
    let (mut host, _reviewer) =
        FileOversight::create_with_learned_text_stream(root.store(), selected, recipe()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let (actor, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    generate(&mut supervisor);
    observe(&mut supervisor);
    let ticket = actor
        .submit(REQUEST, LearnedTextRelease::Message, ElapsedTick(100))
        .unwrap();
    assert!(matches!(
        actor.poll(&ticket),
        ActorKnowledge::Known { value: ActorOutcome::Denied, .. }
    ));
    let state = supervisor.host().unwrap().inspect();
    let numerical = supervisor.host().unwrap().learned_generation_inspection().unwrap();
    let acknowledged = image(&root);
    let events = state.revision as usize;
    let (port, grant) = supervisor
        .explanation_port_with_numerical_budget(2, events, events * 2)
        .unwrap();
    let mut bounded = tower(actor.clone());
    bounded.set_explanation(Some(port.clone())).unwrap();
    let response = explain(&mut bounded);
    assert!(matches!(response.result, PublicationResult::Refused(_)));
    assert_eq!(response.envelope.reason_code, Some(ReasonCode::BudgetExhausted));
    assert!(!response.to_json().unwrap().contains("aaaa"));
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(events));

    // A larger output projection can disclose the same original policy, but its
    // clone must pay a second complete reconstruction from the shared custodian.
    let mut admitted = PublicationTower::new_learned_observer(
        mount(),
        actor,
        PublicationLimits { max_response_bytes: 128 * 1024 },
    )
    .unwrap();
    admitted.set_explanation(Some(port)).unwrap();
    let complete = explain(&mut admitted);
    let historical = explained(&complete);
    assert_eq!(historical.policy(), &original_policy);
    assert_eq!(historical.initial_state(), ActionState::Denied);
    assert_eq!(historical.evaluation().result(), Truth::Violated);
    assert!(complete.to_json().unwrap().contains("aaaa"));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(supervisor.host().unwrap().inspect(), state);
    assert_eq!(
        supervisor.host().unwrap().learned_generation_inspection().unwrap(),
        numerical
    );
    assert_eq!(image(&root), acknowledged);
}
