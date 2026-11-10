//! Explicit numerical-history transition reservations, separate from query and
//! per-query event limits. These are not scalar-products or physical-time meters.
use super::*;
use crate::action::consequence::activation::monitor::decoder::MonitoredStep;
use crate::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderIdentity, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS,
};
use crate::action::consequence::delivery::persistent::observed::decoder::FileDecoderConfig;
use crate::action::consequence::delivery::persistent::observed::replay::review_replay_event_attempts;
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
fn numerical_original_history_is_not_replayed_under_an_event_only_review_grant() {
    let root = Root::new();
    let mut host = numerical_owner(&root);
    completed(&mut host, 901, 101);
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.review_explanation_port(1).unwrap();
    let before = supervisor.host().unwrap().inspect();
    let numerical = supervisor.host().unwrap().decoder_inspection().unwrap();
    let files = root.files();
    let attempts = review_replay_event_attempts();
    unknown(port.explain_review(901, 101));
    assert_eq!(review_replay_event_attempts(), attempts);
    assert_eq!(grant.remaining_numerical_replay_events(), None);
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(port.explain_review(901, 101), Err(ActorError::Capacity));
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(
        supervisor.host().unwrap().decoder_inspection().unwrap(),
        numerical
    );
    assert_eq!(root.files(), files);
}

#[test]
fn explicit_numerical_reservations_share_cumulative_work_across_port_clones() {
    let root = Root::new();
    let mut host = numerical_owner(&root);
    let (_, receipt) = completed(&mut host, 901, 101);
    let events = host.revision() as usize;
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .review_explanation_port_with_numerical_budget(3, events, events * 2)
        .unwrap();
    let clone = port.clone();
    let before = supervisor.host().unwrap().inspect();
    let numerical = supervisor.host().unwrap().decoder_inspection().unwrap();
    let files = root.files();
    let attempts = review_replay_event_attempts();
    let explained = known(port.explain_review(901, 101));
    assert_eq!(explained.basis().journal_revision, events as u64);
    assert_eq!(
        explained.application().unwrap().control_sequence,
        receipt.policy.control.sequence
    );
    assert_eq!(grant.remaining_numerical_replay_events(), Some(events));
    assert_eq!(review_replay_event_attempts() - attempts, events);
    assert_eq!(known(clone.explain_review(901, 101)), explained);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(review_replay_event_attempts() - attempts, events * 2);
    unknown(port.explain_review(901, 101));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(review_replay_event_attempts() - attempts, events * 2);
    assert_eq!(clone.explain_review(901, 101), Err(ActorError::Capacity));
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(supervisor.host().unwrap().decoder_inspection().unwrap(), numerical);
    assert_eq!(root.files(), files);
}

#[test]
fn one_short_numerical_cut_is_refused_before_any_original_replay_event() {
    let root = Root::new();
    let mut host = numerical_owner(&root);
    completed(&mut host, 901, 101);
    let events = host.revision() as usize;
    let (_, supervisor) = host.into_actor_gateway();
    let (short, short_grant) = supervisor
        .review_explanation_port_with_numerical_budget(1, events, events - 1)
        .unwrap();
    let (exact, exact_grant) = supervisor
        .review_explanation_port_with_numerical_budget(1, events, events)
        .unwrap();
    let before = supervisor.host().unwrap().inspect();
    let numerical = supervisor.host().unwrap().decoder_inspection().unwrap();
    let files = root.files();
    let attempts = review_replay_event_attempts();
    unknown(short.explain_review(901, 101));
    assert_eq!(review_replay_event_attempts(), attempts);
    assert_eq!(short_grant.remaining_queries(), 0);
    assert_eq!(short_grant.remaining_numerical_replay_events(), Some(events - 1));
    // Only the independent numerical allowance changes; the same original
    // request, round, immutable recipe and acknowledged cut now complete.
    known(exact.explain_review(901, 101));
    assert_eq!(review_replay_event_attempts() - attempts, events);
    assert_eq!(exact_grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(supervisor.host().unwrap().decoder_inspection().unwrap(), numerical);
    assert_eq!(root.files(), files);
}

#[test]
fn complete_numerical_reservation_survives_unknown_attribution_without_refund() {
    let root = Root::new();
    let mut host = numerical_owner(&root);
    completed(&mut host, 901, 101);
    admit(&mut host, 902, b"ok");
    let events = host.revision() as usize;
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .review_explanation_port_with_numerical_budget(3, events, events)
        .unwrap();
    let before = supervisor.host().unwrap().inspect();
    let numerical = supervisor.host().unwrap().decoder_inspection().unwrap();
    let files = root.files();
    let attempts = review_replay_event_attempts();
    // A nonexistent round cannot start the original cursor and spends no work.
    unknown(port.explain_review(902, 102));
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(events));
    assert_eq!(review_replay_event_attempts(), attempts);
    // A real completed round belonging to another request replays successfully,
    // then refuses attribution. Its complete reservation remains charged.
    unknown(port.explain_review(902, 101));
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(review_replay_event_attempts() - attempts, events);
    unknown(port.explain_review(901, 101));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(review_replay_event_attempts() - attempts, events);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(supervisor.host().unwrap().decoder_inspection().unwrap(), numerical);
    assert_eq!(root.files(), files);
}

