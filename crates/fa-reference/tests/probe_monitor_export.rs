#[path = "support/decoder_probe_campaign.rs"]
#[allow(dead_code)]
mod support;
use support::*;
use fa_reference::action::consequence::activation::probe::training::decoder::*;
use fa_reference::action::consequence::activation::probe::training::interchange::*;
use fa_reference::action::consequence::activation::probe::training::calibration::*;
use fa_reference::action::consequence::activation::monitor::{RefinementBudget, RefinementMonitor};
use fa_reference::action::consequence::activation::monitor::decoder::{MonitoredDecoder, MonitoredStep};
use fa_reference::action::consequence::activation::monitor::decoder::config::MAX_MONITOR_CONFIG_BYTES;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, MAX_DECODER_PRODUCTS};
use fa_reference::strict_json::{self, Limits};
use fa_reference::Error;
use std::fs;

fn settings() -> MonitorExportSettings {
    let budget = RefinementBudget { encoded_bytes: 100_000, probe_coordinates: 100_000 };
    MonitorExportSettings { generation: 99, budget, layers: (1..=2).map(|layer| (layer,
        LayerMonitorSettings { levels: vec![0, 8, 23], budget })).collect() }
}
fn report() -> DecoderCampaign { campaign(&capture(&model(), &cases())) }
fn compute() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }

#[test]
fn exported_roster_runs_through_original_config_parser_with_identical_decisions() {
    let report = report(); let config = settings();
    let bytes = report.monitor_json(&config, MAX_MONITOR_CONFIG_BYTES).unwrap();
    let mut imported = MonitoredDecoder::from_json(model(), 8, &bytes).unwrap();
    let monitors = report.probes().unwrap().into_iter().map(|(id, probe)| {
        let setting = &config.layers[&id];
        (id, RefinementMonitor::new(vec![probe], setting.levels.clone(), setting.budget).unwrap())
    }).collect();
    let mut direct = MonitoredDecoder::new(model(), 8, config.generation, monitors, config.budget).unwrap();
    for (position, token) in [0, 0, 1].into_iter().enumerate() {
        let a = imported.advance(position as u64, token, compute()).unwrap();
        let b = direct.advance(position as u64, token, compute()).unwrap();
        assert_eq!(a.review(), b.review());
        assert_eq!(imported.monitoring_work(), direct.monitoring_work());
        if token == 0 { assert!(matches!(a, MonitoredStep::Released(_))); }
        else { assert!(matches!(a, MonitoredStep::Held(_))); }
    }
    assert!(imported.advance(3, 0, compute()).is_err());
}

#[test]
fn decimal_export_preserves_all_fitted_coefficient_threshold_and_identity_bits() {
    let report = report(); let bytes = report.monitor_json(&settings(), MAX_MONITOR_CONFIG_BYTES).unwrap();
    let json = strict_json::parse(&bytes, Limits::default()).unwrap();
    let layers = json.get("layers").unwrap().as_array().unwrap();
    for (layer, expected) in report.layers() {
        let item = &layers[*layer as usize - 1];
        let probe = &item.get("probes").unwrap().as_array().unwrap()[0];
        let fitted = expected.calibration().fitted();
        assert_eq!(probe.get("id").unwrap().as_u64(), Some(fitted.policy().id()));
        assert_eq!(probe.get("generation").unwrap().as_u64(), Some(fitted.policy().generation()));
        let scalar = |value: &strict_json::Json| match value {
            strict_json::Json::Number(n) => n.lexeme().parse::<f32>().unwrap().to_bits(), _ => panic!("not a number")
        };
        let words: Vec<_> = probe.get("weights").unwrap().as_array().unwrap().iter().map(scalar).collect();
        assert_eq!(words, fitted.weights().iter().map(|v| v.to_bits()).collect::<Vec<_>>());
        assert_eq!(scalar(probe.get("bias").unwrap()), fitted.bias().to_bits());
        assert_eq!(scalar(probe.get("threshold").unwrap()), expected.calibration().selected_threshold().unwrap().to_bits());
    }
    let text = std::str::from_utf8(&bytes).unwrap();
    for forbidden in ["tokens", "lineage", "seed", "label", "accepted"] { assert!(!text.contains(forbidden)); }
}

