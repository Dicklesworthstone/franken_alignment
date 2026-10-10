//! Explicit complete K/V probe registration for the ORIGINAL learned monitor.
//! These coefficients are operator data, not claimed training or qualification.
use super::fields::{Fields, MAX_DATA_BYTES};
use crate::config::debug;
use fa_reference::action::consequence::activation::monitor::learned::{
    LearnedMonitorBudget, LearnedRefinementMonitor,
    model::{KvTap, LearnedAuditBudget, LearnedAuditPreparationBudget, LearnedModelMonitor},
};
use fa_reference::action::consequence::activation::probe::{LinearProbe, learned::CheckedKvBudget};
use fa_reference::action::consequence::activation::tensor::kv::{
    decoder::{DecoderBudget, DecoderModel, monitoring::{LearnedDecoderPolicy, LearnedStreamRetention}},
    experiment::KvSide, model::learned::{CompressionBudget, LearnedKvCodec},
};
use std::collections::BTreeMap;

struct Probe { id: u64, generation: u64, weights: Vec<f32>, bias: f32, threshold: f32 }
struct Tap { budget: LearnedMonitorBudget, probes: Vec<Probe> }
pub(super) struct MonitorInput {
    taps: BTreeMap<KvTap, Tap>,
    budget: LearnedAuditBudget,
    preparation: LearnedAuditPreparationBudget,
    inference: DecoderBudget,
    retention: LearnedStreamRetention,
}
pub(super) struct BoundMonitor {
    pub policy: LearnedDecoderPolicy,
    pub probes: BTreeMap<KvTap, Vec<LinearProbe>>,
}
impl MonitorInput {
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut root = Fields::parse(bytes, MAX_DATA_BYTES)?;
        if root.text("schema")? != "fa.learned-kv-monitor/1" {
            return Err("unsupported learned K/V monitor schema".into());
        }
        let retention = match root.text("retention")?.as_str() {
            "all" => LearnedStreamRetention::All,
            "none" => LearnedStreamRetention::None,
            _ => return Err("unsupported learned residual retention".into()),
        };
        let inference = DecoderBudget { scalar_products: root.number("inference_products")? };
        let mut preparation = root.object("preparation")?;
        let mut compression = preparation.object("compression")?;
        let compression_value = CompressionBudget { source_values: compression.size("source_values")?,
            encoded_bytes: compression.size("encoded_bytes")?, work_units: compression.number("work_units")? };
        compression.end()?;
        let mut source = preparation.object("source_check")?;
        let source_value = CheckedKvBudget { source_values: source.size("source_values")?,
            encoded_bytes: source.size("encoded_bytes")?, reconstruction_products: source.number("reconstruction_products")? };
        source.end()?; preparation.end()?;
        let mut audit = root.object("audit")?;
        let rows = audit.size("rows")?;
        let budget = LearnedAuditBudget { rows, monitoring: monitoring(audit.object("budget")?)? };
        audit.end()?;
        let mut taps = BTreeMap::new();
        for tap in root.array("taps", 4096)? {
            let mut tap = Fields::new(tap)?;
            let layer = tap.number("layer")?;
            let side = match tap.text("side")?.as_str() {
                "key" => KvSide::Key, "value" => KvSide::Value,
                _ => return Err("K/V tap side must be key or value".into()),
            };
            let budget = monitoring(tap.object("budget")?)?;
            let mut probes = Vec::new();
            for probe in tap.array("probes", 4096)? {
                let mut probe = Fields::new(probe)?;
                let id = probe.number("id")?;
                let generation = probe.number("generation")?;
                let weights = probe.scalars("weights", 1_048_576)?;
                let bias = probe.scalar("bias")?;
                let threshold = probe.scalar("threshold")?;
                probe.end()?;
                if id == 0 || generation == 0 || weights.is_empty() {
                    return Err("learned probes require explicit nonzero identities and coefficients".into());
                }
                probes.push(Probe { id, generation, weights, bias, threshold });
            }
            tap.end()?;
            if probes.is_empty() || taps.insert(KvTap { layer, side }, Tap { budget, probes }).is_some() {
                return Err("every K/V tap needs one unique complete probe roster".into());
            }
        }
        root.end()?;
        if taps.is_empty() { return Err("learned monitoring requires a complete K/V tap inventory".into()); }
        Ok(Self { taps, budget, preparation: LearnedAuditPreparationBudget {
            compression: compression_value, source_check: source_value }, inference, retention })
    }

    /// Every layer and BOTH K/V sides bind to the imported ORIGINAL model.
    /// No empty, residual-stream, missing-layer or inferred quiet roster enters.
    pub(super) fn bind(self, model: &DecoderModel, codec: LearnedKvCodec) -> Result<BoundMonitor, String> {
        if codec.profile() != model.cache_profile() { return Err("fit archive does not bind this model's K/V profile".into()); }
        let mut taps = BTreeMap::new();
        let mut registered = BTreeMap::new();
        for (tap, selected) in self.taps {
            let layer = model.cache_profile().layers().get(&tap.layer).ok_or("unknown learned monitor layer")?;
            let contract = match tap.side { KvSide::Key => layer.keys(), KvSide::Value => layer.values() };
            let probes = selected.probes.into_iter().map(|probe|
                LinearProbe::new(probe.id, probe.generation, contract.profile(),
                    &probe.weights, probe.bias, probe.threshold).map_err(debug))
                .collect::<Result<Vec<_>, _>>()?;
            let monitor = LearnedRefinementMonitor::new(probes.clone(), selected.budget).map_err(debug)?;
            registered.insert(tap, probes);
            taps.insert(tap, monitor);
        }
        let monitor = LearnedModelMonitor::new(model.cache_profile().clone(), taps, self.budget).map_err(debug)?;
        let policy = LearnedDecoderPolicy::new(codec, monitor, self.retention, self.preparation, self.inference).map_err(debug)?;
        Ok(BoundMonitor { policy, probes: registered })
    }
}

fn monitoring(mut fields: Fields) -> Result<LearnedMonitorBudget, String> {
    let budget = LearnedMonitorBudget { encoded_bytes: fields.size("encoded_bytes")?,
        probe_coordinates: fields.size("probe_coordinates")?,
        reconstruction_products: fields.number("reconstruction_products")?,
        materialized_values: fields.size("materialized_values")?, refinements: fields.size("refinements")? };
    fields.end()?; Ok(budget)
}