#[test]
fn numerical_reservation_does_not_widen_per_query_event_limit_or_plain_history_cost() {
    let root = Root::new();
    let mut host = numerical_owner(&root);
    completed(&mut host, 901, 101);
    let events = host.revision() as usize;
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .review_explanation_port_with_numerical_budget(2, events - 1, events)
        .unwrap();
    let attempts = review_replay_event_attempts();
    unknown(port.explain_review(901, 101));
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(events));
    assert_eq!(review_replay_event_attempts(), attempts);

    let plain_root = Root::new();
    let mut plain = owner(&plain_root);
    completed(&mut plain, 901, 101);
    let (_, plain_supervisor) = plain.into_actor_gateway();
    let (plain_port, plain_grant) = plain_supervisor
        .review_explanation_port_with_numerical_budget(1, MAX_REVIEW_EXPLANATION_EVENTS, 1)
        .unwrap();
    known(plain_port.explain_review(901, 101));
    assert_eq!(plain_grant.remaining_queries(), 0);
    assert_eq!(plain_grant.remaining_numerical_replay_events(), Some(1));
}

#[test]
fn numerical_grant_validates_selected_query_and_event_ceiling() {
    let root = Root::new();
    let (_, supervisor) = owner(&root).into_actor_gateway();
    for (queries, events, numerical) in [(0, 1, 1), (1, 0, 1), (1, 1, 0)] {
        assert!(matches!(
            supervisor.review_explanation_port_with_numerical_budget(queries, events, numerical),
            Err(JournalError::Contract(Error::InvalidInput))
        ));
    }
    for (queries, events, numerical) in [
        (1, 1, 2),
        (2, 3, 7),
        (MAX_REVIEW_EXPLANATION_QUERIES + 1, 1, 1),
        (1, MAX_REVIEW_EXPLANATION_EVENTS + 1, 1),
        (usize::MAX, usize::MAX, usize::MAX),
    ] {
        assert!(matches!(
            supervisor.review_explanation_port_with_numerical_budget(queries, events, numerical),
            Err(JournalError::Contract(Error::Limit))
        ));
    }
    let (_port, grant) = supervisor
        .review_explanation_port_with_numerical_budget(2, 3, 6)
        .unwrap();
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(grant.max_replay_events(), 3);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(6));
}

#[test]
fn raw_forecast_without_decoder_also_requires_its_explicit_numerical_reservation() {
    use crate::action::consequence::activation::consistency::{
        BinaryForecast, ErrorBudget, ForecastRegistration,
    };
    use crate::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
    use crate::action::consequence::delivery::persistent::observed::consistency::{
        FileConsistencyConfig, FileConsistencyParameters,
    };
    use crate::action::consequence::gate::containment::ActorState;

    let root = Root::new();
    let mut selected = profile();
    selected.delivery.actor = ActorState::new(
        selected.delivery.actor.profile(),
        vec![0],
        vec![1],
        vec![2],
        1,
    )
    .unwrap();
    let capture = CaptureProfile {
        tenant: 1,
        model: 9,
        model_generation: 1,
        tap: 4,
        layout_generation: 1,
    };
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
    let (mut host, _) = FileOversight::create(root.store(), selected).unwrap();
    let observer = host.enable_action_consistency(host.revision(), config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
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
    completed(&mut host, 901, 101);
    assert!(!host.decoder_required());
    assert!(!host.learned_action_consistency_required());
    assert!(host.action_consistency_required());
    let events = host.revision() as usize;
    let (_, supervisor) = host.into_actor_gateway();
    let before = supervisor.host().unwrap().inspect();
    let forecast = supervisor.host().unwrap().action_consistency_snapshot().unwrap();
    let files = root.files();
    let attempts = review_replay_event_attempts();

    let (default, default_grant) = supervisor.review_explanation_port(1).unwrap();
    unknown(default.explain_review(901, 101));
    assert_eq!(default_grant.remaining_queries(), 0);
    assert_eq!(review_replay_event_attempts(), attempts);
    let (short, short_grant) = supervisor
        .review_explanation_port_with_numerical_budget(1, events, events - 1)
        .unwrap();
    unknown(short.explain_review(901, 101));
    assert_eq!(short_grant.remaining_queries(), 0);
    assert_eq!(short_grant.remaining_numerical_replay_events(), Some(events - 1));
    assert_eq!(review_replay_event_attempts(), attempts);
    let (exact, exact_grant) = supervisor
        .review_explanation_port_with_numerical_budget(1, events, events)
        .unwrap();
    assert_eq!(known(exact.explain_review(901, 101)).decision().consequence, Consequence::Continue);
    assert_eq!(exact_grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(review_replay_event_attempts() - attempts, events);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(supervisor.host().unwrap().action_consistency_snapshot().unwrap(), forecast);
    assert_eq!(root.files(), files);
}
