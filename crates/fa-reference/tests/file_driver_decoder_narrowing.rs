//! Original numerical source and actual helper sockets across target restriction.
//! Deterministic model coefficients test custody, not detector effectiveness.
#![cfg(unix)]
#[path = "support/file_driver.rs"]
mod fixture;
#[path = "support/decoder_inputs.rs"]
mod data;

use fa_reference::action::consequence::activation::monitor::decoder::MonitoredStep;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderIdentity, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS,
};
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileOversight, decoder::FileDecoderConfig, driver::FileDriverEvent,
    narrowing::FileAuthorityNarrowRequest,
};
use fa_reference::action::consequence::oversight::{
    actor::{ActorOutcome, Knowledge}, decoder_monitoring::DecoderBindingLimits,
};
use fixture::Rig;

const REQUEST: u64 = 90;

fn advance(host: &mut FileOversight) {
    let numerical = host.decoder_inspection().unwrap().numerical;
    assert!(matches!(host.advance_decoder_forced(host.revision(), numerical.actor_revision,
        numerical.position, 0, DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS })
        .unwrap().unwrap(), MonitoredStep::Released(_)));
}

fn rig() -> Rig {
    let mut rig = Rig::new();
    {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
        let decoder = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 9,
            model_generation: 1, tokenizer_generation: 1, profile_generation: 1 },
            DecoderShape { vocabulary: 2, hidden: 2, intermediate: 2, layers: 1,
                query_heads: 1, cache_heads: 1, context: 4 }, 1e-5, 10000.0).unwrap();
        let config = FileDecoderConfig::new(decoder, data::weights(), data::monitor(3.0),
            data::sampling(), 5, DecoderBindingLimits::default()).unwrap();
        let revision = host.revision();
        host.enable_decoder(revision, config).unwrap();
        advance(&mut host);
    }
    rig
}

fn retain(rig: &mut Rig, operation: u64) {
    let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
    let state = host.inspect();
    let target = host.request_action(REQUEST).unwrap().spec().target.unwrap();
    let request = FileAuthorityNarrowRequest::new(operation, state.control.sequence,
        state.control.ledger.epoch, vec![target]).unwrap();
    let receipt = host.narrow_authority(state.revision, request).unwrap();
    assert!(receipt.change().cancelled.is_empty());
    assert_eq!(receipt.change().refunded_units, 0);
}

#[test]
fn original_decoder_review_can_publish_after_narrowing_and_final_source_check_remains_required() {
    for advance_before_publication in [false, true] {
        let mut rig = rig();
        let ticket = rig.submit(REQUEST);
        rig.reviewed(REQUEST);
        let numerical = rig.driver.supervisor().host().unwrap().decoder_inspection().unwrap().numerical;
        let old_human = rig.human(1001, 30);
        retain(&mut rig, 501);
        let state = rig.driver.supervisor().host().unwrap().inspect();
        let bytes = std::fs::read(rig.root.store().join("delivery.bin")).unwrap();
        assert_eq!(rig.driver.pending_narrowing_continuation(REQUEST).unwrap(), Some(501));
        let continuation = rig.driver.continue_after_narrowing(REQUEST, 501).unwrap();
        assert_eq!(rig.driver.continue_after_narrowing(REQUEST, 501).unwrap(), continuation);
        assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), state);
        assert_eq!(std::fs::read(rig.root.store().join("delivery.bin")).unwrap(), bytes);
        assert_eq!(rig.driver.supervisor().host().unwrap().decoder_inspection().unwrap().numerical,
            numerical);
        drop(old_human);
        let human = rig.human(1002, 30);
        assert!(matches!(rig.step(Some(&human)), FileDriverEvent::Dispatched { request: REQUEST, .. }));
        if advance_before_publication {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            advance(&mut host);
        }
        let FileDriverEvent::PublicationChecked { publication, .. } = rig.step(None)
            else { panic!("original guarded publication was not checked"); };
        assert_eq!(matches!(publication.outcome, EndpointOutcome::Executed { resulting_version: 2 }),
            !advance_before_publication);
        assert!(matches!(rig.step(None), FileDriverEvent::Reconciled { .. }));
        let state = rig.driver.supervisor().host().unwrap().inspect();
        assert_eq!(state.executions, u64::from(!advance_before_publication));
        let Knowledge::Known { value, .. } = rig.port.poll(&ticket)
            else { panic!("original ticket did not settle"); };
        assert_eq!(value, if advance_before_publication {
            ActorOutcome::ConfirmedNotExecuted
        } else { ActorOutcome::Executed });
    }
}

#[test]
fn advancing_original_decoder_source_refuses_read_only_continuation() {
    let mut rig = rig();
    rig.submit(REQUEST);
    rig.reviewed(REQUEST);
    {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
        advance(&mut host);
    }
    retain(&mut rig, 501);
    let state = rig.driver.supervisor().host().unwrap().inspect();
    let numerical = rig.driver.supervisor().host().unwrap().decoder_inspection().unwrap();
    let bytes = std::fs::read(rig.root.store().join("delivery.bin")).unwrap();
    assert!(rig.driver.pending_narrowing_continuation(REQUEST).is_err());
    assert!(rig.driver.continue_after_narrowing(REQUEST, 501).is_err());
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), state);
    assert_eq!(rig.driver.supervisor().host().unwrap().decoder_inspection().unwrap(), numerical);
    assert_eq!(std::fs::read(rig.root.store().join("delivery.bin")).unwrap(), bytes);
    assert_eq!(state.executions, 0);
}
