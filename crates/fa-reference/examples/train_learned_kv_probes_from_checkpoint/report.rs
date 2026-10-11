//! Complete original numerical outcomes, including failed/unevaluated taps.
use super::{Plan, assets::Inputs};
use fa_reference::action::consequence::activation::{
    monitor::learned::LearnedMonitorBudget,
    probe::{SCORE_UNIT_EXPONENT, training::{CaseLabel, DataSplit,
        calibration::{CaseScore, ConfusionCounts}, decoder::kv::{KvCampaignWork, KvCaptureWork, KvDecoderCampaign}}},
    tensor::kv::{experiment::KvSide, model::learned::LearnedKvCodec,
        decoder::monitoring::LearnedStreamRetention}};
use std::cmp::Ordering;
use std::io::{self, Write};

pub(super) fn start(output: &mut impl Write, plan: &Plan, inputs: &Inputs) -> io::Result<()> {
    let profile = inputs.model.profile(); let id = profile.identity(); let s = profile.shape();
    writeln!(output, "{{\"kind\":\"campaign_started\",\"schema\":\"fa.learned-kv-probe-report/1\",\"id\":{},\"generation\":{},\"identity\":{{\"tenant\":{},\"model\":{},\"model_generation\":{},\"tokenizer_generation\":{},\"profile_generation\":{}}},\"shape\":{{\"vocabulary\":{},\"hidden\":{},\"intermediate\":{},\"layers\":{},\"query_heads\":{},\"cache_heads\":{},\"context\":{}}},\"epsilon_bits\":{},\"theta_bits\":{},\"planned_cases\":{},\"planned_taps\":{},\"permission\":\"not_issued\"}}",
        plan.id, plan.generation, id.tenant, id.model, id.model_generation, id.tokenizer_generation, id.profile_generation,
        s.vocabulary, s.hidden, s.intermediate, s.layers, s.query_heads, s.cache_heads, s.context,
        profile.epsilon().to_bits(), profile.theta().to_bits(), plan.cases.len(), plan.taps.len())?;
    use fa_reference::action::consequence::activation::tensor::kv::decoder::RotaryScaling;
    match profile.rotary_scaling() {
        RotaryScaling::None => writeln!(output, "{{\"kind\":\"rotary_profile\",\"scaling\":\"none\"}}")?,
        RotaryScaling::Linear(p) => writeln!(output, "{{\"kind\":\"rotary_profile\",\"scaling\":\"linear\",\"factor_bits\":{}}}", p.factor().to_bits())?,
        RotaryScaling::Llama3(p) => writeln!(output, "{{\"kind\":\"rotary_profile\",\"scaling\":\"llama3\",\"factor_bits\":{},\"low_freq_factor_bits\":{},\"high_freq_factor_bits\":{},\"original_max_position_embeddings\":{}}}",
            p.factor().to_bits(), p.low_freq_factor().to_bits(), p.high_freq_factor().to_bits(), p.original_max_position_embeddings())?,
    }
    let w = inputs.work;
    writeln!(output, "{{\"kind\":\"input_work\",\"auxiliary_bytes\":{},\"weight_bytes\":{},\"weight_read_calls\":{},\"codec_bytes\":{},\"codec_read_calls\":{}}}",
        w.auxiliary_bytes, w.weight_bytes, w.weight_read_calls, w.codec_bytes, w.codec_read_calls)?;
    if w.checkpoint_source {
        writeln!(output, "{{\"kind\":\"checkpoint_source\",\"schema\":\"fa.checkpoint-weights/1\",\"base_files\":{},\"adapter_parameters\":{},\"updated_parameters\":{},\"merge_products\":{},\"permission\":\"not_issued\"}}",
            w.base_files, w.adapter_parameters, w.updated_parameters, w.merge_products)?;
    }
    capture_work(output, "capture_allowance", plan.remaining_capture())?;
    campaign_work(output, "campaign_allowance", plan.remaining_campaign())?;
    for case in &plan.cases {
        writeln!(output, "{{\"kind\":\"planned_case\",\"task\":{},\"lineage\":{},\"split\":\"{}\",\"label\":\"{}\",\"tokens\":{:?}}}",
            case.origin.task, case.origin.lineage, split(case.split), label(case.label), case.tokens)?;
    }
    let settings = &plan.settings;
    let retention = match settings.retention { LearnedStreamRetention::All => "all", LearnedStreamRetention::None => "none", LearnedStreamRetention::Heads(_) => "heads" };
    let c = settings.preparation.compression; let checked = settings.preparation.source_check;
    writeln!(output, "{{\"kind\":\"monitor_settings\",\"retention\":\"{retention}\",\"inference_products\":{},\"compression\":{{\"source_values\":{},\"encoded_bytes\":{},\"work_units\":{}}},\"source_check\":{{\"source_values\":{},\"encoded_bytes\":{},\"reconstruction_products\":{}}},\"audit_rows\":{},\"audit_budget\":{}}}",
        settings.inference.scalar_products, c.source_values, c.encoded_bytes, c.work_units,
        checked.source_values, checked.encoded_bytes, checked.reconstruction_products,
        settings.audit.rows, monitoring(settings.audit.monitoring))?;
    for (tap, policy) in &plan.taps {
        let f = &policy.fit; let c = &policy.calibration;
        let thresholds: Vec<_> = c.thresholds().map(f32::to_bits).collect();
        let calibration = c.calibration_criteria(); let evaluation = c.evaluation_criteria();
        writeln!(output, "{{\"kind\":\"tap_policy\",\"layer\":{},\"side\":\"{}\",\"fit\":{{\"id\":{},\"generation\":{},\"epochs\":{},\"learning_rate_bits\":{},\"l2_bits\":{},\"scale_floor_bits\":{}}},\"calibration_id\":{},\"calibration_generation\":{},\"threshold_bits\":{thresholds:?},\"calibration_criteria\":{{\"min_violation_alarms\":{},\"max_benign_holds\":{}}},\"evaluation_criteria\":{{\"min_violation_alarms\":{},\"max_benign_holds\":{}}},\"monitor_budget\":{}}}",
            tap.layer, side(tap.side), f.id(), f.generation(), f.epochs(), f.learning_rate().to_bits(), f.l2().to_bits(), f.scale_floor().to_bits(),
            c.id(), c.generation(), calibration.min_violation_alarms(), calibration.max_benign_holds(),
            evaluation.min_violation_alarms(), evaluation.max_benign_holds(), monitoring(settings.taps[tap]))?;
    }
    Ok(())
}

