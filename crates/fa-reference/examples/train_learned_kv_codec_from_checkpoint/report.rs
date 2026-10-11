//! Fixed training inputs and original completed, failed or cancelled work.
use super::{Plan, assets::Inputs};
use fa_reference::action::consequence::activation::tensor::kv::{
    decoder::RotaryScaling, experiment::KvSide,
    model::learned::replay::training::{CodecTrainingReport, CodecTrainingStatus, TrainingDocumentWork}};
use std::io::{self, Write};

pub(super) fn start(output: &mut impl Write, plan: &Plan, inputs: &Inputs) -> io::Result<()> {
    let profile = inputs.model.profile(); let id = profile.identity(); let s = profile.shape();
    writeln!(output, "{{\"kind\":\"codec_training_started\",\"schema\":\"fa.learned-kv-codec-report/1\",\"identity\":{{\"tenant\":{},\"model\":{},\"model_generation\":{},\"tokenizer_generation\":{},\"profile_generation\":{}}},\"shape\":{{\"vocabulary\":{},\"hidden\":{},\"intermediate\":{},\"layers\":{},\"query_heads\":{},\"cache_heads\":{},\"context\":{}}},\"epsilon_bits\":{},\"theta_bits\":{},\"planned_documents\":{},\"permission\":\"not_issued\"}}",
        id.tenant, id.model, id.model_generation, id.tokenizer_generation, id.profile_generation,
        s.vocabulary, s.hidden, s.intermediate, s.layers, s.query_heads, s.cache_heads, s.context,
        profile.epsilon().to_bits(), profile.theta().to_bits(), plan.documents.len())?;
    match profile.rotary_scaling() {
        RotaryScaling::None => writeln!(output, "{{\"kind\":\"rotary_profile\",\"scaling\":\"none\"}}")?,
        RotaryScaling::Linear(p) => writeln!(output, "{{\"kind\":\"rotary_profile\",\"scaling\":\"linear\",\"factor_bits\":{}}}", p.factor().to_bits())?,
        RotaryScaling::Llama3(p) => writeln!(output, "{{\"kind\":\"rotary_profile\",\"scaling\":\"llama3\",\"factor_bits\":{},\"low_freq_factor_bits\":{},\"high_freq_factor_bits\":{},\"original_max_position_embeddings\":{}}}",
            p.factor().to_bits(), p.low_freq_factor().to_bits(), p.high_freq_factor().to_bits(), p.original_max_position_embeddings())?,
    }
    let p = plan.policy; let b = plan.budget; let f = b.fitting; let e = inputs.estimate;
    writeln!(output, "{{\"kind\":\"training_policy\",\"id\":{},\"generation\":{},\"rank\":{},\"sweeps\":{},\"tokens\":{},\"source_values\":{},\"decoder_products\":{},\"fitting\":{{\"source_values\":{},\"parameter_values\":{},\"scratch_values\":{},\"work_units\":{}}}}}",
        p.id(), p.generation(), p.rank(), p.sweeps(), b.tokens, b.source_values, b.decoder_products,
        f.source_values, f.parameter_values, f.scratch_values, f.work_units)?;
    writeln!(output, "{{\"kind\":\"training_admission\",\"tokens\":{},\"source_values\":{},\"decoder_products\":{},\"groups\":{},\"parameter_values\":{},\"scratch_values\":{},\"fit_work_units\":{},\"retained_source_bytes\":{},\"binding_reservation\":{},\"archive_reservation\":{},\"report_reservation\":{}}}",
        e.tokens, e.source_values, e.decoder_products, e.fit.groups, e.fit.parameter_values,
        e.fit.scratch_values, e.fit.work_units, e.input_bytes, e.binding_reservation, e.archive_reservation, e.report_reservation)?;
    let w = inputs.work;
    writeln!(output, "{{\"kind\":\"input_work\",\"auxiliary_bytes\":{},\"weight_bytes\":{},\"weight_read_calls\":{}}}",
        w.auxiliary_bytes, w.weight_bytes, w.weight_read_calls)?;
    if w.checkpoint_source {
        writeln!(output, "{{\"kind\":\"checkpoint_source\",\"schema\":\"fa.checkpoint-weights/1\",\"base_files\":{},\"adapter_parameters\":{},\"updated_parameters\":{},\"merge_products\":{},\"permission\":\"not_issued\"}}",
            w.base_files, w.adapter_parameters, w.updated_parameters, w.merge_products)?;
    }
    for document in &plan.documents {
        writeln!(output, "{{\"kind\":\"planned_document\",\"origin\":{},\"stream\":{},\"tokens\":{:?}}}",
            document.origin, document.stream, document.tokens)?;
    }
    Ok(())
}

