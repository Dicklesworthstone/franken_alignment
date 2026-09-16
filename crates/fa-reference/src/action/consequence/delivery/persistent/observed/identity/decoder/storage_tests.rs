//! New numerical runner at every ORIGINAL canonical replacement barrier.
//! Registered below the existing identity test fixture; no new storage seam.
use super::*;
use crate::action::consequence::activation::identity::decoder::DecoderIdentityProbe;
use crate::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderIdentity, DecoderLayerWeights, DecoderModel, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS,
};

fn computed_model(changed: bool) -> (DecoderModel, ModelPassport) {
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 9, model_generation: 1,
        tokenizer_generation: 1, profile_generation: 1 }, DecoderShape { vocabulary: 2, hidden: 2,
        intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 2 }, 1e-5, 10000.0).unwrap();
    let model = DecoderModel::new(profile, vec![if changed { 2.0 } else { 1.0 }, 0.0, 1.0, 0.0],
        vec![DecoderLayerWeights { attention_norm: vec![1.0; 2], queries: vec![0.0; 4], keys: vec![0.0; 4],
            values: vec![0.0; 4], attention_output: vec![0.0; 4], feed_forward_norm: vec![1.0; 2],
            gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4] }], vec![1.0; 2], vec![0.0; 4]).unwrap();
    let expected = ModelPassport::new(51, 1, passport().manifest().clone(), vec![IdentityAnchor::new(10,
        model.residual_contract(1).unwrap().profile(), 5, vec![0], &[[1.0, 1.0], [0.0, 0.0]]).unwrap()]).unwrap();
    (model, expected)
}

#[test]
fn numerical_runner_does_not_retry_measurement_installation_or_mismatch_after_storage_failure() {
    // 0: matching measurement; 1: installation after a matched measurement;
    // 2: real parameter mismatch with original containment in the same write.
    for site in 0..3 {
        for stage in BARRIERS {
            let root = Directory::new();
            let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
            let (model, expected) = computed_model(site == 2);
            let observer = host.enable_identity_checks(host.revision(), expected.clone(), policy()).unwrap();
            host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
            let control = host.inspect().control;
            let check = host.begin_identity_check(host.revision(), 1, control.sequence,
                host.actor_snapshot().unwrap().actor_revision).unwrap().unwrap();
            let probe = DecoderIdentityProbe::new(model, &expected, 1,
                DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap();
            let mut run = observer.decoder_probe(&host, &check, probe, expected.manifest().clone()).unwrap();
            run.step_with_clock(&mut host, &observer, || ElapsedTick(1)).unwrap();
            if site == 1 {
                run.step_with_clock(&mut host, &observer, || ElapsedTick(1)).unwrap();
                assert_eq!(host.identity_report(1).unwrap().outcome, IdentityOutcome::Matched);
            }
            let before = host.inspect();
            host.store.fail_once(stage);
            fault(run.step_with_clock(&mut host, &observer, || ElapsedTick(1)).unwrap_err(), stage);
            assert!(run.is_closed()); assert_eq!(run.work().completed_tokens, 1);
            assert_eq!(host.inspect(), before);
            assert_eq!(host.identity_status(), Err(JournalError::Unavailable));
            let work = run.work();
            assert!(run.step_with_clock(&mut host, &observer, || panic!("automatic retry after I/O fault")).is_err());
            assert_eq!(run.work(), work);
            let disk = canonical(&host);
            let visible = stage == JournalIo::DirectorySync;
            let report = disk.broker.identity_report(1).unwrap();
            assert_eq!(report.observations.len(), usize::from(site == 1 || visible));
            assert_eq!(disk.broker.identity_installation(1).unwrap().is_some(), visible && site != 0);
            assert_eq!(disk.broker.inspect().suspended, visible && site == 2);
            drop(host);
            let (mut host, _, observer) = FileOversight::open_with_identity_observer(root.store(), profile(), &expected, policy()).unwrap();
            host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
            if visible && site == 2 {
                assert_eq!(host.identity_status().unwrap(), IdentityStatus::Mismatch { check: 1 });
                assert!(host.inspect().control.suspended);
            } else {
                assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
                let control = host.inspect().control;
                let check = host.begin_identity_check(host.revision(), 2, control.sequence,
                    host.actor_snapshot().unwrap().actor_revision).unwrap().unwrap();
                let (model, _) = computed_model(false);
                let probe = DecoderIdentityProbe::new(model, &expected, 2,
                    DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap();
                let mut fresh = observer.decoder_probe(&host, &check, probe, expected.manifest().clone()).unwrap();
                for _ in 0..3 { fresh.step_with_clock(&mut host, &observer, || ElapsedTick(2)).unwrap(); }
                assert!(fresh.is_closed());
                assert!(matches!(host.identity_status().unwrap(), IdentityStatus::Matching { check: 2, .. }));
            }
        }
    }
}
