//! Complete learned-cache audits, not isolated quiet rows relabeled as coverage.
//! Every registered layer and both K/V sides are mandatory. Source construction,
//! score certificates and selected exact promotions use the original paths.
use super::{LearnedMonitorBudget, LearnedMonitorReport, LearnedMonitorWork, LearnedRefinementMonitor};
use super::super::MonitorOutcome;
use super::super::super::probe::learned::{CheckedKvBudget, CheckedLearnedKv, KvRow, ResidualRetention};
use super::super::super::tensor::kv::decoder::DecoderCheckpoint;
use super::super::super::tensor::kv::experiment::KvSide;
use super::super::super::tensor::kv::model::{ModelKvImage, ModelKvProfile};
use super::super::super::tensor::kv::model::learned::{CompressionBudget, CompressionReport, LearnedKvCodec};
use crate::Error;
use std::collections::BTreeMap;

pub const MAX_LEARNED_AUDIT_ROWS: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct KvTap { pub layer: u64, pub side: KvSide }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LearnedAuditBudget {
    /// Complete planned row-report capacity, not an allowance to omit late rows.
    pub rows: usize,
    /// Aggregate across ALL rows, with the checked base counted only once.
    pub monitoring: LearnedMonitorBudget,
}
impl Default for LearnedAuditBudget {
    fn default() -> Self { Self { rows: MAX_LEARNED_AUDIT_ROWS, monitoring: LearnedMonitorBudget::default() } }
}

#[derive(Clone, Debug)]
pub struct LearnedModelMonitor {
    profile: ModelKvProfile,
    taps: BTreeMap<KvTap, LearnedRefinementMonitor>,
    budget: LearnedAuditBudget,
}
impl LearnedModelMonitor {
    pub fn new(profile: ModelKvProfile, taps: BTreeMap<KvTap, LearnedRefinementMonitor>, budget: LearnedAuditBudget)
        -> Result<Self, Error>
    {
        budget.monitoring.check()?;
        if budget.rows > MAX_LEARNED_AUDIT_ROWS { return Err(Error::Limit); }
        if taps.len() != profile.layers().len().checked_mul(2).ok_or(Error::Overflow)? { return Err(Error::Binding); }
        for (layer, contract) in profile.layers() {
            for (side, tensor) in [(KvSide::Key, contract.keys()), (KvSide::Value, contract.values())] {
                let monitor = taps.get(&KvTap { layer: *layer, side }).ok_or(Error::Binding)?;
                if monitor.profile() != tensor.profile() || monitor.dimensions() != tensor.dimensions() { return Err(Error::Binding); }
            }
        }
        Ok(Self { profile, taps, budget })
    }
    pub fn profile(&self) -> &ModelKvProfile { &self.profile }
    pub fn taps(&self) -> &BTreeMap<KvTap, LearnedRefinementMonitor> { &self.taps }
    pub fn budget(&self) -> LearnedAuditBudget { self.budget }

    /// Entire captured interval, in absolute-position then layer/side order.
    /// There is no caller-supplied subset or unknown-layer skip. Empty evidence
    /// is incomplete, not a vacuous NoAlarm. The fitted codec is never retuned.
    pub fn analyze(&self, source: &CheckedLearnedKv) -> Result<LearnedModelReport, Error> {
        self.analyze_with_budget(source, self.budget.monitoring)
    }

    /// A higher-level stream owner may conserve one allowance across many
    /// independently captured rows. It can only tighten this monitor's frozen
    /// aggregate; rows, taps and every per-row probe budget remain unchanged.
    pub fn analyze_with_budget(&self, source: &CheckedLearnedKv, remaining: LearnedMonitorBudget)
        -> Result<LearnedModelReport, Error>
    {
        remaining.check()?;
        self.analyze_inner(source, self.budget.monitoring.intersect(remaining))
    }

    fn analyze_inner(&self, source: &CheckedLearnedKv, budget: LearnedMonitorBudget)
        -> Result<LearnedModelReport, Error>
    {
        if source.image().codec().profile() != self.profile() { return Err(Error::Binding); }
        let first = source.descriptor().layers().values().next().ok_or(Error::Incomplete)?;
        let positions = first.token_count;
        if positions == 0 { return Err(Error::Incomplete); }
        let planned_rows = positions.checked_mul(self.taps.len()).ok_or(Error::Overflow)?;
        let end_position = first.first_position.checked_add(positions as u64).ok_or(Error::Overflow)?;
        // Check every registered row's metadata before even a zero-budget report
        // can be returned. Complete descriptors already bind a common layer cut.
        for position in first.first_position..end_position {
            for (tap, monitor) in &self.taps {
                monitor.check_row(source, KvRow { layer: tap.layer, side: tap.side, position })?;
            }
        }
        let mut report = LearnedModelReport { monitor: self.clone(), source: source.clone(),
            first_position: first.first_position, end_position, planned_rows, quiet_rows: 0,
            rows: Vec::new(), outcome: MonitorOutcome::Unresolved, work: LearnedMonitorWork::default(), blocked_row: None };
        let base = source.report().base_encoded_bytes;
        if planned_rows > self.budget.rows || base > budget.encoded_bytes {
            report.outcome = MonitorOutcome::BudgetExhausted; return Ok(report);
        }
        report.rows.try_reserve_exact(planned_rows).map_err(|_| Error::Limit)?;
        report.work.encoded_bytes = base;
        for position in first.first_position..end_position {
            for (tap, monitor) in &self.taps {
                let row = KvRow { layer: tap.layer, side: tap.side, position };
                let mut remaining = budget.remaining(report.work)?;
                // Each row still obeys its original base-inclusive fixed budget.
                // The aggregate has already paid that same immutable base, so it
                // can be credited here, but only the private caller computes it.
                remaining.encoded_bytes = remaining.encoded_bytes.checked_add(base).ok_or(Error::Overflow)?;
                let result = monitor.analyze_with_budget(source, row, remaining)?;
                let mut added = result.work();
                if !result.steps().is_empty() {
                    added.encoded_bytes = added.encoded_bytes.checked_sub(base).ok_or(Error::Binding)?;
                }
                let next = report.work.add(added)?;
                if !next.fits(budget) { return Err(Error::Binding); }
                let result_outcome = result.outcome();
                report.work = next;
                report.rows.push(result);
                if result_outcome != MonitorOutcome::NoAlarm {
                    report.outcome = result_outcome; report.blocked_row = Some(row); return Ok(report);
                }
                report.quiet_rows += 1;
            }
        }
        if report.quiet_rows != planned_rows { return Err(Error::Binding); }
        report.outcome = MonitorOutcome::NoAlarm;
        Ok(report)
    }
}

