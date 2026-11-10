//! Current acknowledged numerical owners can disclose recorded publication
//! conditions without replaying decoder products or granting execution.
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
fn acknowledged_numerical_owner_yields_recorded_conditions_without_replay() {
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
    let (port, grant) = supervisor.hold_diagnosis_port(1).unwrap();
    let before = supervisor.host().unwrap().inspect();
    let numerical = supervisor.host().unwrap().decoder_inspection().unwrap();
    let files = root.files();
    let value = known(port.why_held(901));
    assert_eq!(value.stage(), ActionState::Reviewing);
    assert_eq!((value.basis().request, value.basis().attempt), (901, 1));
    assert_eq!(value.basis().journal_revision, before.revision);
    assert_eq!(value.basis().control_sequence, before.control.sequence);
    recorded(&value, FileHoldCondition::RecordedStageAuthorized, false);
    recorded(&value, FileHoldCondition::DispatchNotRecorded, true);
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(port.why_held(901), Err(ActorError::Capacity));
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(
        supervisor.host().unwrap().decoder_inspection().unwrap(),
        numerical
    );
    assert_eq!(root.files(), files);
}
