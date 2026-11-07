//! Event count is not a numerical-products budget. Real numerical ownership
//! therefore has a charged Unknown counterpart to ordinary policy explanation.
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
fn numerical_original_history_is_not_replayed_under_an_event_only_read_grant() {
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
    unknown(port.explain_request(901));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(port.explain_request(901), Err(ActorError::Capacity));
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(
        supervisor.host().unwrap().decoder_inspection().unwrap(),
        numerical
    );
    assert_eq!(root.files(), files);
}