/// Immutable audit evidence over the exact retained source descriptor and frozen
/// monitor roster. A partial scan retains both its denominator and stopping row.
/// Neither a quiet prefix nor an empty/missing row can construct complete quiet.
///
/// ```compile_fail,E0308
/// use fa_reference::action::{Permit, consequence::activation::monitor::learned::model::LearnedModelReport};
/// fn release(report: LearnedModelReport) -> Permit { report }
/// ```
#[derive(Clone, Debug)]
pub struct LearnedModelReport {
    monitor: LearnedModelMonitor,
    source: CheckedLearnedKv,
    first_position: u64,
    end_position: u64,
    planned_rows: usize,
    quiet_rows: usize,
    rows: Vec<LearnedMonitorReport>,
    outcome: MonitorOutcome,
    work: LearnedMonitorWork,
    blocked_row: Option<KvRow>,
}
impl LearnedModelReport {
    pub fn monitor(&self) -> &LearnedModelMonitor { &self.monitor }
    pub fn source(&self) -> &CheckedLearnedKv { &self.source }
    pub fn first_position(&self) -> u64 { self.first_position }
    pub fn end_position(&self) -> u64 { self.end_position }
    pub fn planned_rows(&self) -> usize { self.planned_rows }
    pub fn quiet_rows(&self) -> usize { self.quiet_rows }
    pub fn rows(&self) -> &[LearnedMonitorReport] { &self.rows }
    pub fn examined_rows(&self) -> usize { self.rows.iter().filter(|row| !row.steps().is_empty()).count() }
    pub fn unexamined_rows(&self) -> usize { self.planned_rows - self.examined_rows() }
    pub fn blocked_row(&self) -> Option<KvRow> { self.blocked_row }
    pub fn outcome(&self) -> MonitorOutcome { self.outcome }
    pub fn work(&self) -> LearnedMonitorWork { self.work }
    pub fn complete_quiet(&self) -> bool { self.outcome == MonitorOutcome::NoAlarm && self.quiet_rows == self.planned_rows }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LearnedAuditPreparationBudget {
    pub compression: CompressionBudget,
    /// Full retained representation, including residuals not subsequently used.
    pub source_check: CheckedKvBudget,
}
impl Default for LearnedAuditPreparationBudget {
    fn default() -> Self { Self { compression: CompressionBudget::default(), source_check: CheckedKvBudget::default() } }
}
#[derive(Clone, Debug)]
pub struct LearnedCheckpointAudit {
    evaluation_origin: u64,
    compression: CompressionReport,
    monitoring: LearnedModelReport,
}
impl LearnedCheckpointAudit {
    pub fn evaluation_origin(&self) -> u64 { self.evaluation_origin }
    pub fn compression(&self) -> &CompressionReport { &self.compression }
    pub fn monitoring(&self) -> &LearnedModelReport { &self.monitoring }
}
impl DecoderCheckpoint {
    /// Audit the actual original-engine captured cache: independent fitted codec
    /// -> compressed image -> original source-check -> complete adaptive roster.
    /// No source arrays or original model checkpoint are retained by the returned
    /// audit, apart from explicitly budgeted exact residual information.
    ///
    /// This is a checkpoint audit, not a token-release or live authority adapter.
    /// Repeated whole-prefix calls have cumulative quadratic traffic; this API
    /// does not conceal that cost or pretend to implement incremental capture.
    pub fn audit_learned_cache(&self, evaluation_origin: u64, codec: &LearnedKvCodec,
        monitor: &LearnedModelMonitor, retention: ResidualRetention, budget: LearnedAuditPreparationBudget)
        -> Result<LearnedCheckpointAudit, Error>
    {
        let source: &ModelKvImage = self.cache();
        if source.is_empty() { return Err(Error::Incomplete); }
        if source.profile() != self.model().cache_profile() || source.profile() != monitor.profile()
            || source.len() != self.tokens().len() { return Err(Error::Binding); }
        let planned = source.len().checked_mul(monitor.taps.len()).ok_or(Error::Overflow)?;
        if planned > monitor.budget.rows { return Err(Error::Limit); }
        let (image, compression) = codec.evaluate_held_out(evaluation_origin, source, budget.compression)?;
        let checked = CheckedLearnedKv::new(image, source, retention, budget.source_check)?;
        let monitoring = monitor.analyze(&checked)?;
        Ok(LearnedCheckpointAudit { evaluation_origin, compression, monitoring })
    }
}
