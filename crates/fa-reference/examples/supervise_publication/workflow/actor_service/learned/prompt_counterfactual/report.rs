//! Render only original typed observations, never infer an outcome from a failure.
use super::{assets::Loaded, plan::Plan};
use fa_reference::{Error, action::consequence::activation::{monitor::MonitorOutcome,
    tensor::kv::decoder::{DecoderWork, experiment::prompt::{PromptArmReport, PromptComparisonBudget, PromptComparisonReport,
        PromptComparisonStatus, campaign::{OutcomeClass, PromptCampaignBudget, PromptCampaignStatus,
            TokenObservation, TrialDisposition}}, sampling::monitored::{GenerationBudget, GenerationStatus,
        GenerationStop, GenerationTelemetryBudget, GenerationTelemetryWork, GenerationWork}}}};
use std::time::Duration;

const MAX_REPORT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy)]
pub(super) struct Output {
    pub elapsed: Duration,
    pub cancelled_at: Option<Duration>,
    pub deadline_reached: bool,
    pub error: Option<Error>,
}

pub(super) fn encode(plan: &Plan, loaded: &Loaded, output: Output) -> Result<Vec<u8>, String> {
    let campaign = &loaded.campaign;
    let progress = campaign.progress();
    let experiment = campaign.plan();
    let original = &experiment.config().original;
    let sampling = original.sampling();
    let identity = plan.identity;
    let mut trials = Vec::with_capacity(campaign.trials().len());
    for trial in campaign.trials() {
        let disposition = match trial.disposition() {
            TrialDisposition::Observed => "{\"kind\":\"observed\"}".to_owned(),
            TrialDisposition::AdmissionFailed(error) => failure("admission_failed", error),
            TrialDisposition::Interrupted(error) => failure("interrupted", error),
        };
        let outcomes = match trial.outcomes() {
            Some((baseline, treated)) => format!("{{\"baseline\":{},\"treated\":{}}}",
                outcome(baseline), outcome(treated)),
            None => "null".to_owned(),
        };
        trials.push(format!(concat!("{{\"seed\":\"{}\",\"disposition\":{},",
            "\"comparison\":{},\"outcomes\":{},\"outcome_comparisons\":\"{}\"}}"),
            trial.seed(), disposition, optional_comparison(trial.comparison().copied()),
            outcomes, trial.outcome_comparisons()));
    }
    let classes = [(OutcomeClass::Present, "present"), (OutcomeClass::Absent, "absent_within_completed_horizon"),
        (OutcomeClass::Censored, "censored")];
    let mut table = Vec::with_capacity(9);
    for (baseline, left) in classes {
        for (treated, right) in classes {
            table.push(format!("{{\"baseline\":\"{left}\",\"treated\":\"{right}\",\"count\":\"{}\"}}",
                progress.outcomes.count(baseline, treated)));
        }
    }
    let status = match progress.status {
        PromptCampaignStatus::Active => "{\"kind\":\"active\"}".to_owned(),
        PromptCampaignStatus::Finished => "{\"kind\":\"finished\"}".to_owned(),
        PromptCampaignStatus::Cancelled => "{\"kind\":\"cancelled\"}".to_owned(),
        PromptCampaignStatus::Failed(error) => failure("failed", error),
    };
    let error = output.error.map_or_else(|| "null".to_owned(), |error| failure("native_error", error));
    let cancelled_at = output.cancelled_at.map_or_else(|| "null".to_owned(),
        |elapsed| format!("\"{}\"", elapsed.as_millis()));
    let tokenization = loaded.tokenization;
    // All dynamic strings are decimal numbers or compiled enum labels. Prompt
    // bytes, paths, helper material and Debug text never become JSON syntax.
    let bytes = format!(concat!(
        "{{\"schema\":\"fa.learned-prompt-counterfactual-result/1\",\"purpose\":\"experiment\",",
        "\"effect_authority\":\"none\",\"knowledge\":{{\"state\":\"known\",\"basis\":{{",
        "\"claim_class\":\"bounded_numerical_experiment\",\"authenticated_model\":false,",
        "\"population\":\"one_declared_source_lineage\",\"schedule_complete\":{}}},\"value\":{{",
        "\"campaign\":\"{}\",\"intervention\":\"{}\",\"revision\":\"{}\",\"status\":{},",
        "\"deadline_reached\":{},\"elapsed_ms\":\"{}\",\"runtime_ms\":\"{}\",\"cancelled_at_ms\":{},\"operation_error\":{},",
        "\"model\":{{\"tenant\":\"{}\",\"model\":\"{}\",\"model_generation\":\"{}\",",
        "\"tokenizer_generation\":\"{}\",\"profile_generation\":\"{}\",\"context\":\"{}\",",
        "\"stream\":\"{}\",\"evaluation_origin\":\"{}\"}},",
        "\"sampling\":{{\"policy\":\"{}\",\"generation\":\"{}\",\"vocabulary\":\"{}\",",
        "\"temperature_bits\":\"{}\",\"top_k\":\"{}\",\"top_p_bits\":\"{}\",",
        "\"stream\":\"{}\",\"template_seed\":\"{}\"}},",
        "\"original_prompt_tokens\":[{}],\"treated_prompt_tokens\":[{}],\"max_new_tokens\":\"{}\",",
        "\"stop_tokens\":[{}],\"edit\":{{\"start\":\"{}\",\"expected\":[{}],\"replacement\":[{}]}},",
        "\"seeds\":[{}],\"outcome_tokens\":[{}],\"reservation\":{},",
        "\"limits\":{{\"generation\":{},\"telemetry\":{},\"campaign\":{}}},",
        "\"input\":{{\"asset_bytes\":\"{}\",\"asset_limit\":\"{}\",\"tokenization\":{{",
        "\"input_bytes\":\"{}\",\"pair_lookups\":\"{}\",\"heap_pops\":\"{}\",\"merges\":\"{}\"}}}},",
        "\"denominator\":{{\"declared_trials\":\"{}\",\"started_trials\":\"{}\",\"recorded_trials\":\"{}\",",
        "\"unrecorded_started_trials\":\"{}\",\"unstarted_trials\":\"{}\",\"admission_failures\":\"{}\",",
        "\"interrupted_trials\":\"{}\",\"observed_pairs\":\"{}\"}},",
        "\"outcome_comparisons\":\"{}\",\"outcome_table\":[{}],\"trials\":[{}],\"active_report\":{}}}}}}}\n"),
        progress.status == PromptCampaignStatus::Finished,
        progress.campaign, experiment.id(), progress.revision, status,
        output.deadline_reached, output.elapsed.as_millis(), plan.runtime_ms, cancelled_at, error,
        identity.tenant, identity.model, identity.model_generation, identity.tokenizer_generation,
        identity.profile_generation, plan.context, plan.stream, plan.evaluation_origin,
        sampling.policy.id(), sampling.policy.generation(), sampling.policy.vocabulary(),
        sampling.policy.temperature().to_bits(), sampling.policy.top_k(), sampling.policy.top_p().to_bits(),
        sampling.stream, sampling.seed,
        ids(original.prompt()), ids(experiment.treated_spec().prompt()), original.max_new_tokens(),
        ids(&original.stop_tokens().iter().copied().collect::<Vec<_>>()),
        experiment.edit().start, ids(&experiment.edit().expected), ids(&experiment.edit().replacement),
        numbers(campaign.seeds()), ids(campaign.question()), budget(campaign.reservation()),
        generation_budget(experiment.config().generation), telemetry_budget(experiment.config().telemetry), budget(plan.budget),
        loaded.input_bytes, plan.asset_bytes, tokenization.input_bytes, tokenization.pair_lookups,
        tokenization.heap_pops, tokenization.merges,
        progress.declared_trials, progress.started_trials, progress.recorded_trials,
        progress.unrecorded_started_trials, progress.unstarted_trials, progress.admission_failures,
        progress.interrupted_trials, progress.outcomes.observed_pairs(), progress.outcome_comparisons,
        table.join(","), trials.join(","), optional_comparison(campaign.active_report())).into_bytes();
    if bytes.len() > MAX_REPORT_BYTES { return Err("bounded prompt experiment report exceeds byte limit".into()); }
    Ok(bytes)
}

