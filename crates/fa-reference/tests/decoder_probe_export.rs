//! Actual fitted monitor export, existing JSON import and held-token enforcement.
#![forbid(unsafe_code)]
#[path = "support/decoder_probe_campaign.rs"]
#[allow(dead_code)]
mod fixture;

use fixture::*;
use fa_reference::action::consequence::activation::monitor::decoder::{MonitoredDecoder, MonitoredStep, MonitoringStatus};
use fa_reference::action::consequence::activation::probe::training::decoder::{CampaignWork, CaptureWork, MonitorExport};
use fa_reference::action::consequence::activation::probe::training::decoder::plan::{ProbeRunPlan, PlanError};
use fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderBudget;
use fa_reference::strict_json::{self, Json, Limits};
use fa_reference::Error;

const PLAN: &[u8] = include_bytes!("fixtures/decoder_probe_campaign.json");
fn text() -> String { std::str::from_utf8(PLAN).unwrap().to_owned() }
fn json(bytes: &[u8]) -> Json { strict_json::parse(bytes, Limits::default()).unwrap() }
fn float(value: &Json) -> f32 {
    match value { Json::Number(n) => n.lexeme().parse().unwrap(), _ => panic!("expected number") }
}

#[test]
fn configured_capture_fit_export_import_preserves_coefficients_and_numerical_release() {
    let model = model();
    let mut plan = ProbeRunPlan::from_json(PLAN).unwrap();
    let report = plan.run(&model).unwrap();
    let bytes = report.to_monitor_json(plan.settings()).unwrap();
    let root = json(&bytes);
    assert_eq!(root.get("schema").unwrap().as_str(), Some("fa.decoder-monitor/1"));
    for entry in root.get("layers").unwrap().as_array().unwrap() {
        let layer = entry.get("layer").unwrap().as_u64().unwrap();
        let serialized = &entry.get("probes").unwrap().as_array().unwrap()[0];
        let fitted = report.layers()[&layer].calibration().fitted();
        let stored: Vec<_> = serialized.get("weights").unwrap().as_array().unwrap().iter().map(|v| float(v).to_bits()).collect();
        assert_eq!(stored, fitted.weights().iter().map(|v| v.to_bits()).collect::<Vec<_>>());
        assert_eq!(float(serialized.get("bias").unwrap()).to_bits(), fitted.bias().to_bits());
        assert_eq!(float(serialized.get("threshold").unwrap()).to_bits(), report.layers()[&layer].calibration().selected_threshold().unwrap().to_bits());
    }
    let settings = plan.settings();
    let mut direct = MonitoredDecoder::new(model.clone(), 90, settings.generation(),
        report.monitors(settings).unwrap(), settings.total_budget()).unwrap();
    let mut imported = MonitoredDecoder::from_json(model.clone(), 90, &bytes).unwrap();
    let mut raw = model.session(90).unwrap();
    for position in 0..3 {
        let budget = DecoderBudget { scalar_products: model.estimate(position, 1).unwrap().scalar_products().unwrap() };
        let a = direct.advance(position as u64, 0, budget).unwrap();
        let b = imported.advance(position as u64, 0, budget).unwrap();
        assert_eq!(a.review(), b.review());
        match (a, b) {
            (MonitoredStep::Released(a), MonitoredStep::Released(b)) => {
                let expected = raw.advance(position as u64, 0, budget).unwrap();
                assert_eq!(a.step().logits.as_ref(), expected.logits.as_ref());
                assert_eq!(b.step().logits.as_ref(), expected.logits.as_ref());
            }
            other => panic!("benign control was not released: {other:?}"),
        }
    }
    let budget = DecoderBudget { scalar_products: model.estimate(3, 1).unwrap().scalar_products().unwrap() };
    let a = direct.advance(3, 1, budget).unwrap();
    let b = imported.advance(3, 1, budget).unwrap();
    assert_eq!(a.review(), b.review());
    assert!(matches!(a, MonitoredStep::Held(_)));
    assert!(matches!(b, MonitoredStep::Held(_)));
    assert_eq!(imported.status(), MonitoringStatus::Held);
    assert_eq!(imported.advance_greedy(4, budget).unwrap_err(), Error::WrongState);
}

#[test]
fn failed_evaluation_produces_report_but_no_monitor_json_or_partial_roster() {
    let modified = text().replace("\"task\":5,\"lineage\":105,\"split\":\"evaluation\",\"label\":\"benign\",\"tokens\":[0]",
        "\"task\":5,\"lineage\":105,\"split\":\"evaluation\",\"label\":\"benign\",\"tokens\":[1]");
    assert_ne!(modified, text());
    let mut plan = ProbeRunPlan::from_json(modified.as_bytes()).unwrap();
    let report = plan.run(&model()).unwrap();
    assert!(!report.accepted());
    assert_eq!(report.to_monitor_json(plan.settings()), Err(Error::WrongState));
    assert_eq!(report.monitors(plan.settings()).unwrap_err(), Error::WrongState);
    assert_eq!(report.layers().len(), 2);
    assert_eq!(report.layers()[&1].evaluation().unwrap().counts().benign_alarm, 1);
}

