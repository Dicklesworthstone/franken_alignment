//! Real computed residuals -> fit -> calibration -> evaluation -> original gates.
//! Tiny deterministic weights/labels are algorithm controls, not safety evidence.
#[path = "support/decoder_fixture.rs"]
#[allow(dead_code)]
mod decoder_fixture;
#[path = "support/probe_corpus.rs"]
#[allow(dead_code)]
mod corpus_fixture;
#[path = "support/decoder_control.rs"]
#[allow(dead_code)]
mod control;
use fa_reference::action::consequence::activation::monitor::{RefinementBudget, RefinementMonitor, MonitorOutcome};
use fa_reference::action::consequence::activation::monitor::decoder::{MonitoredDecoder, MonitoredStep, MonitoringStatus};
use fa_reference::action::consequence::activation::probe::training::{ProbeCorpus, DataSplit};
use fa_reference::action::consequence::activation::probe::training::calibration::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::*;
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use fa_reference::Error;
use std::collections::BTreeMap;

fn compute() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
fn trained() -> (DecoderModel, EvaluationReport) {
    let p = DecoderProfile::new(DecoderIdentity { model_generation: 1, tokenizer_generation: 1,
        ..decoder_fixture::profile(8).identity() }, DecoderShape {
        vocabulary: 2, hidden: 2, intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 8,
    }, 0.00001, 10000.0).unwrap();
    let layers = decoder_fixture::zero_layers(&p);
    let model = DecoderModel::new(p, vec![1.0, 0.0, -1.0, 0.0], layers,
        vec![1.0; 2], vec![1.0, 0.0, -1.0, 0.0]).unwrap();
    let contract = model.residual_contract(1).unwrap();
    let mut builder = ProbeCorpus::new(1, 1, contract.profile(), contract.dimensions(), corpus_fixture::assignments()).unwrap();
    for id in 1..=12 {
        let token = if id % 2 == 0 { 1 } else { 0 };
        let step = model.session(id + 100).unwrap().advance(0, token, compute()).unwrap();
        builder.capture(corpus_fixture::origin(id), corpus_fixture::label(id), step.layers[0].residual.source().clone()).unwrap();
    }
    let sealed = builder.seal().unwrap();
    assert_eq!(sealed.counts(DataSplit::Evaluation).total(), 4);
    let fitted = corpus_fixture::fit(&sealed);
    assert!(fitted.weights()[0] < 0.0);
    let rules = ScreeningCriteria::new(2, 0).unwrap();
    let config = CalibrationPolicy::new(1, 1, &[-10.0, 0.0, 10.0], rules, rules).unwrap();
    let budget = ScoringBudget { encoded_bytes: 100_000, probe_coordinates: 1000, threshold_comparisons: 1000 };
    let evaluated = fitted.calibrate(config, budget).unwrap().evaluate(budget).unwrap();
    assert!(evaluated.accepted());
    assert_eq!(evaluated.counts().violation_alarm, 2);
    (model, evaluated)
}
fn monitored(model: DecoderModel, result: &EvaluationReport) -> MonitoredDecoder {
    let budget = RefinementBudget { encoded_bytes: 100_000, probe_coordinates: 100_000 };
    let monitor = RefinementMonitor::new(vec![result.probe().unwrap()], vec![0, 23], budget).unwrap();
    MonitoredDecoder::new(model, 7, 11, BTreeMap::from([(1, monitor)]), budget).unwrap()
}

#[test]
fn fitted_actual_residuals_release_benign_computation_and_hold_the_opposite_case() {
    let (model, report) = trained();
    let mut run = monitored(model, &report);
    let observer = run.observation();
    let good = run.advance(0, 0, compute()).unwrap();
    assert!(matches!(good, MonitoredStep::Released(_)));
    let good_evidence = observer.capture().unwrap();
    assert_eq!(good_evidence.tokens(), &[0]);
    let bad = run.advance(1, 1, compute()).unwrap();
    assert!(matches!(bad, MonitoredStep::Held(_)));
    assert_eq!(bad.review().outcome(), MonitorOutcome::Alarm);
    assert_eq!(run.status(), MonitoringStatus::Held);
    assert_eq!(observer.validate(&good_evidence), Err(Error::Incomplete));
    assert!(matches!(run.advance(2, 0, compute()), Err(Error::WrongState)));
    assert_eq!(run.decoder_work().tokens, 2);
    assert_eq!(report.counts().classes().total(), 4);
}

#[test]
fn learned_alarm_blocks_original_preissued_permit_while_near_identical_quiet_control_publishes() {
    let (model, report) = trained();
    for hold in [false, true] {
        let mut run = monitored(model.clone(), &report);
        let mut f = control::Fixture::new(false);
        f.broker.enable_decoder_monitoring(run.observation(), DecoderBindingLimits::default()).unwrap();
        assert!(matches!(run.advance(0, 0, compute()).unwrap(), MonitoredStep::Released(_)));
        f.broker.replace_actor_state(f.broker.actor_revision(), control::actor(&[0])).unwrap();
        let (action, inputs, permit) = control::approved(&mut f, 1);
        if hold { assert!(matches!(run.advance(1, 1, compute()).unwrap(), MonitoredStep::Held(_))); }
        let result = f.broker.dispatch(&permit, &action, Some(&inputs), &control::snapshot());
        if hold {
            assert!(matches!(result, Err(Error::Incomplete)));
            assert_eq!(f.broker.inspect().ledger.reserved, 16);
            assert_eq!(f.broker.inspect().ledger.charged, 0);
            assert_eq!(f.endpoint.execution_count(), 0);
        } else {
            let receipt = f.endpoint.deliver(&result.unwrap()).unwrap();
            f.broker.accept_receipt(receipt).unwrap();
            assert_eq!(f.endpoint.execution_count(), 1);
            assert_eq!(f.endpoint.payload(), b"publish");
            assert_eq!(f.broker.inspect().ledger.charged, 16);
        }
    }
}
