//! An independent source inventory admits only the original verified refit.
use super::fields::{Fields, hex, MAX_DATA_BYTES};
use crate::config::debug;
use fa_reference::action::consequence::activation::tensor::kv::model::{
    ModelKvDescriptor, MAX_MODEL_DESCRIPTOR_BYTES,
    learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy,
        replay::archive::{LearnedKvFitArchive, LearnedKvFitBinding}},
};
use std::collections::BTreeMap;

pub(super) fn binding(bytes: &[u8]) -> Result<LearnedKvFitBinding, String> {
    let mut root = Fields::parse(bytes, MAX_DATA_BYTES)?;
    if root.text("schema")? != "fa.learned-fit-binding/1" { return Err("unsupported learned fitting binding".into()); }
    let mut policy = root.object("policy")?;
    let selected = LearnedKvPolicy::new(policy.number("id")?, policy.number("generation")?,
        policy.size("rank")?, policy.size("sweeps")?).map_err(debug)?;
    policy.end()?;
    let mut budget = root.object("budget")?;
    let budget_value = FitBudget { source_values: budget.size("source_values")?,
        parameter_values: budget.size("parameter_values")?, scratch_values: budget.size("scratch_values")?,
        work_units: budget.number("work_units")? };
    budget.end()?;
    let mut sources = BTreeMap::new();
    for source in root.array("sources", 64)? {
        let mut source = Fields::new(source)?;
        let origin = source.number("origin")?;
        let descriptor = ModelKvDescriptor::decode(&hex(&source.text("descriptor_hex")?,
            MAX_MODEL_DESCRIPTOR_BYTES)?).map_err(debug)?;
        source.end()?;
        if origin == 0 || sources.insert(origin, descriptor).is_some() {
            return Err("fit sources need unique nonzero original identities".into());
        }
    }
    if sources.is_empty() { return Err("fit binding needs a complete original source inventory".into()); }
    root.end()?;
    Ok(LearnedKvFitBinding { policy: selected, budget: budget_value, sources })
}

/// No coefficient/report parser supplies a codec. The ORIGINAL archive importer
/// checks complete binding/framing, then reruns and compares the ORIGINAL fit.
pub(super) fn replay(bytes: &[u8], expected: &LearnedKvFitBinding, limit: usize)
    -> Result<LearnedKvCodec, String>
{
    let archive = LearnedKvFitArchive::decode(bytes, expected, limit).map_err(debug)?;
    let (codec, _verified_checkpoint) = archive.replay(expected.budget).map_err(debug)?;
    Ok(codec)
}