pub(super) fn codec(output: &mut impl Write, codec: &LearnedKvCodec) -> io::Result<()> {
    let policy = codec.policy(); let fit = codec.fit_report();
    writeln!(output, "{{\"kind\":\"codec_replayed\",\"id\":{},\"generation\":{},\"rank\":{},\"sweeps\":{},\"sources\":{},\"groups\":{},\"training_values\":{},\"source_coordinate_visits\":{},\"parameter_values\":{},\"scratch_values_reserved\":{},\"work_units_reserved\":{}}}",
        policy.id(), policy.generation(), policy.rank(), policy.sweeps(), fit.sources.len(), fit.groups.len(),
        fit.training_values, fit.source_coordinate_visits, fit.parameter_values, fit.scratch_values_reserved, fit.work_units_reserved)
}

pub(super) fn capture_completed(output: &mut impl Write, plan: &Plan) -> io::Result<()> {
    let actual = plan.captured_work().ok_or_else(|| super::invalid("completed campaign has no original capture work"))?;
    capture_work(output, "capture_completed_work", actual)?;
    capture_work(output, "capture_remaining", plan.remaining_capture())?;
    campaign_work(output, "campaign_remaining", plan.remaining_campaign())
}

pub(super) fn campaign(output: &mut impl Write, campaign: &KvDecoderCampaign) -> io::Result<()> {
    for (tap, result) in campaign.taps() {
        let calibration = result.calibration(); let fitted = calibration.fitted();
        let weights: Vec<_> = fitted.weights().iter().map(|v| v.to_bits()).collect();
        let means: Vec<_> = fitted.training_mean().iter().map(|v| v.to_bits()).collect();
        let scales: Vec<_> = fitted.training_scale().iter().map(|v| v.to_bits()).collect();
        writeln!(output, "{{\"kind\":\"tap_fitted\",\"layer\":{},\"side\":\"{}\",\"weights_bits\":{weights:?},\"bias_bits\":{},\"training_mean_bits\":{means:?},\"training_scale_bits\":{scales:?},\"benign\":{},\"violation\":{},\"training_visits\":{}}}",
            tap.layer, side(tap.side), fitted.bias().to_bits(), fitted.work().classes.benign,
            fitted.work().classes.violation, fitted.work().source_coordinate_visits)?;
        for score in calibration.scores() { case_score(output, tap.layer, tap.side, "calibration", score)?; }
        for trial in calibration.trials() {
            writeln!(output, "{{\"kind\":\"calibration_trial\",\"layer\":{},\"side\":\"{}\",\"threshold_bits\":{},\"accepted\":{},\"counts\":{}}}",
                tap.layer, side(tap.side), trial.threshold().to_bits(), trial.accepted(), counts(trial.counts()))?;
        }
        if let Some(evaluation) = result.evaluation() {
            for score in evaluation.scores() { case_score(output, tap.layer, tap.side, "evaluation", score)?; }
        }
        let selected = calibration.selected_threshold().map_or_else(|| "null".to_owned(), |v| v.to_bits().to_string());
        let classes = fitted.corpus().counts(DataSplit::Evaluation);
        let observed = result.evaluation().map_or_else(|| "null".to_owned(), |value| counts(value.counts()));
        writeln!(output, "{{\"kind\":\"evaluation\",\"layer\":{},\"side\":\"{}\",\"selected_threshold_bits\":{selected},\"planned_benign\":{},\"planned_violation\":{},\"evaluated\":{},\"accepted\":{},\"counts\":{observed}}}",
            tap.layer, side(tap.side), classes.benign, classes.violation, result.evaluation().is_some(), result.accepted())?;
    }
    campaign_work(output, "campaign_admitted_work", campaign.admitted_work())?;
    campaign_work(output, "campaign_completed_work", campaign.completed_work())?;
    writeln!(output, "{{\"kind\":\"campaign_complete\",\"taps\":{},\"accepted\":{},\"permission\":\"not_issued\"}}",
        campaign.taps().len(), campaign.accepted())
}

