//! Explicit original-transition reservations for real numerical admissions.
//! Event reservations are separate from original immutable numerical limits.
use super::*;
use crate::action::consequence::delivery::persistent::observed::guarded::investigation::proposal_replay_event_attempts;
use crate::action::consequence::activation::monitor::decoder::MonitoredStep;
use crate::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderIdentity, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS,
};
use crate::action::consequence::delivery::persistent::observed::decoder::FileDecoderConfig;
use crate::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;

mod data {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/decoder_inputs.rs"
    ));
}

fn numerical_owner(root: &Root) -> FileOversight {
    let decoder = DecoderProfile::new(
        DecoderIdentity {
            tenant: 1,
            model: 9,
            model_generation: 1,
            tokenizer_generation: 1,
            profile_generation: 1,
        },
        DecoderShape {
            vocabulary: 2,
            hidden: 2,
            intermediate: 2,
            layers: 1,
            query_heads: 1,
            cache_heads: 1,
            context: 4,
        },
        1e-5,
        10000.0,
    )
    .unwrap();
    let config = FileDecoderConfig::new(
        decoder,
        data::weights(),
        data::monitor(3.0),
        data::sampling(),
        5,
        DecoderBindingLimits::default(),
    )
    .unwrap();
    let (mut host, _reviewer) = FileOversight::create(root.store(), profile()).unwrap();
    host.enable_decoder(host.revision(), config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let numerical = host.decoder_inspection().unwrap().numerical;
    assert!(matches!(
        host.advance_decoder_forced(
            host.revision(),
            numerical.actor_revision,
            numerical.position,
            0,
            DecoderBudget {
                scalar_products: MAX_DECODER_PRODUCTS,
            },
        )
        .unwrap()
        .unwrap(),
        MonitoredStep::Released(_)
    ));
    host
}

#[test]
fn numerical_original_history_is_not_replayed_under_an_event_only_read_grant() {
    let root = Root::new();
    let mut host = numerical_owner(&root);
    assert!(matches!(
        host.submit_request(host.revision(), 901, spec(b"ok"), snapshot())
            .unwrap()
            .disposition,
        FileRequestDisposition::Admitted { .. }
    ));
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.explanation_port(1).unwrap();
    let before = supervisor.host().unwrap().inspect();
    let numerical = supervisor.host().unwrap().decoder_inspection().unwrap();
    let files = root.files();
    let attempts = proposal_replay_event_attempts();
    unknown(port.explain_request(901));
    assert_eq!(proposal_replay_event_attempts(), attempts);
    assert_eq!(grant.remaining_numerical_replay_events(), None);
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(port.explain_request(901), Err(ActorError::Capacity));
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(
        supervisor.host().unwrap().decoder_inspection().unwrap(),
        numerical
    );
    assert_eq!(root.files(), files);
}

fn raw_owner(root: &Root, predictor: bool) -> FileOversight {
    use crate::action::consequence::activation::consistency::{
        BinaryForecast, ErrorBudget, ForecastRegistration,
    };
    use crate::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
    use crate::action::consequence::delivery::persistent::observed::consistency::{
        FileConsistencyConfig, FileConsistencyParameters,
    };
    use crate::action::consequence::gate::containment::ActorState;

    let mut selected = profile();
    selected.delivery.actor = ActorState::new(
        selected.delivery.actor.profile(),
        vec![0],
        vec![1],
        vec![2],
        1,
    )
    .unwrap();
    let (mut host, _) = FileOversight::create(root.store(), selected).unwrap();
    let capture = CaptureProfile {
        tenant: 1,
        model: 9,
        model_generation: 1,
        tap: 4,
        layout_generation: 1,
    };
    let observer = predictor.then(|| {
        let config = FileConsistencyConfig::new(FileConsistencyParameters {
            probe_id: 1,
            probe_generation: 1,
            profile: capture,
            weights: vec![1.0],
            bias: 0.0,
            threshold: 0.0,
            forecast: ForecastRegistration {
                domain: 19,
                generation: 1,
                policy_generation: 1,
                event_prefix: b"ok".to_vec(),
                negative: BinaryForecast::new(16384, 49152).unwrap(),
                at_threshold: BinaryForecast::new(32768, 32768).unwrap(),
                positive: BinaryForecast::new(49152, 16384).unwrap(),
            },
            alpha: ErrorBudget::new(1, 4).unwrap(),
            stream: 17,
            max_predictions: 4,
            max_prediction_age_ticks: 10,
        })
        .unwrap();
        host.enable_action_consistency(host.revision(), config).unwrap()
    });
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    if let Some(observer) = observer {
        let frame = SourceFrame::capture(
            FrameIdentity {
                profile: capture,
                stream: 17,
                sequence: 1,
                position: 0,
            },
            &[1.0],
        )
        .unwrap();
        let revision = host.revision();
        let actor_revision = host.actor_snapshot().unwrap().actor_revision;
        let prediction = observer
            .forecast_request(&mut host, revision, 901, actor_revision, &frame)
            .unwrap()
            .unwrap();
        assert_eq!(prediction.forecast().null_numerator(), 49152);
    }
    host
}

#[test]
fn raw_predictor_only_admission_is_unknown_while_the_plain_control_remains_explainable() {
    for predictor in [false, true] {
        let root = Root::new();
        let mut host = raw_owner(&root, predictor);
        assert!(matches!(
            host.submit_request(host.revision(), 901, spec(b"ok"), snapshot())
                .unwrap()
                .disposition,
            FileRequestDisposition::Admitted { .. }
        ));
        assert!(!host.decoder_required());
        assert!(!host.learned_action_consistency_required());
        assert_eq!(host.action_consistency_required(), predictor);
        let forecast = predictor.then(|| host.action_consistency_snapshot().unwrap());
        let (_, supervisor) = host.into_actor_gateway();
        let (port, grant) = supervisor.explanation_port(1).unwrap();
        let before = supervisor.host().unwrap().inspect();
        let files = root.files();
        let attempts = proposal_replay_event_attempts();
        let result = port.explain_request(901);
        if predictor {
            unknown(result);
            assert_eq!(proposal_replay_event_attempts(), attempts);
        } else {
            let explained = known(result);
            assert_eq!(explained.basis().request, 901);
            assert_eq!(explained.basis().journal_revision, before.revision);
            assert_eq!(proposal_replay_event_attempts() - attempts, before.revision as usize);
        }
        assert_eq!(grant.remaining_numerical_replay_events(), None);
        assert_eq!(grant.remaining_queries(), 0);
        assert_eq!(port.explain_request(901), Err(ActorError::Capacity));

        // The same original raw prediction becomes explainable only when its
        // complete cut is explicitly funded. Plain history needs no reservation.
        let events = before.revision as usize;
        let (selected, selected_grant) = supervisor
            .explanation_port_with_numerical_budget(1, events, events)
            .unwrap();
        let attempts = proposal_replay_event_attempts();
        let explained = known(selected.explain_request(901));
        assert_eq!(explained.evaluation().result(), Truth::Satisfied);
        assert_eq!(explained.initial_state(), ActionState::Reviewing);
        assert_eq!(explained.basis().journal_revision, before.revision);
        assert_eq!(proposal_replay_event_attempts() - attempts, events);
        assert_eq!(
            selected_grant.remaining_numerical_replay_events(),
            Some(if predictor { 0 } else { events })
        );
        assert_eq!(selected_grant.remaining_queries(), 0);
        assert_eq!(supervisor.host().unwrap().inspect(), before);
        if let Some(forecast) = forecast {
            assert_eq!(
                supervisor.host().unwrap().action_consistency_snapshot().unwrap(),
                forecast
            );
        }
        assert_eq!(root.files(), files);
    }
}

#[test]
fn numerical_allowance_explains_actual_admission_and_denial_with_one_cut_per_shared_query() {
    for (payload, truth, stage) in [
        (b"ok".as_slice(), Truth::Satisfied, ActionState::Reviewing),
        (b"blocked".as_slice(), Truth::Violated, ActionState::Denied),
    ] {
        let root = Root::new();
        let mut host = numerical_owner(&root);
        let status = host
            .submit_request(host.revision(), 901, spec(payload), snapshot())
            .unwrap();
        assert_eq!(
            status.disposition,
            FileRequestDisposition::Admitted { attempt: 1, stage }
        );
        let original = host
            .investigate_proposal(
                1,
                InterventionScope::new(901, false, false, false, &[]).unwrap(),
            )
            .unwrap();
        let events = host.revision() as usize;
        let (_, mut supervisor) = host.into_actor_gateway();
        let (port, grant) = supervisor
            .explanation_port_with_numerical_budget(3, events, events * 2)
            .unwrap();
        let before = supervisor.host().unwrap().inspect();
        let numerical = supervisor.host().unwrap().decoder_inspection().unwrap();
        supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
        let files = root.files();
        let attempts = proposal_replay_event_attempts();

        let explained = known(port.explain_request(901));
        assert_eq!(explained.policy(), original.origin().proposal().policy.as_ref());
        assert_eq!(explained.evaluation(), &original.origin().proposal().evaluation);
        assert_eq!(explained.evaluation().result(), truth);
        assert_eq!(explained.initial_state(), stage);
        assert_eq!(explained.ending_state(), stage);
        assert_eq!((explained.basis().request, explained.basis().attempt), (901, 1));
        assert_eq!(
            explained.basis().admission_revision,
            original.origin().admission_snapshot().revision
        );
        assert_eq!(explained.basis().journal_revision, before.revision);
        assert_eq!(grant.remaining_queries(), 2);
        assert_eq!(grant.remaining_numerical_replay_events(), Some(events));
        assert_eq!(proposal_replay_event_attempts() - attempts, events);

        assert_eq!(known(port.clone().explain_request(901)), explained);
        assert_eq!(grant.remaining_queries(), 1);
        assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
        assert_eq!(proposal_replay_event_attempts() - attempts, events * 2);
        unknown(port.explain_request(901));
        assert_eq!(grant.remaining_queries(), 0);
        assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
        assert_eq!(proposal_replay_event_attempts() - attempts, events * 2);
        assert_eq!(port.explain_request(901), Err(ActorError::Capacity));
        assert!(supervisor.owner.borrow().snapshot.is_some());
        assert_eq!(supervisor.host().unwrap().inspect(), before);
        assert_eq!(
            supervisor.host().unwrap().decoder_inspection().unwrap(),
            numerical
        );
        assert_eq!(root.files(), files);
    }
}

#[test]
fn short_numerical_or_event_allowance_stops_before_any_original_admission_replay() {
    let root = Root::new();
    let mut host = numerical_owner(&root);
    host.submit_request(host.revision(), 901, spec(b"ok"), snapshot()).unwrap();
    let events = host.revision() as usize;
    assert!(events > 1);
    let (_, supervisor) = host.into_actor_gateway();
    let (short, short_grant) = supervisor
        .explanation_port_with_numerical_budget(1, events, events - 1)
        .unwrap();
    let (narrow, narrow_grant) = supervisor
        .explanation_port_with_numerical_budget(2, events - 1, events)
        .unwrap();
    let (exact, exact_grant) = supervisor
        .explanation_port_with_numerical_budget(1, events, events)
        .unwrap();
    let before = supervisor.host().unwrap().inspect();
    let numerical = supervisor.host().unwrap().decoder_inspection().unwrap();
    let files = root.files();
    let attempts = proposal_replay_event_attempts();

    unknown(short.explain_request(901));
    assert_eq!(short_grant.remaining_queries(), 0);
    assert_eq!(short_grant.remaining_numerical_replay_events(), Some(events - 1));
    assert_eq!(proposal_replay_event_attempts(), attempts);
    unknown(narrow.explain_request(901));
    assert_eq!(narrow_grant.remaining_queries(), 1);
    assert_eq!(narrow_grant.remaining_numerical_replay_events(), Some(events));
    assert_eq!(proposal_replay_event_attempts(), attempts);
    assert_eq!(known(exact.explain_request(901)).basis().journal_revision, before.revision);
    assert_eq!(exact_grant.remaining_queries(), 0);
    assert_eq!(exact_grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(proposal_replay_event_attempts() - attempts, events);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(supervisor.host().unwrap().decoder_inspection().unwrap(), numerical);
    assert_eq!(root.files(), files);
}

#[test]
fn missing_and_unadmitted_requests_do_not_reserve_a_numerical_cut_or_invent_admission() {
    let root = Root::new();
    let mut host = numerical_owner(&root);
    host.submit_request(host.revision(), 901, spec(b"ok"), snapshot()).unwrap();
    let mut invalid = spec(b"ok");
    invalid.target.as_mut().unwrap().expected_version += 1;
    assert!(matches!(
        host.submit_request(host.revision(), 902, invalid, snapshot())
            .unwrap().disposition,
        FileRequestDisposition::NotAdmitted(_)
    ));
    let events = host.revision() as usize;
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .explanation_port_with_numerical_budget(4, events, events)
        .unwrap();
    let before = supervisor.host().unwrap().inspect();
    let files = root.files();
    let attempts = proposal_replay_event_attempts();
    assert_eq!(port.explain_request(0), Err(ActorError::MalformedProposal));
    assert_eq!(grant.remaining_queries(), 4);
    // Attempt 1 belongs to request 901; request 1 does not acquire its history.
    for (request, remaining) in [(999, 3), (902, 2), (1, 1)] {
        unknown(port.explain_request(request));
        assert_eq!(grant.remaining_queries(), remaining);
        assert_eq!(grant.remaining_numerical_replay_events(), Some(events));
        assert_eq!(proposal_replay_event_attempts(), attempts);
    }
    assert_eq!(known(port.explain_request(901)).basis().attempt, 1);
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(proposal_replay_event_attempts() - attempts, events);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), files);
}

