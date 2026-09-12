//! Strict configuration controls are paired with executable numerical controls.
#[path = "support/decoder_fixture.rs"]
mod fixture;
use fa_reference::action::consequence::activation::monitor::decoder::*;
use fa_reference::action::consequence::activation::monitor::decoder::config::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, DecoderModel, MAX_DECODER_PRODUCTS};
use fa_reference::action::consequence::activation::monitor::MonitorOutcome;
use fa_reference::Error;

const BASE: &str = include_str!("fixtures/decoder_monitor_quiet.json");
fn parse(bytes: &[u8]) -> Result<MonitoredDecoder, MonitorConfigError> {
    MonitoredDecoder::from_json(fixture::model(fixture::profile(16)), 7, bytes)
}
fn changed(from: &str, to: &str) -> String {
    assert!(BASE.contains(from), "missing fixture anchor {from}"); BASE.replacen(from, to, 1)
}
fn control() {
    let mut session = parse(BASE.as_bytes()).unwrap();
    let step = session.advance(0, 1, DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap();
    assert!(matches!(&step, MonitoredStep::Released(_)));
    assert_eq!(step.review().generation(), 11); assert_eq!(step.review().unreviewed_layers(), 0);
}

#[test]
fn all_explicit_model_identity_epochs_bind_before_any_numerical_session() {
    control();
    for (from, to) in [("\"tenant\":1", "\"tenant\":2"), ("\"model\":2", "\"model\":3"),
        ("\"model_generation\":3", "\"model_generation\":4"),
        ("\"tokenizer_generation\":4", "\"tokenizer_generation\":5"),
        ("\"profile_generation\":5", "\"profile_generation\":6")]
    {
        assert!(matches!(parse(changed(from, to).as_bytes()), Err(MonitorConfigError::ModelIdentity)));
    }
}

#[test]
fn unknown_capability_fields_duplicate_keys_and_numeric_id_coercions_refuse() {
    control();
    for (from, to) in [
        ("\"schema\":", "\"permit\":true,\"schema\":"),
        ("\"schema\":", "\"schema\":\"fa.decoder-monitor/1\",\"schema\":"),
        ("\"generation\":11", "\"generation\":11.0"),
        ("\"id\":1", "\"id\":\"1\""),
        ("\"bias\":0.0", "\"bias\":0.0,\"command\":\"ignored?\""),
        ("\"threshold\":1000000.0", "\"threshold\":1e999"),
        ("\"threshold\":1000000.0", "\"threshold\":\"1000000\""),
    ] { assert!(parse(changed(from, to).as_bytes()).is_err(), "{to}"); }
}

#[test]
fn full_residual_roster_probe_widths_and_exact_escape_rungs_are_required() {
    control();
    assert!(matches!(parse(changed("\"layer\":2", "\"layer\":1").as_bytes()), Err(MonitorConfigError::Monitor(Error::Duplicate))));
    for (from, to) in [
        ("\"layer\":2", "\"layer\":3"),
        ("\"weights\":[1.0,0.0,0.0,0.0]", "\"weights\":[1.0,0.0,0.0]"),
        ("\"levels\":[23]", "\"levels\":[0,8]"),
        ("\"levels\":[23]", "\"levels\":[23,23]"),
        ("\"levels\":[23]", "\"levels\":[24]"),
    ] { assert!(parse(changed(from, to).as_bytes()).is_err(), "{to}"); }
    let progressed = changed("\"levels\":[23]", "\"levels\":[0,8,23]");
    let mut session = parse(progressed.as_bytes()).unwrap();
    assert!(matches!(session.advance(0, 1, DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap(), MonitoredStep::Released(_)));
}

#[test]
fn strict_truncation_and_size_limits_do_not_fall_back_to_empty_monitoring() {
    let bytes = BASE.trim_end().as_bytes();
    for length in 0..bytes.len() { assert!(parse(&bytes[..length]).is_err(), "prefix {length}"); }
    control();
    assert!(matches!(parse(&vec![b' '; MAX_MONITOR_CONFIG_BYTES + 1]), Err(MonitorConfigError::Limit)));
    assert!(parse(b"{}").is_err());
}

#[test]
fn configured_global_budget_is_shared_rather_than_renewed_per_layer() {
    let mut session = parse(changed("\"encoded_bytes\":1000000", "\"encoded_bytes\":94").as_bytes()).unwrap();
    let result = session.advance(0, 0, DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap();
    assert!(matches!(&result, MonitoredStep::Held(_)));
    assert_eq!(result.review().outcome(), MonitorOutcome::BudgetExhausted);
    assert_eq!(result.review().layers()[0].report.outcome(), MonitorOutcome::NoAlarm);
    assert_eq!(session.monitoring_work().encoded_bytes, 94);
    control();
}

#[test]
fn configured_imported_weights_match_original_numerics_with_complete_reviews() {
    let (model, _) = DecoderModel::from_safetensors(fixture::profile(16), include_bytes!("fixtures/decoder_mixed.safetensors")).unwrap();
    let mut raw = model.session(7).unwrap();
    let mut session = MonitoredDecoder::from_json(model, 7, BASE.as_bytes()).unwrap();
    for token in [1, 2, 0, 4] {
        let expected = raw.advance(raw.position(), token, DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap();
        let result = session.advance(session.position(), token, DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap();
        match result {
            MonitoredStep::Released(actual) => {
                assert!(actual.step().logits.iter().zip(expected.logits.iter()).all(|(a,b)| a.to_bits() == b.to_bits()));
                assert_eq!(actual.review().unreviewed_layers(), 0);
            }
            MonitoredStep::Held(report) => panic!("unexpected hold {report:?}"),
        }
    }
}