fn ids(values: &[u32]) -> String { values.iter().map(|value| format!("\"{value}\"")).collect::<Vec<_>>().join(",") }
fn numbers(values: &[u64]) -> String { values.iter().map(|value| format!("\"{value}\"")).collect::<Vec<_>>().join(",") }
fn budget(value: PromptCampaignBudget) -> String {
    format!(concat!("{{\"trials\":\"{}\",\"positions\":\"{}\",\"decoder_products\":\"{}\",",
        "\"vocabulary_scores\":\"{}\",\"outcome_comparisons\":\"{}\"}}"),
        value.trials, value.positions, value.decoder_products, value.vocabulary_scores, value.outcome_comparisons)
}
fn generation_budget(value: GenerationBudget) -> String {
    format!("{{\"decoder_products\":\"{}\",\"vocabulary_scores\":\"{}\"}}", value.decoder_products, value.vocabulary_scores)
}
fn comparison_budget(value: PromptComparisonBudget) -> String {
    format!("{{\"positions\":\"{}\",\"decoder_products\":\"{}\",\"vocabulary_scores\":\"{}\"}}",
        value.positions, value.decoder_products, value.vocabulary_scores)
}
fn error_code(error: Error) -> &'static str {
    match error { Error::InvalidInput => "invalid_input", Error::Incomplete => "incomplete",
        Error::Limit => "limit", Error::Overflow => "overflow", Error::Duplicate => "duplicate",
        Error::Missing => "missing", Error::WrongState => "wrong_state", Error::Stale => "stale", Error::Binding => "binding" }
}
fn failure(kind: &'static str, error: Error) -> String {
    format!("{{\"kind\":\"{kind}\",\"error\":\"{}\"}}", error_code(error))
}
fn status(value: GenerationStatus) -> String {
    match value {
        GenerationStatus::Prefilling => "{\"kind\":\"prefilling\"}".into(),
        GenerationStatus::Generating => "{\"kind\":\"generating\"}".into(),
        GenerationStatus::Finished(GenerationStop::TokenLimit) => "{\"kind\":\"finished\",\"reason\":\"token_limit\"}".into(),
        GenerationStatus::Finished(GenerationStop::StopToken(token)) =>
            format!("{{\"kind\":\"finished\",\"reason\":\"stop_token\",\"token\":\"{token}\"}}"),
        GenerationStatus::Held(outcome) => {
            let reason = match outcome { MonitorOutcome::NoAlarm => "no_alarm", MonitorOutcome::Alarm => "alarm",
                MonitorOutcome::AtThreshold => "at_threshold", MonitorOutcome::BudgetExhausted => "budget_exhausted",
                MonitorOutcome::Unresolved => "unresolved" };
            format!("{{\"kind\":\"held\",\"reason\":\"{reason}\"}}")
        }
        GenerationStatus::Failed(error) => failure("failed", error),
    }
}
fn outcome(value: TokenObservation) -> String {
    match value {
        TokenObservation::Present { first_generated_offset } =>
            format!("{{\"kind\":\"present\",\"first_generated_offset\":\"{first_generated_offset}\"}}"),
        TokenObservation::AbsentWithinCompletedHorizon => "{\"kind\":\"absent_within_completed_horizon\"}".into(),
        TokenObservation::Censored { stopped } => format!("{{\"kind\":\"censored\",\"stopped\":{}}}", status(stopped)),
    }
}
fn optional_comparison(value: Option<PromptComparisonReport>) -> String {
    let Some(value) = value else { return "null".into(); };
    let status = match value.status { PromptComparisonStatus::Active => "{\"kind\":\"active\"}".to_owned(),
        PromptComparisonStatus::Stopped => "{\"kind\":\"stopped\"}".to_owned(),
        PromptComparisonStatus::Failed(error) => failure("failed", error) };
    format!(concat!("{{\"experiment\":\"{}\",\"revision\":\"{}\",\"status\":{},",
        "\"reservation\":{},\"baseline\":{},\"treated\":{}}}"), value.experiment, value.revision, status,
        comparison_budget(value.reservation), arm(value.baseline), arm(value.treated))
}
fn arm(value: PromptArmReport) -> String {
    format!(concat!("{{\"status\":{},\"attempted_calls\":\"{}\",\"accepted_prompt_tokens\":\"{}\",",
        "\"accepted_continuation_tokens\":\"{}\",\"numerical\":{},\"telemetry\":{},",
        "\"all_attempted_telemetry_reported\":{}}}"), status(value.status), value.attempted_calls,
        value.accepted_prompt_tokens, value.accepted_continuation_tokens, numerical(value.numerical),
        telemetry(value.telemetry), value.all_attempted_telemetry_reported)
}
fn numerical(value: GenerationWork) -> String {
    format!(concat!("{{\"admitted_tokens\":\"{}\",\"reserved_decoder_products\":\"{}\",",
        "\"sampling_attempts\":\"{}\",\"reserved_vocabulary_scores\":\"{}\",\"accepted_decoder\":{}}}"),
        value.admitted_tokens, value.reserved_decoder_products, value.sampling_attempts,
        value.reserved_vocabulary_scores, decoder(value.accepted_decoder))
}
fn decoder(value: DecoderWork) -> String {
    format!(concat!("{{\"tokens\":\"{}\",\"matrix_products\":\"{}\",\"attention_products\":\"{}\",",
        "\"attention_exponentials\":\"{}\",\"normalization_coordinates\":\"{}\",\"rotary_pairs\":\"{}\",",
        "\"gate_coordinates\":\"{}\",\"cache_values_appended\":\"{}\"}}"),
        value.tokens, value.matrix_products, value.attention_products, value.attention_exponentials,
        value.normalization_coordinates, value.rotary_pairs, value.gate_coordinates, value.cache_values_appended)
}
fn telemetry(value: GenerationTelemetryWork) -> String {
    format!(concat!("{{\"compression_source_values\":\"{}\",\"compression_encoded_bytes\":\"{}\",",
        "\"compression_work_units\":\"{}\",\"source_check_values\":\"{}\",\"source_check_encoded_bytes\":\"{}\",",
        "\"source_check_reconstruction_products\":\"{}\",\"monitor_encoded_bytes\":\"{}\",",
        "\"monitor_probe_coordinates\":\"{}\",\"monitor_reconstruction_products\":\"{}\",",
        "\"monitor_materialized_values\":\"{}\",\"monitor_refinements\":\"{}\"}}"),
        value.compression_source_values, value.compression_encoded_bytes, value.compression_work_units,
        value.source_check_values, value.source_check_encoded_bytes, value.source_check_reconstruction_products,
        value.monitor_encoded_bytes, value.monitor_probe_coordinates, value.monitor_reconstruction_products,
        value.monitor_materialized_values, value.monitor_refinements)
}
fn telemetry_budget(value: GenerationTelemetryBudget) -> String {
    format!(concat!("{{\"compression_source_values\":\"{}\",\"compression_encoded_bytes\":\"{}\",",
        "\"compression_work_units\":\"{}\",\"source_check_values\":\"{}\",\"source_check_encoded_bytes\":\"{}\",",
        "\"source_check_reconstruction_products\":\"{}\",\"monitor_encoded_bytes\":\"{}\",",
        "\"monitor_probe_coordinates\":\"{}\",\"monitor_reconstruction_products\":\"{}\",",
        "\"monitor_materialized_values\":\"{}\",\"monitor_refinements\":\"{}\"}}"),
        value.compression_source_values, value.compression_encoded_bytes, value.compression_work_units,
        value.source_check_values, value.source_check_encoded_bytes, value.source_check_reconstruction_products,
        value.monitor_encoded_bytes, value.monitor_probe_coordinates, value.monitor_reconstruction_products,
        value.monitor_materialized_values, value.monitor_refinements)
}