#[test]
fn later_numerical_admission_queries_pay_the_advanced_cut_without_rewriting_original_policy() {
    let root = Root::new();
    let mut host = numerical_owner(&root);
    host.submit_request(host.revision(), 901, spec(b"ok"), snapshot()).unwrap();
    let (_, mut supervisor) = host.into_actor_gateway();
    let total = 2 * MAX_EXPLANATION_EVENTS;
    let (port, grant) = supervisor
        .explanation_port_with_numerical_budget(2, MAX_EXPLANATION_EVENTS, total)
        .unwrap();
    let attempts = proposal_replay_event_attempts();
    let admitted = known(port.explain_request(901));
    let first_events = admitted.basis().journal_revision as usize;
    assert_eq!(grant.remaining_numerical_replay_events(), Some(total - first_events));
    assert_eq!(proposal_replay_event_attempts() - attempts, first_events);
    let later_policy = Policy::new(2, vec![Predicate::PayloadAtMost(1)]).unwrap();
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.cancel_request(revision, 901).unwrap();
        let state = host.inspect();
        let update = PolicyUpdate::new(
            61,
            state.control.sequence,
            state.control.ledger.epoch,
            later_policy.clone(),
        )
        .unwrap();
        let revision = host.revision();
        host.replace_policy(revision, &update).unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(5)).unwrap();
    }
    let before = supervisor.host().unwrap().inspect();
    let numerical = supervisor.host().unwrap().decoder_inspection().unwrap();
    let files = root.files();
    let later = known(port.explain_request(901));
    assert_eq!(later.policy(), admitted.policy());
    assert_eq!(later.evaluation(), admitted.evaluation());
    assert_ne!(later.policy(), &later_policy);
    assert_eq!(later.initial_state(), ActionState::Reviewing);
    assert_eq!(later.ending_state(), ActionState::Cancelled);
    assert_eq!(later.basis().admission_revision, admitted.basis().admission_revision);
    assert_eq!(later.basis().admitted_at, Some(ElapsedTick(1)));
    assert_eq!(later.basis().recorded_at, Some(ElapsedTick(5)));
    assert_eq!(later.basis().journal_revision, before.revision);
    let all_events = first_events + before.revision as usize;
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(total - all_events));
    assert_eq!(proposal_replay_event_attempts() - attempts, all_events);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(supervisor.host().unwrap().decoder_inspection().unwrap(), numerical);
    assert_eq!(root.files(), files);
}

