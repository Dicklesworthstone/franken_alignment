//! Complete evaluated probes enter the original learned monitor and its existing
//! operator schema. JSON is provisioning data; it does not authenticate the model,
//! tokenizer, labels, split declarations, or experiment lineage.
use super::KvDecoderCampaign;
use crate::action::consequence::activation::monitor::learned::{
    LearnedMonitorBudget, LearnedRefinementMonitor,
    model::{KvTap, LearnedAuditBudget, LearnedAuditPreparationBudget, LearnedModelMonitor},
};
use crate::action::consequence::activation::tensor::kv::{
    decoder::{DecoderBudget, monitoring::{LearnedDecoderPolicy, LearnedStreamRetention}},
    experiment::KvSide, model::learned::LearnedKvCodec,
};
use crate::Error;
use std::collections::BTreeMap;
use std::fmt::{self, Write as _};

/// The existing learned publication command's auxiliary-data ceiling.
pub const MAX_KV_MONITOR_JSON_BYTES: usize = 2 * 1024 * 1024;

/// Explicit deployment settings. Training supplies the original probe identity,
/// coefficients and selected threshold; callers only supply operational caps.
#[derive(Clone, Debug)]
pub struct KvMonitorSettings {
    pub taps: BTreeMap<KvTap, LearnedMonitorBudget>,
    pub audit: LearnedAuditBudget,
    pub preparation: LearnedAuditPreparationBudget,
    pub inference: DecoderBudget,
    pub retention: LearnedStreamRetention,
}

impl KvDecoderCampaign {
    pub fn monitor(&self, settings: &KvMonitorSettings) -> Result<LearnedModelMonitor, Error> {
        if !settings.taps.keys().eq(self.taps().keys()) { return Err(Error::Binding); }
        let taps = self.probes()?.into_iter().map(|(tap, probe)| {
            Ok((tap, LearnedRefinementMonitor::new(vec![probe], settings.taps[&tap])?))
        }).collect::<Result<BTreeMap<_, _>, Error>>()?;
        LearnedModelMonitor::new(self.model.cache_profile().clone(), taps, settings.audit)
    }

    /// Bind the original codec/cache profile and refuse declared overlap with
    /// the probe population. Here codec fit source keys mean lineage IDs, while
    /// descriptor streams mean task IDs. These are declarations, not proofs of
    /// hidden-origin independence or authentication of external model bytes.
    pub fn policy(&self, codec: LearnedKvCodec, settings: &KvMonitorSettings)
        -> Result<LearnedDecoderPolicy, Error>
    {
        if codec.profile() != self.model.cache_profile() { return Err(Error::Binding); }
        for (lineage, source) in &codec.fit_report().sources {
            if self.cases.keys().any(|origin| origin.lineage == *lineage
                || source.descriptor.layers().values().any(|layer| layer.stream == origin.task)) {
                return Err(Error::Duplicate);
            }
        }
        LearnedDecoderPolicy::new(codec, self.monitor(settings)?, settings.retention.clone(),
            settings.preparation, settings.inference)
    }

