//! Sampled validation of the exact all-layer trained campaign, without refitting.
use super::DecoderCampaign;
use super::super::interchange::MonitorExportSettings;
use crate::action::consequence::activation::monitor::decoder::config::MAX_MONITOR_CONFIG_BYTES;
use crate::action::consequence::activation::monitor::decoder::sampled::evaluation::RolloutBuildError;
use crate::action::consequence::activation::monitor::decoder::sampled::evaluation::plan::{PreparedRollout, RolloutPlan};
use crate::Error;
use std::collections::BTreeSet;

impl DecoderCampaign {
    /// The fourth task population cannot reuse ANY training, calibration or
    /// evaluation task/lineage or exact original prefix. This checks declarations
    /// and exact bytes, not semantic near-duplicates or independent label quality.
    /// Coefficients, selected thresholds and immutable model come from this
    /// campaign; the plan cannot substitute them or drop a failed layer.
    pub fn sampled_rollout(&self, settings: MonitorExportSettings, plan: &RolloutPlan)
        -> Result<PreparedRollout, RolloutBuildError>
    {
        let tasks: BTreeSet<_> = self.sources.keys().map(|origin| origin.task).collect();
        let lineages: BTreeSet<_> = self.sources.keys().map(|origin| origin.lineage).collect();
        let prefixes: BTreeSet<&[u32]> = self.sources.values().map(|case| case.tokens.as_slice()).collect();
        if plan.cases().iter().any(|case| tasks.contains(&case.origin.task)
            || lineages.contains(&case.origin.lineage) || prefixes.contains(case.prompt.as_slice()))
        { return Err(Error::Duplicate.into()); }
        let monitor = self.monitor_json(&settings, MAX_MONITOR_CONFIG_BYTES)?;
        plan.bind(self.model.clone(), &monitor)
    }
}