#[test]
fn numerical_admission_selection_obeys_query_event_limits_and_preserves_plain_history_cost() {
    let root = Root::new();
    let mut host = owner(&root);
    host.submit_request(host.revision(), 901, spec(b"ok"), snapshot()).unwrap();
    let (_, supervisor) = host.into_actor_gateway();
    for (queries, events, numerical) in [(0, 1, 1), (1, 0, 1), (1, 1, 0)] {
        assert!(matches!(
            supervisor.explanation_port_with_numerical_budget(queries, events, numerical),
            Err(JournalError::Contract(Error::InvalidInput))
        ));
    }
    for (queries, events, numerical) in [
        (MAX_EXPLANATION_QUERIES + 1, 1, 1),
        (1, MAX_EXPLANATION_EVENTS + 1, 1),
        (1, 1, 2),
        (2, 3, 7),
    ] {
        assert!(matches!(
            supervisor.explanation_port_with_numerical_budget(queries, events, numerical),
            Err(JournalError::Contract(Error::Limit))
        ));
    }
    let (_, boundary) = supervisor.explanation_port_with_numerical_budget(2, 3, 6).unwrap();
    assert_eq!(boundary.remaining_queries(), 2);
    assert_eq!(boundary.max_replay_events(), 3);
    assert_eq!(boundary.remaining_numerical_replay_events(), Some(6));
    let events = supervisor.host().unwrap().revision() as usize;
    let (port, grant) = supervisor
        .explanation_port_with_numerical_budget(1, events, 1)
        .unwrap();
    let before = supervisor.host().unwrap().inspect();
    let files = root.files();
    let attempts = proposal_replay_event_attempts();
    assert_eq!(known(port.explain_request(901)).initial_state(), ActionState::Reviewing);
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(1));
    assert_eq!(proposal_replay_event_attempts() - attempts, events);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), files);
}
