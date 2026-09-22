//! V1 complete immutable recipe comparison, not a digest or parameter importer.
use super::{Recipe, wire::Writer};
use crate::action::consequence::activation::{CaptureProfile, monitor::learned::LearnedMonitorBudget};
use crate::action::consequence::activation::tensor::kv::{
    decoder::monitoring::LearnedStreamRetention, experiment::KvSide, model::learned::GroupKey,
};
use crate::Error;

pub(super) fn write(w: &mut Writer<'_>, recipe: &Recipe) -> Result<(), Error> {
    w.bytes(b"FALGRCP\x01")?;
    let model = &recipe.model.data;
    let p = &model.profile; let id = p.identity(); let s = p.shape();
    for value in [id.tenant, id.model, id.model_generation, id.tokenizer_generation,
        id.profile_generation, p.epsilon().to_bits(), p.theta().to_bits()] { w.u64(value)?; }
    for count in [s.vocabulary, s.hidden, s.intermediate, s.layers, s.query_heads,
        s.cache_heads, s.context, p.parameter_count()] { w.size(count)?; }
    // Original parameters in native execution order, including unused weights
    // and signed-zero bits. Equal model IDs or equal prefix logits do not suffice.
    w.floats(&model.embeddings)?;
    w.size(model.layers.len())?;
    for layer in &model.layers { for tensor in layer.weights.tensors() { w.floats(tensor)?; } }
    w.floats(&model.final_norm)?; w.floats(&model.output)?;
    w.u64(recipe.stream)?; w.u64(recipe.evaluation_origin)?;
    w.words(recipe.spec.prompt())?; w.size(recipe.spec.max_new_tokens())?;
    w.size(recipe.spec.stop_tokens().len())?;
    for token in recipe.spec.stop_tokens() { w.u32(*token)?; }
    let start = recipe.spec.sampling(); let sampling = &start.policy;
    for value in [sampling.id(), sampling.generation(), sampling.temperature().to_bits(),
        sampling.top_p().to_bits(), start.stream, start.seed] { w.u64(value)?; }
    w.size(sampling.vocabulary())?; w.size(sampling.top_k())?;
    w.u64(recipe.budget.decoder_products)?; w.u64(recipe.budget.vocabulary_scores)?;
    let t = recipe.telemetry;
    for value in [t.compression_source_values, t.compression_encoded_bytes, t.compression_work_units,
        t.source_check_values, t.source_check_encoded_bytes, t.source_check_reconstruction_products,
        t.monitor_encoded_bytes, t.monitor_probe_coordinates, t.monitor_reconstruction_products,
        t.monitor_materialized_values, t.monitor_refinements] { w.u64(value)?; }
    let policy = &recipe.policy; let prep = policy.preparation();
    w.u64(policy.inference().scalar_products)?;
    w.size(prep.compression.source_values)?; w.size(prep.compression.encoded_bytes)?;
    w.u64(prep.compression.work_units)?;
    w.size(prep.source_check.source_values)?; w.size(prep.source_check.encoded_bytes)?;
    w.u64(prep.source_check.reconstruction_products)?;
    match policy.retention_policy() {
        LearnedStreamRetention::None => w.u64(0)?,
        LearnedStreamRetention::All => w.u64(1)?,
        LearnedStreamRetention::Heads(heads) => {
            w.u64(2)?; w.size(heads.len())?;
            for head in heads { group(w, *head)?; }
        }
    }
    let codec = policy.codec(); let c = codec.policy();
    w.u64(c.id())?; w.u64(c.generation())?; w.size(c.rank())?; w.size(c.sweeps())?;
    w.size(codec.groups().len())?;
    for (key, basis) in codec.groups() {
        group(w, *key)?; w.floats(basis.mean())?; w.floats(basis.axes())?;
    }
    // Complete retained training descriptors also bind codec profiles and the
    // original held-out exclusion, not merely an unauthenticated codec label.
    let fit = codec.fit_report();
    w.size(fit.sources.len())?;
    for (origin, source) in &fit.sources { w.u64(*origin)?; w.blob(&source.descriptor.encode()?)?; }
    for count in [fit.rows_per_group, fit.training_values, fit.source_coordinate_visits,
        fit.parameter_values, fit.scratch_values_reserved] { w.size(count)?; }
    w.u64(fit.work_units_reserved)?; w.size(fit.groups.len())?;
    for (key, report) in &fit.groups {
        group(w, *key)?;
        for count in [report.rows, report.channels, report.rotations] { w.size(count)?; }
        for value in [report.covariance_trace, report.remaining_off_diagonal_squared,
            report.emitted_orthogonality_max_error] { w.u64(value.to_bits())?; }
        w.size(report.selected_diagonal.len())?;
        for value in &report.selected_diagonal { w.u64(value.to_bits())?; }
    }
    let monitor = policy.monitor();
    w.size(monitor.budget().rows)?; budget(w, monitor.budget().monitoring)?;
    w.size(monitor.taps().len())?;
    for (tap, row) in monitor.taps() {
        w.u64(tap.layer)?; side(w, tap.side)?; budget(w, row.budget())?;
        w.size(row.registered_probes().len())?;
        for probe in row.registered_probes() {
            let id = probe.identity(); w.u64(id.id)?; w.u64(id.generation)?;
            capture(w, id.profile)?; w.size(id.dimensions)?;
            let (weights, bias, threshold) = probe.coefficient_bits();
            w.words(weights)?; w.u32(bias)?; w.u32(threshold)?;
        }
    }
    Ok(())
}
fn side(w: &mut Writer<'_>, side: KvSide) -> Result<(), Error> {
    w.u64(match side { KvSide::Key => 0, KvSide::Value => 1 })
}
fn group(w: &mut Writer<'_>, key: GroupKey) -> Result<(), Error> {
    w.u64(key.layer)?; side(w, key.side)?; w.size(key.head)
}
fn capture(w: &mut Writer<'_>, p: CaptureProfile) -> Result<(), Error> {
    for value in [p.tenant, p.model, p.model_generation, p.tap, p.layout_generation] { w.u64(value)?; }
    Ok(())
}
fn budget(w: &mut Writer<'_>, b: LearnedMonitorBudget) -> Result<(), Error> {
    w.size(b.encoded_bytes)?; w.size(b.probe_coordinates)?; w.u64(b.reconstruction_products)?;
    w.size(b.materialized_values)?; w.size(b.refinements)
}
