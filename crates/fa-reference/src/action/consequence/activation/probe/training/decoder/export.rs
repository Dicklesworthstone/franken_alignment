//! Uniform plan settings over the existing evaluated-monitor interchange.

use super::DecoderCampaign;
use super::super::interchange::{LayerMonitorSettings, MonitorExportSettings};
use crate::action::consequence::activation::monitor::{RefinementBudget, RefinementMonitor, MAX_LEVELS};
use crate::action::consequence::activation::monitor::decoder::config::MAX_MONITOR_CONFIG_BYTES;
use crate::Error;
use std::collections::BTreeMap;

/// Explicit deployment-data settings, not automatic policy promotion. This plan
/// applies one ladder/local budget to each layer. The underlying interchange
/// also supports separately configured layer settings. No serializer is replaced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonitorExport {
    generation: u64,
    levels: Vec<u8>,
    per_layer: RefinementBudget,
    total: RefinementBudget,
}
impl MonitorExport {
    pub fn new(generation: u64, levels: Vec<u8>, per_layer: RefinementBudget,
        total: RefinementBudget) -> Result<Self, Error>
    {
        if generation == 0 || levels.is_empty() || levels.last() != Some(&23)
            || levels.iter().any(|v| *v > 23) || levels.windows(2).any(|p| p[0] >= p[1]) {
            return Err(Error::InvalidInput);
        }
        if levels.len() > MAX_LEVELS { return Err(Error::Limit); }
        Ok(Self { generation, levels, per_layer, total })
    }
    pub fn generation(&self) -> u64 { self.generation }
    pub fn levels(&self) -> &[u8] { &self.levels }
    pub fn per_layer_budget(&self) -> RefinementBudget { self.per_layer }
    pub fn total_budget(&self) -> RefinementBudget { self.total }
    pub fn for_campaign(&self, campaign: &DecoderCampaign) -> MonitorExportSettings {
        MonitorExportSettings { generation: self.generation, budget: self.total,
            layers: campaign.layers().keys().map(|layer| (*layer, LayerMonitorSettings {
                levels: self.levels.clone(), budget: self.per_layer,
            })).collect() }
    }
}

impl DecoderCampaign {
    pub fn monitors(&self, settings: &MonitorExport) -> Result<BTreeMap<u64, RefinementMonitor>, Error> {
        self.probes()?.into_iter().map(|(layer, probe)| {
            Ok((layer, RefinementMonitor::new(vec![probe], settings.levels.clone(), settings.per_layer)?))
        }).collect()
    }

    /// Delegate to the original complete-roster serializer and its consumer
    /// bounds. Neither this adapter nor its plan can replace evaluated weights,
    /// change selected thresholds, omit failed layers or authorize promotion.
    pub fn to_monitor_json(&self, settings: &MonitorExport) -> Result<Vec<u8>, Error> {
        self.monitor_json(&settings.for_campaign(self), MAX_MONITOR_CONFIG_BYTES)
    }
}