fn case_score(output: &mut impl Write, layer: u64, tap_side: KvSide, split: &str, score: &CaseScore) -> io::Result<()> {
    let frame = score.frame(); let p = frame.profile; let origin = score.origin();
    let negative = score.score().sign() == Ordering::Less;
    writeln!(output, "{{\"kind\":\"case_score\",\"layer\":{layer},\"side\":\"{}\",\"split\":\"{split}\",\"task\":{},\"lineage\":{},\"label\":\"{}\",\"frame\":{{\"tenant\":{},\"model\":{},\"model_generation\":{},\"tap\":{},\"layout_generation\":{},\"stream\":{},\"sequence\":{},\"position\":{}}},\"score\":{{\"negative\":{negative},\"unit_exponent\":{SCORE_UNIT_EXPONENT},\"little_endian_magnitude_words\":{:?}}}}}",
        side(tap_side), origin.task, origin.lineage, label(score.label()), p.tenant, p.model, p.model_generation,
        p.tap, p.layout_generation, frame.stream, frame.sequence, frame.position, score.score().magnitude_words())
}

pub(super) fn failure(output: &mut impl Write, stage: &str, error: &dyn std::error::Error, plan: &Plan) -> io::Result<()> {
    capture_work(output, "capture_remaining_after_failure", plan.remaining_capture())?;
    campaign_work(output, "campaign_remaining_after_failure", plan.remaining_campaign())?;
    let message = error.to_string();
    let clipped: String = message.chars().take(2048).collect();
    writeln!(output, "{{\"kind\":\"campaign_failed\",\"stage\":\"{stage}\",\"training_started\":{},\"planned_cases\":{},\"planned_taps\":{},\"error\":{},\"error_truncated\":{},\"permission\":\"not_issued\"}}",
        plan.started(), plan.cases.len(), plan.taps.len(), quoted(&clipped), clipped.len() != message.len())
}
fn capture_work(output: &mut impl Write, kind: &str, w: KvCaptureWork) -> io::Result<()> {
    writeln!(output, "{{\"kind\":\"{kind}\",\"cases\":{},\"original_tokens\":{},\"kv_coordinates\":{},\"scalar_products\":{}}}",
        w.cases, w.original_tokens, w.kv_coordinates, w.scalar_products)
}
fn campaign_work(output: &mut impl Write, kind: &str, w: KvCampaignWork) -> io::Result<()> {
    writeln!(output, "{{\"kind\":\"{kind}\",\"training_visits\":{},\"scoring_bytes\":{},\"scoring_coordinates\":{},\"threshold_comparisons\":{}}}",
        w.training_visits, w.scoring_bytes, w.scoring_coordinates, w.threshold_comparisons)
}
fn counts(c: ConfusionCounts) -> String {
    format!("{{\"benign_alarm\":{},\"benign_quiet\":{},\"benign_boundary\":{},\"violation_alarm\":{},\"violation_quiet\":{},\"violation_boundary\":{}}}",
        c.benign_alarm, c.benign_quiet, c.benign_boundary, c.violation_alarm, c.violation_quiet, c.violation_boundary)
}
fn monitoring(b: LearnedMonitorBudget) -> String {
    format!("{{\"encoded_bytes\":{},\"probe_coordinates\":{},\"reconstruction_products\":{},\"materialized_values\":{},\"refinements\":{}}}",
        b.encoded_bytes, b.probe_coordinates, b.reconstruction_products, b.materialized_values, b.refinements)
}
fn side(side: KvSide) -> &'static str { match side { KvSide::Key => "key", KvSide::Value => "value" } }
fn split(split: DataSplit) -> &'static str { match split { DataSplit::Training => "training", DataSplit::Calibration => "calibration", DataSplit::Evaluation => "evaluation" } }
fn label(label: CaseLabel) -> &'static str { match label { CaseLabel::Benign => "benign", CaseLabel::Violation => "violation" } }
fn quoted(text: &str) -> String {
    use std::fmt::Write as _;
    let mut value = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => value.push_str("\\\""), '\\' => value.push_str("\\\\"),
            ch if ch <= '\u{1f}' => { let _ = write!(value, "\\u{:04x}", u32::from(ch)); }
            ch => value.push(ch),
        }
    }
    value.push('"'); value
}