    /// Export directly consumable `fa.learned-kv-monitor/1` data after the whole
    /// roster passes held-out evaluation and the original policy binds. The
    /// existing schema contains NO model/tokenizer/lineage identity fields: its
    /// consumer independently imports and binds those assets. Retain the exact
    /// model and case metadata outside this JSON when provisioning that consumer.
    /// Head-selected retention remains available through `policy`; v1 can encode
    /// only All or None. No file creation or live policy promotion occurs here.
    pub fn monitor_json(&self, codec: &LearnedKvCodec, settings: &KvMonitorSettings,
        max_bytes: usize) -> Result<Vec<u8>, Error>
    {
        if max_bytes > MAX_KV_MONITOR_JSON_BYTES { return Err(Error::Limit); }
        self.policy(codec.clone(), settings)?;
        let retention = match settings.retention {
            LearnedStreamRetention::All => "all",
            LearnedStreamRetention::None => "none",
            LearnedStreamRetention::Heads(_) => return Err(Error::InvalidInput),
        };
        let probes = self.probes()?;
        let compression = settings.preparation.compression;
        let checked = settings.preparation.source_check;
        let mut json = LimitedJson { bytes: String::new(), limit: max_bytes };
        write!(json, "{{\"schema\":\"fa.learned-kv-monitor/1\",\"retention\":\"{retention}\",\"inference_products\":{},\"preparation\":{{\"compression\":{{\"source_values\":{},\"encoded_bytes\":{},\"work_units\":{}}},\"source_check\":{{\"source_values\":{},\"encoded_bytes\":{},\"reconstruction_products\":{}}}}},\"audit\":{{\"rows\":{},\"budget\":",
            settings.inference.scalar_products, compression.source_values, compression.encoded_bytes,
            compression.work_units, checked.source_values, checked.encoded_bytes,
            checked.reconstruction_products, settings.audit.rows).map_err(|_| Error::Limit)?;
        monitoring(&mut json, settings.audit.monitoring)?;
        json.write_str("},\"taps\":[").map_err(|_| Error::Limit)?;
        for (index, (tap, probe)) in probes.iter().enumerate() {
            if index > 0 { json.write_str(",").map_err(|_| Error::Limit)?; }
            let side = match tap.side { KvSide::Key => "key", KvSide::Value => "value" };
            write!(json, "{{\"layer\":{},\"side\":\"{side}\",\"budget\":", tap.layer).map_err(|_| Error::Limit)?;
            monitoring(&mut json, settings.taps[tap])?;
            let identity = probe.identity();
            let (weights, bias, threshold) = probe.coefficient_bits();
            write!(json, ",\"probes\":[{{\"id\":{},\"generation\":{},\"weights\":[",
                identity.id, identity.generation).map_err(|_| Error::Limit)?;
            for (index, weight) in weights.iter().enumerate() {
                if index > 0 { json.write_str(",").map_err(|_| Error::Limit)?; }
                write!(json, "{}", f32::from_bits(*weight)).map_err(|_| Error::Limit)?;
            }
            write!(json, "],\"bias\":{},\"threshold\":{}}}]}}",
                f32::from_bits(bias), f32::from_bits(threshold)).map_err(|_| Error::Limit)?;
        }
        json.write_str("]}").map_err(|_| Error::Limit)?;
        let bytes = json.bytes.into_bytes();
        // Match the actual unchanged consumer's parser bounds, in addition to
        // incremental output admission. Original probe constructors ensure all
        // emitted binary32 values are finite; Display preserves their bits.
        crate::strict_json::parse(&bytes, crate::strict_json::Limits {
            max_bytes: MAX_KV_MONITOR_JSON_BYTES, max_depth: 10,
            max_items: 262_144, max_string_bytes: MAX_KV_MONITOR_JSON_BYTES,
        }).map_err(|_| Error::Limit)?;
        Ok(bytes)
    }
}

fn monitoring(json: &mut LimitedJson, budget: LearnedMonitorBudget) -> Result<(), Error> {
    write!(json, "{{\"encoded_bytes\":{},\"probe_coordinates\":{},\"reconstruction_products\":{},\"materialized_values\":{},\"refinements\":{}}}",
        budget.encoded_bytes, budget.probe_coordinates, budget.reconstruction_products,
        budget.materialized_values, budget.refinements).map_err(|_| Error::Limit)
}

struct LimitedJson { bytes: String, limit: usize }
impl fmt::Write for LimitedJson {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let length = self.bytes.len().checked_add(text.len()).ok_or(fmt::Error)?;
        if length > self.limit { return Err(fmt::Error); }
        self.bytes.try_reserve_exact(text.len()).map_err(|_| fmt::Error)?;
        self.bytes.push_str(text);
        Ok(())
    }
}