pub(super) fn document(output: &mut impl Write, kind: &str, origin: u64, work: &TrainingDocumentWork)
    -> io::Result<()>
{
    let w = work.accepted;
    writeln!(output, "{{\"kind\":\"{kind}\",\"origin\":{origin},\"stream\":{},\"declared_tokens\":{},\"attempted_tokens\":{},\"reserved_decoder_products\":{},\"accepted\":{{\"tokens\":{},\"matrix_products\":{},\"attention_products\":{},\"attention_exponentials\":{},\"normalization_coordinates\":{},\"rotary_pairs\":{},\"gate_coordinates\":{},\"cache_values_appended\":{}}},\"captured_values\":{},\"complete\":{}}}",
        work.stream, work.declared_tokens, work.attempted_tokens, work.reserved_decoder_products,
        w.tokens, w.matrix_products, w.attention_products, w.attention_exponentials,
        w.normalization_coordinates, w.rotary_pairs, w.gate_coordinates, w.cache_values_appended,
        work.captured_values, work.complete)
}

pub(super) fn outcome(output: &mut impl Write, report: &CodecTrainingReport) -> io::Result<()> {
    for (origin, work) in &report.documents { document(output, "document_outcome", *origin, work)?; }
    if let Some(fit) = &report.fit {
        writeln!(output, "{{\"kind\":\"fit_completed_work\",\"rows_per_group\":{},\"training_values\":{},\"source_coordinate_visits\":{},\"parameter_values\":{},\"scratch_values_reserved\":{},\"work_units_reserved\":{},\"groups\":{}}}",
            fit.rows_per_group, fit.training_values, fit.source_coordinate_visits, fit.parameter_values,
            fit.scratch_values_reserved, fit.work_units_reserved, fit.groups.len())?;
        for (key, group) in &fit.groups {
            let side = match key.side { KvSide::Key => "key", KvSide::Value => "value" };
            let diagonal: Vec<_> = group.selected_diagonal.iter().map(|value| value.to_bits()).collect();
            writeln!(output, "{{\"kind\":\"fit_group\",\"layer\":{},\"side\":\"{side}\",\"head\":{},\"rows\":{},\"channels\":{},\"rotations\":{},\"covariance_trace_bits\":{},\"remaining_off_diagonal_squared_bits\":{},\"emitted_orthogonality_max_error_bits\":{},\"selected_diagonal_bits\":{diagonal:?}}}",
                key.layer, key.head, group.rows, group.channels, group.rotations,
                group.covariance_trace.to_bits(), group.remaining_off_diagonal_squared.to_bits(),
                group.emitted_orthogonality_max_error.to_bits())?;
        }
    }
    let (status, error) = match report.status {
        CodecTrainingStatus::Capturing => ("capturing", None),
        CodecTrainingStatus::ReadyToFit => ("ready_to_fit", None),
        CodecTrainingStatus::Fitted => ("fitted", None),
        CodecTrainingStatus::Cancelled => ("cancelled", None),
        CodecTrainingStatus::Failed(error) => ("failed", Some(error)),
    };
    let error = error.map_or_else(|| "null".to_owned(), |error| quoted(&format!("{error:?}")));
    writeln!(output, "{{\"kind\":\"training_outcome\",\"status\":\"{status}\",\"declared_tokens\":{},\"declared_values\":{},\"admitted_decoder_products\":{},\"all_inference_reported\":{},\"fit_attempted\":{},\"error\":{error},\"permission\":\"not_issued\"}}",
        report.declared_tokens, report.declared_values, report.admitted_decoder_products,
        report.all_inference_reported, report.fit_attempted)
}

pub(super) fn artifacts(output: &mut impl Write, binding: usize, archive: usize, saved: bool) -> io::Result<()> {
    writeln!(output, "{{\"kind\":\"codec_artifacts\",\"binding_format\":\"fa.learned-fit-binding/1\",\"archive_format\":\"FAKVFIT/1\",\"binding_bytes\":{binding},\"archive_bytes\":{archive},\"saved\":{saved},\"permission\":\"not_issued\"}}")
}
pub(super) fn failure(output: &mut impl Write, stage: &str, error: &dyn std::error::Error) -> io::Result<()> {
    let message = error.to_string(); let clipped: String = message.chars().take(2048).collect();
    writeln!(output, "{{\"kind\":\"codec_training_failed\",\"stage\":\"{stage}\",\"error\":{},\"error_truncated\":{},\"permission\":\"not_issued\"}}",
        quoted(&clipped), clipped.len() != message.len())
}
fn quoted(text: &str) -> String {
    use std::fmt::Write as _;
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""), '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"), '\r' => out.push_str("\\r"), '\t' => out.push_str("\\t"),
            ch if ch.is_control() => { write!(out, "\\u{:04x}", ch as u32).expect("writing to string"); },
            ch => out.push(ch),
        }
    }
    out.push('"'); out
}