#[test]
fn incomplete_fit_allowance_is_rejected_before_capture_and_can_never_be_renewed_by_retry() {
    let modified = text().replace("\"scoring_bytes\":688", "\"scoring_bytes\":687");
    let mut short = ProbeRunPlan::from_json(modified.as_bytes()).unwrap();
    let c = short.remaining_capture(); let t = short.remaining_campaign();
    assert_eq!(short.run(&model()).unwrap_err(), Error::Limit);
    assert_eq!(short.remaining_capture(), c); assert_eq!(short.remaining_campaign(), t);
    let mut exact = ProbeRunPlan::from_json(PLAN).unwrap();
    assert!(exact.run(&model()).unwrap().accepted());
    assert_eq!(exact.remaining_capture(), CaptureWork::default());
    assert_eq!(exact.remaining_campaign(), CampaignWork::default());
    assert_eq!(exact.run(&model()).unwrap_err(), Error::Limit);
}

#[test]
fn unknown_duplicate_malformed_and_nonfinite_configuration_cannot_select_an_alternate_path() {
    for changed in [
        text().replacen("{", "{\"execute\":\"other-backend\",", 1),
        text().replacen("\"id\":1", "\"id\":1,\"id\":2", 1),
        text().replace("\"levels\":[0,8,23]", "\"levels\":[0,8]"),
        text().replace("\"learning_rate\":0.25", "\"learning_rate\":1e999"),
        text().replace("\"thresholds\":[-10.0,0.0,10.0]", "\"thresholds\":[10.0,0.0]"),
        text().replace("\"tokens\":[1]", "\"tokens\":[4294967296]"),
        text().replace("\"split\":\"evaluation\"", "\"split\":\"test-and-retune\""),
    ] {
        assert_ne!(changed, text());
        assert!(ProbeRunPlan::from_json(changed.as_bytes()).is_err());
    }
    assert!(matches!(ProbeRunPlan::from_json(b"{\"a\":1,\"a\":2}"), Err(PlanError::Syntax)));
    assert!(ProbeRunPlan::from_json(PLAN).is_ok());
}

#[test]
fn identity_context_roster_and_late_vocabulary_mismatches_refuse_without_numerical_work() {
    for changed in [text().replace("\"model_generation\":3", "\"model_generation\":4"),
        text().replace("\"context\":8", "\"context\":7"),
        text().replace("\"layer\":2", "\"layer\":3"),
        text().replace("\"tokens\":[1]", "\"tokens\":[2]"),
        text().replace("\"lineage\":106", "\"lineage\":101")]
    {
        let mut plan = ProbeRunPlan::from_json(changed.as_bytes()).unwrap();
        let capture = plan.remaining_capture(); let training = plan.remaining_campaign();
        assert!(plan.run(&model()).is_err());
        assert_eq!(plan.remaining_capture(), capture); assert_eq!(plan.remaining_campaign(), training);
    }
}

#[test]
fn admitted_decoder_failure_keeps_charge_and_does_not_attempt_training() {
    let mut plan = ProbeRunPlan::from_json(PLAN).unwrap();
    let training = plan.remaining_campaign();
    assert_eq!(plan.run(&model_with_output(true)).unwrap_err(), Error::Overflow);
    assert_eq!(plan.remaining_capture(), CaptureWork::default());
    assert_eq!(plan.remaining_campaign(), training);
    assert_eq!(plan.run(&model()).unwrap_err(), Error::Limit);
}

#[test]
fn export_cannot_widen_monitor_limits_or_change_model_identity_at_import() {
    let model = model();
    let mut plan = ProbeRunPlan::from_json(PLAN).unwrap();
    let report = plan.run(&model).unwrap();
    let mut no_work = plan.settings().total_budget(); no_work.encoded_bytes = 0;
    let settings = MonitorExport::new(8, vec![23], plan.settings().per_layer_budget(), no_work).unwrap();
    let bytes = report.to_monitor_json(&settings).unwrap();
    let mut guarded = MonitoredDecoder::from_json(model.clone(), 90, &bytes).unwrap();
    assert!(matches!(guarded.advance(0, 0, DecoderBudget { scalar_products: 68 }).unwrap(), MonitoredStep::Held(_)));
    let changed = String::from_utf8(bytes).unwrap().replace("\"model_generation\":3", "\"model_generation\":4");
    assert!(MonitoredDecoder::from_json(model, 90, changed.as_bytes()).is_err());
}