#[test]
fn one_failed_required_layer_prevents_any_configuration_export() {
    let corpus = capture(&model(), &cases()); let mut plans = policies();
    let old = &plans[&2];
    plans.insert(2, LayerPolicy { fit: old.fit.clone(), calibration: CalibrationPolicy::new(2, 1,
        &[-10.0, 0.0, 10.0], ScreeningCriteria::new(1, 0).unwrap(), ScreeningCriteria::new(2, 0).unwrap()).unwrap() });
    let mut budget = CampaignBudget::new(corpus.estimate_campaign(&plans).unwrap()).unwrap();
    let report = corpus.run(plans, &mut budget).unwrap();
    assert!(report.layers()[&1].accepted()); assert!(!report.layers()[&2].accepted());
    assert_eq!(report.monitor_json(&settings(), MAX_MONITOR_CONFIG_BYTES), Err(Error::WrongState));
    assert_eq!(report.layers()[&2].evaluation().unwrap().counts().classes().total(), 2);
}

#[test]
fn byte_ceiling_roster_and_invalid_refinement_ladder_refuse_without_retuning() {
    let report = report(); let exact = report.monitor_json(&settings(), MAX_MONITOR_CONFIG_BYTES).unwrap();
    assert_eq!(report.monitor_json(&settings(), exact.len()).unwrap(), exact);
    assert_eq!(report.monitor_json(&settings(), exact.len() - 1), Err(Error::Limit));
    assert_eq!(report.monitor_json(&settings(), MAX_MONITOR_CONFIG_BYTES + 1), Err(Error::Limit));
    let mut missing = settings(); missing.layers.remove(&2);
    assert_eq!(report.monitor_json(&missing, MAX_MONITOR_CONFIG_BYTES), Err(Error::Binding));
    let mut invalid = settings(); invalid.layers.get_mut(&2).unwrap().levels = vec![23, 8];
    assert!(report.monitor_json(&invalid, MAX_MONITOR_CONFIG_BYTES).is_err());
    assert!(report.accepted());
    assert_eq!(report.monitor_json(&settings(), exact.len()).unwrap(), exact);
}

#[test]
fn exported_global_allowance_does_not_reset_at_a_new_layer_or_token() {
    let mut options = settings(); options.budget = RefinementBudget { encoded_bytes: 0, probe_coordinates: 0 };
    let bytes = report().monitor_json(&options, MAX_MONITOR_CONFIG_BYTES).unwrap();
    let mut session = MonitoredDecoder::from_json(model(), 8, &bytes).unwrap();
    assert!(matches!(session.advance(0, 0, compute()).unwrap(), MonitoredStep::Held(_)));
    assert_eq!(session.monitoring_work().encoded_bytes, 0);
    assert!(session.advance(1, 0, compute()).is_err());
}

#[test]
fn file_export_is_no_overwrite_and_failed_validation_creates_nothing() {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("fa-probe-export-{}-{stamp}", std::process::id()));
    fs::create_dir(&dir).unwrap(); let path = dir.join("monitor.json");
    let report = report();
    assert_eq!(report.save_monitor_new(&settings(), 1, &path).unwrap_err(), MonitorExportError::Refused(Error::Limit));
    assert!(!path.exists());
    let count = report.save_monitor_new(&settings(), MAX_MONITOR_CONFIG_BYTES, &path).unwrap();
    let bytes = fs::read(&path).unwrap(); assert_eq!(count, bytes.len());
    assert_eq!(report.save_monitor_new(&settings(), MAX_MONITOR_CONFIG_BYTES, &path).unwrap_err(),
        MonitorExportError::Io { stage: ExportStage::Create, kind: std::io::ErrorKind::AlreadyExists });
    assert_eq!(fs::read(&path).unwrap(), bytes);
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o077, 0);
    }
    let mut session = MonitoredDecoder::from_json(model(), 8, &bytes).unwrap();
    fs::remove_dir_all(dir).unwrap();
    assert!(matches!(session.advance(0, 0, compute()).unwrap(), MonitoredStep::Released(_)));
}
