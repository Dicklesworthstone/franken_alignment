//! Adaptive monitoring of SOURCE-CHECKED learned KV, using the original exact
//! probe evaluator and original XOR-refinement implementation. No fitted MSE,
//! approximate score, missing residual or exhausted budget can certify quiet.
pub mod model;
use super::{MonitorOutcome, RefinementBudget, RefinementMonitor};
use super::super::{CaptureProfile, FrameIdentity};
use super::super::probe::{LinearProbe, ProbeIdentity, ProbeOutcome};
use super::super::probe::learned::{CheckedLearnedKv, KvGroup, KvRow, KvRefinementBudget,
    KvRefinementReceipt, LearnedKvView, LearnedProbeObservation, MAX_CHECKED_KV_BYTES,
    MAX_CHECKED_KV_GROUPS, MAX_CHECKED_KV_PRODUCTS};
use super::super::tensor::kv::model::MAX_MODEL_KV_VALUES;
use crate::Error;
use std::collections::BTreeMap;

pub const MAX_LEARNED_MONITOR_COORDINATES: usize = 16_777_216;
/// Bounds retained per-row observation history independently of numerical work.
pub const MAX_LEARNED_ROW_REFINEMENTS: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LearnedMonitorBudget {
    /// Base representation once, plus the exact residual blocks actually used.
    /// Already-retained evidence storage is separately priced by CheckedKvBudget.
    pub encoded_bytes: usize,
    pub probe_coordinates: usize,
    /// Coarse scoring plus reconstructing groups for exact promotion.
    pub reconstruction_products: u64,
    pub materialized_values: usize,
    pub refinements: usize,
}
impl Default for LearnedMonitorBudget {
    fn default() -> Self {
        Self { encoded_bytes: MAX_CHECKED_KV_BYTES, probe_coordinates: MAX_LEARNED_MONITOR_COORDINATES,
            reconstruction_products: MAX_CHECKED_KV_PRODUCTS, materialized_values: MAX_MODEL_KV_VALUES,
            refinements: MAX_LEARNED_ROW_REFINEMENTS }
    }
}
impl LearnedMonitorBudget {
    pub(super) fn check(self) -> Result<(), Error> {
        if self.encoded_bytes > MAX_CHECKED_KV_BYTES || self.probe_coordinates > MAX_LEARNED_MONITOR_COORDINATES
            || self.reconstruction_products > MAX_CHECKED_KV_PRODUCTS || self.materialized_values > MAX_MODEL_KV_VALUES
            || self.refinements > MAX_CHECKED_KV_GROUPS { return Err(Error::Limit); }
        Ok(())
    }
    pub(super) fn intersect(self, other: Self) -> Self {
        Self { encoded_bytes: self.encoded_bytes.min(other.encoded_bytes),
            probe_coordinates: self.probe_coordinates.min(other.probe_coordinates),
            reconstruction_products: self.reconstruction_products.min(other.reconstruction_products),
            materialized_values: self.materialized_values.min(other.materialized_values),
            refinements: self.refinements.min(other.refinements) }
    }
    pub(super) fn remaining(self, used: LearnedMonitorWork) -> Result<Self, Error> {
        if !used.fits(self) { return Err(Error::Binding); }
        Ok(Self { encoded_bytes: self.encoded_bytes - used.encoded_bytes,
            probe_coordinates: self.probe_coordinates - used.probe_coordinates,
            reconstruction_products: self.reconstruction_products - used.reconstruction_products,
            materialized_values: self.materialized_values - used.materialized_values,
            refinements: self.refinements - used.refinements })
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LearnedMonitorWork {
    pub encoded_bytes: usize,
    pub probe_coordinates: usize,
    pub reconstruction_products: u64,
    pub materialized_values: usize,
    pub refinements: usize,
}
impl LearnedMonitorWork {
    pub(super) fn add(self, other: Self) -> Result<Self, Error> {
        Ok(Self { encoded_bytes: self.encoded_bytes.checked_add(other.encoded_bytes).ok_or(Error::Overflow)?,
            probe_coordinates: self.probe_coordinates.checked_add(other.probe_coordinates).ok_or(Error::Overflow)?,
            reconstruction_products: self.reconstruction_products.checked_add(other.reconstruction_products).ok_or(Error::Overflow)?,
            materialized_values: self.materialized_values.checked_add(other.materialized_values).ok_or(Error::Overflow)?,
            refinements: self.refinements.checked_add(other.refinements).ok_or(Error::Overflow)? })
    }
    pub(super) fn fits(self, budget: LearnedMonitorBudget) -> bool {
        self.encoded_bytes <= budget.encoded_bytes && self.probe_coordinates <= budget.probe_coordinates
            && self.reconstruction_products <= budget.reconstruction_products
            && self.materialized_values <= budget.materialized_values && self.refinements <= budget.refinements
    }
}

#[derive(Clone, Debug)]
pub struct LearnedMonitorStep {
    /// None for the initial coarse pass. Later receipts belong to the original
    /// immutable source and exact branch-local refinement, not a new capture.
    pub refinement: Option<KvRefinementReceipt>,
    pub observations: Vec<LearnedProbeObservation>,
    pub cumulative_work: LearnedMonitorWork,
}
/// NoAlarm means only that every frozen probe certified quiet on THIS source
/// row. It is not detector qualification, model-wide coverage, or effect authority.
///
/// ```compile_fail,E0308
/// use fa_reference::action::{Permit, consequence::activation::monitor::learned::LearnedMonitorReport};
/// fn authorize(report: LearnedMonitorReport) -> Permit { report }
/// ```
#[derive(Clone, Debug)]
pub struct LearnedMonitorReport {
    frame: FrameIdentity,
    row: KvRow,
    probes: Vec<ProbeIdentity>,
    outcome: MonitorOutcome,
    steps: Vec<LearnedMonitorStep>,
    view: LearnedKvView,
    work: LearnedMonitorWork,
    unavailable_groups: Vec<KvGroup>,
}
impl LearnedMonitorReport {
    pub fn frame(&self) -> FrameIdentity { self.frame }
    pub fn row(&self) -> KvRow { self.row }
    pub fn probes(&self) -> &[ProbeIdentity] { &self.probes }
    pub fn outcome(&self) -> MonitorOutcome { self.outcome }
    pub fn steps(&self) -> &[LearnedMonitorStep] { &self.steps }
    pub fn view(&self) -> &LearnedKvView { &self.view }
    pub fn work(&self) -> LearnedMonitorWork { self.work }
    /// Missing retained blocks relevant at the last unsuccessful selection.
    /// An available sibling is tried before declaring these gaps unresolved.
    pub fn unavailable_groups(&self) -> &[KvGroup] { &self.unavailable_groups }
}

#[derive(Clone, Debug)]
pub struct LearnedRefinementMonitor {
    roster: RefinementMonitor,
    budget: LearnedMonitorBudget,
}
impl LearnedRefinementMonitor {
    pub fn new(probes: Vec<LinearProbe>, budget: LearnedMonitorBudget) -> Result<Self, Error> {
        budget.check()?;
        // Reuse the original nonempty/common-profile/unique-probe validation.
        // Its scalar-codec ladder is not executed by this learned-row adapter.
        let roster = RefinementMonitor::new(probes, vec![23], RefinementBudget { encoded_bytes: 0, probe_coordinates: 0 })?;
        Ok(Self { roster, budget })
    }
    pub fn profile(&self) -> CaptureProfile { self.roster.profile() }
    pub fn dimensions(&self) -> usize { self.roster.dimensions() }
    pub fn budget(&self) -> LearnedMonitorBudget { self.budget }
    pub fn analyze(&self, source: &CheckedLearnedKv, row: KvRow) -> Result<LearnedMonitorReport, Error> {
        self.analyze_with_budget(source, row, self.budget)
    }
    /// Remaining shared allowance may tighten, never enlarge, frozen limits.
    pub fn analyze_with_budget(&self, source: &CheckedLearnedKv, row: KvRow,
        remaining: LearnedMonitorBudget) -> Result<LearnedMonitorReport, Error>
    {
        remaining.check()?;
        self.run(source, row, self.budget.intersect(remaining), true)
    }

    pub(super) fn check_row(&self, source: &CheckedLearnedKv, row: KvRow) -> Result<FrameIdentity, Error> {
        let (frame, heads, channels) = source.row_shape(row)?;
        if frame.profile != self.profile() || heads.checked_mul(channels).ok_or(Error::Overflow)? != self.dimensions() {
            return Err(Error::Binding);
        }
        Ok(frame)
    }

    /// Private shared-base entry point for a complete model audit. No public
    /// caller can claim a free base or insert pre-refined, uncharged evidence.
    pub(super) fn run(&self, source: &CheckedLearnedKv, row: KvRow,
        remaining: LearnedMonitorBudget, charge_base: bool) -> Result<LearnedMonitorReport, Error>
    {
        let frame = self.check_row(source, row)?;
        let budget = self.budget.intersect(remaining);
        let mut report = LearnedMonitorReport { frame, row,
            probes: self.roster.probes.iter().map(LinearProbe::identity).collect(),
            outcome: MonitorOutcome::Unresolved, steps: Vec::new(), view: source.view(),
            work: LearnedMonitorWork::default(), unavailable_groups: Vec::new() };
        let mut latest = vec![None; self.roster.probes.len()];
        let all: Vec<usize> = (0..latest.len()).collect();
        let mut cost = self.pass_cost(&report.view, row, &all)?;
        if charge_base { cost.encoded_bytes = source.report().base_encoded_bytes; }
        if !cost.fits(budget) { report.outcome = MonitorOutcome::BudgetExhausted; return Ok(report); }
        report.steps.try_reserve(1).map_err(|_| Error::Limit)?;
        let observations = self.evaluate(&report.view, row, &all, &mut latest)?;
        report.work = cost;
        report.steps.push(LearnedMonitorStep { refinement: None, observations, cumulative_work: cost });
        loop {
            if let Some(outcome) = outcome(&latest) { report.outcome = outcome; return Ok(report); }
            let mut candidates: BTreeMap<KvGroup, Vec<(usize, u64)>> = BTreeMap::new();
            for (index, probe) in self.roster.probes.iter().enumerate() {
                if latest[index] != Some(ProbeOutcome::NeedsRefinement) { continue; }
                for dependency in probe.learned_dependencies(&report.view, row)? {
                    let affected = candidates.entry(dependency.group).or_default();
                    affected.try_reserve(1).map_err(|_| Error::Limit)?;
                    affected.push((index, dependency.reconstruction_products));
                }
            }
            let mut selected = None;
            let mut unaffordable = false;
            report.unavailable_groups.clear();
            for (group, affected) in candidates {
                let bytes = match source.residual_bytes(group) {
                    Ok(bytes) => bytes,
                    Err(Error::Missing) => {
                        report.unavailable_groups.try_reserve(1).map_err(|_| Error::Limit)?;
                        report.unavailable_groups.push(group); continue;
                    }
                    Err(error) => return Err(error),
                };
                let channels = source.channels(group)?;
                let refinement_products = (channels as u64).checked_mul(source.image().codec().policy().rank() as u64).ok_or(Error::Overflow)?;
                let mut cost = LearnedMonitorWork { encoded_bytes: bytes.len(), materialized_values: channels,
                    reconstruction_products: refinement_products, refinements: 1, ..LearnedMonitorWork::default() };
                for (index, removed_products) in &affected {
                    let next = self.roster.probes[*index].learned_work(&report.view, row)?;
                    cost.probe_coordinates = cost.probe_coordinates.checked_add(next.coordinates).ok_or(Error::Overflow)?;
                    cost.reconstruction_products = cost.reconstruction_products.checked_add(
                        next.reconstruction_products.checked_sub(*removed_products).ok_or(Error::Binding)?).ok_or(Error::Overflow)?;
                }
                let total = report.work.add(cost)?;
                if !total.fits(budget) || total.refinements > MAX_LEARNED_ROW_REFINEMENTS {
                    unaffordable = true; continue;
                }
                selected = Some((group, affected, total, refinement_products)); break;
            }
            let Some((group, affected, total, refinement_products)) = selected else {
                report.outcome = if unaffordable { MonitorOutcome::BudgetExhausted } else { MonitorOutcome::Unresolved };
                return Ok(report);
            };
            report.steps.try_reserve(1).map_err(|_| Error::Limit)?;
            let block = source.verify_residual(group, source.residual_bytes(group)?)?;
            let refinement = report.view.refine(report.view.revision(), &block, KvRefinementBudget {
                encoded_bytes: block.encoded_bytes(), materialized_values: source.channels(group)?,
                reconstruction_products: refinement_products,
            })?;
            let affected: Vec<usize> = affected.into_iter().map(|(index, _)| index).collect();
            let observations = self.evaluate(&report.view, row, &affected, &mut latest)?;
            report.work = total;
            report.steps.push(LearnedMonitorStep { refinement: Some(refinement), observations, cumulative_work: total });
            report.unavailable_groups.clear();
        }
    }
    fn pass_cost(&self, view: &LearnedKvView, row: KvRow, indices: &[usize]) -> Result<LearnedMonitorWork, Error> {
        let mut cost = LearnedMonitorWork::default();
        for index in indices {
            let work = self.roster.probes[*index].learned_work(view, row)?;
            cost.probe_coordinates = cost.probe_coordinates.checked_add(work.coordinates).ok_or(Error::Overflow)?;
            cost.reconstruction_products = cost.reconstruction_products.checked_add(work.reconstruction_products).ok_or(Error::Overflow)?;
        }
        Ok(cost)
    }
    fn evaluate(&self, view: &LearnedKvView, row: KvRow, indices: &[usize], latest: &mut [Option<ProbeOutcome>])
        -> Result<Vec<LearnedProbeObservation>, Error>
    {
        let mut observations = Vec::new(); observations.try_reserve_exact(indices.len()).map_err(|_| Error::Limit)?;
        for index in indices {
            let observation = self.roster.probes[*index].evaluate_learned(view, row)?;
            latest[*index] = Some(observation.outcome()); observations.push(observation);
        }
        Ok(observations)
    }
}
fn outcome(latest: &[Option<ProbeOutcome>]) -> Option<MonitorOutcome> {
    if latest.contains(&Some(ProbeOutcome::CertifiedAlarm)) { Some(MonitorOutcome::Alarm) }
    else if latest.contains(&Some(ProbeOutcome::AtThreshold)) { Some(MonitorOutcome::AtThreshold) }
    else if latest.iter().all(|value| *value == Some(ProbeOutcome::CertifiedQuiet)) { Some(MonitorOutcome::NoAlarm) }
    else { None }
}
