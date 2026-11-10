//! The event-only grant never replays an actual numerical owner's history.
//! Ordinary original receipt resolution has positive counterparts in tests.rs.
use super::*;
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

#[test]
fn numerical_dispatched_history_is_not_replayed_under_an_event_only_operation_grant() {
    let root = Root::new();
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
    let (mut host, reviewer) = FileOversight::create(root.store(), profile()).unwrap();
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
    assert!(matches!(
        host.submit_request(host.revision(), 901, spec(b"ok"), snapshot())
            .unwrap()
            .disposition,
        FileRequestDisposition::Admitted { .. }
    ));
    dispatch_existing(&mut host, &reviewer, 901);
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .resolution_port(1, 1, MAX_RESOLUTION_REPLAY_EVENTS)
        .unwrap();
    let before = supervisor.host().unwrap().inspect();
    let numerical = supervisor.host().unwrap().decoder_inspection().unwrap();
    let files = root.files();
    let offered = plan(port.next(901, 171));
    assert_eq!(offered.stage(), ActionState::Dispatching);
    assert_eq!((offered.basis().request, offered.basis().attempt), (901, 1));
    assert_eq!(offered.basis().journal_revision, before.revision);
    assert_eq!(
        offered.block_reason(),
        Some(FileResolutionBlockReason::NumericalReplayUnsupported)
    );
    assert_eq!(offered.request(), None);
    assert_eq!(
        port.resolve(FileResolutionRequest::new(171, offered.basis()).unwrap()),
        Err(FileResolutionError::Unsupported)
    );
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(before.control.ledger.charged, 8);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(
        supervisor.host().unwrap().decoder_inspection().unwrap(),
        numerical
    );
    assert_eq!(root.files(), files);
}
