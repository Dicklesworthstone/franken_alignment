//! Registered numerical helpers for the ORIGINAL source-checked sidecar packet.
//! Only residuals present in that packet are materialized. Probe coefficients
//! and the expected source/view are provisioned before an input arrives; neither
//! a worker request nor a question can replace them. No safety or authority claim.

use super::SidecarCommitteeRound;
use super::receiver::{DisclosedProbe, SidecarReceiveBudget, SidecarReceiver};
use crate::action::consequence::activation::probe::{LinearProbe, ProbeOutcome};
use crate::action::consequence::activation::probe::learned::{
    CheckedLearnedKv, KvGroup, KvRefinementBudget, KvRow,
    MAX_CHECKED_KV_BYTES, MAX_CHECKED_KV_PRODUCTS,
};
use crate::action::consequence::activation::tensor::kv::model::MAX_MODEL_KV_VALUES;
use crate::action::consequence::oversight::helper_workers::{
    HelperPhase, HelperPort, wire::{self, WorkerInput, MAX_HELPER_FRAME_BYTES},
};
use crate::full_input::{InputProfileBinding, MAX_SUBMITTED_BYTES};
use crate::round::Verdict;
use crate::Error;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const MAX_SIDECAR_PROBES: usize = 4096;
pub const MAX_SIDECAR_PROBE_COORDINATES: usize = 1_048_576;

/// Per-evaluation logical bounds, not latency, physical allocation, a campaign
/// escrow or a refill of the source/planner's already consumed allowances.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeHelperBudget {
    pub input_bytes: usize,
    pub probes: usize,
    pub probe_coordinates: usize,
    pub refinement_bytes: usize,
    pub materialized_values: usize,
    /// Conservative reservation: all probes on the coarse view PLUS every
    /// disclosed residual's reconstruction. Refinement may reduce actual scoring.
    pub reconstruction_products: u64,
}
impl Default for ProbeHelperBudget {
    fn default() -> Self {
        Self { input_bytes: MAX_SUBMITTED_BYTES, probes: MAX_SIDECAR_PROBES,
            probe_coordinates: MAX_SIDECAR_PROBE_COORDINATES,
            refinement_bytes: MAX_CHECKED_KV_BYTES, materialized_values: MAX_MODEL_KV_VALUES,
            reconstruction_products: MAX_CHECKED_KV_PRODUCTS }
    }
}
impl ProbeHelperBudget {
    fn admits(self, needed: Self) -> Result<(), Error> {
        let maximum = Self::default();
        if self.input_bytes > maximum.input_bytes || self.probes > maximum.probes
            || self.probe_coordinates > maximum.probe_coordinates
            || self.refinement_bytes > maximum.refinement_bytes
            || self.materialized_values > maximum.materialized_values
            || self.reconstruction_products > maximum.reconstruction_products
            || needed.input_bytes > self.input_bytes || needed.probes > self.probes
            || needed.probe_coordinates > self.probe_coordinates
            || needed.refinement_bytes > self.refinement_bytes
            || needed.materialized_values > self.materialized_values
            || needed.reconstruction_products > self.reconstruction_products
        { return Err(Error::Limit); }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeHelperStatus { AwaitingInput, Evaluating, Judged(Verdict), Failed(Error) }

/// Completed original operations only. A failed/unwound operation may have spent
/// additional work that did not return a receipt; these counters do not erase it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProbeHelperWork {
    pub refined_groups: usize,
    pub refinement_bytes: usize,
    pub materialized_values: usize,
    pub refinement_products: u64,
    pub evaluated_probes: usize,
    pub probe_coordinates: usize,
    pub probe_products: u64,
}

/// Complete numerical observations, not empirical accuracy or a DecisionClosure.
/// Hold is an ordinary helper recommendation, NEVER an exact disqualifier.
#[derive(Clone, Debug)]
pub struct ProbeHelperReport {
    verdict: Verdict,
    observations: Vec<DisclosedProbe>,
    selected: Vec<KvGroup>,
    work: ProbeHelperWork,
}
impl ProbeHelperReport {
    pub fn verdict(&self) -> Verdict { self.verdict }
    pub fn observations(&self) -> &[DisclosedProbe] { &self.observations }
    pub fn selected_groups(&self) -> &[KvGroup] { &self.selected }
    pub fn work(&self) -> ProbeHelperWork { self.work }
}

/// One immutable registered helper for one disclosed input, not a vote callback.
/// It retains only this member's request, never other members' views or judgments.
/// Its source is an existing typed locally checked object, NOT imported packet
/// claims. A deployment must independently provision/authenticate that source.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::sidecar::probe_helper::SidecarProbeEvaluator;
/// fn replace(worker: &mut SidecarProbeEvaluator) { worker.probes_mut(); }
/// ```
/// ```compile_fail,E0308
/// use fa_reference::action::{Permit, consequence::oversight::sidecar::probe_helper::ProbeHelperReport};
/// fn permit(report: ProbeHelperReport) -> Permit { report }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::sidecar::probe_helper::ProbeHelperReport;
/// fn hidden(report: &ProbeHelperReport) { report.observations()[0].view(); }
/// ```
pub struct SidecarProbeEvaluator {
    receiver: Option<SidecarReceiver>,
    profile: InputProfileBinding,
    member: String,
    probes: BTreeMap<KvRow, Vec<LinearProbe>>,
    selected: Vec<KvGroup>,
    reservation: ProbeHelperBudget,
    status: ProbeHelperStatus,
    work: ProbeHelperWork,
    report: Option<ProbeHelperReport>,
}
impl fmt::Debug for SidecarProbeEvaluator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SidecarProbeEvaluator").field("member", &self.member)
            .field("status", &self.status).field("work", &self.work).finish_non_exhaustive()
    }
}
impl SidecarProbeEvaluator {
    /// Requires at least one distinct registered probe for EVERY captured row,
    /// including both K and V. It covers exactly this descriptor, not an unseen
    /// prefix or whole run. Same probe IDs may recur at different positions.
    /// Sizing checks all native profile/bounds before encoding the source again.
    pub fn required_budget(source: &CheckedLearnedKv, round: &SidecarCommitteeRound,
        member: &str, probes: &BTreeMap<KvRow, Vec<LinearProbe>>)
        -> Result<ProbeHelperBudget, Error>
    {
        let expected = round.input().views().get(member).ok_or(Error::Missing)?;
        let rows: BTreeSet<_> = source.groups().map(|group| group.row).collect();
        if rows.is_empty() || probes.len() != rows.len()
            || probes.keys().any(|row| !rows.contains(row)) { return Err(Error::Incomplete); }
        if rows.len() > MAX_SIDECAR_PROBES { return Err(Error::Limit); }
        let mut needed = ProbeHelperBudget { input_bytes: expected.actual_input().submitted_bytes().len(),
            probes: 0, probe_coordinates: 0, refinement_bytes: 0,
            materialized_values: 0, reconstruction_products: 0 };
        let view = source.view();
        for (row, roster) in probes {
            if roster.is_empty() { return Err(Error::Incomplete); }
            needed.probes = add(needed.probes, roster.len())?;
            if needed.probes > MAX_SIDECAR_PROBES { return Err(Error::Limit); }
            let mut identities = BTreeSet::new();
            for probe in roster {
                // A duplicate identity cannot be counted twice or silently carry
                // changed coefficients. Model/tap/layout bind via the original API.
                if !identities.insert(probe.identity().id) { return Err(Error::Duplicate); }
                let work = probe.learned_work(&view, *row)?;
                needed.probe_coordinates = add(needed.probe_coordinates, work.coordinates)?;
                needed.reconstruction_products = add_products(needed.reconstruction_products,
                    work.reconstruction_products)?;
                ProbeHelperBudget::default().admits(needed)?;
            }
        }
        let rank = source.image().codec().policy().rank() as u64;
        let mut previous = None;
        let mut payload_bytes = add(24, source.report().base_encoded_bytes)?;
        for group in round.selected_groups() {
            if previous.is_some_and(|value| value >= *group) { return Err(Error::Binding); }
            previous = Some(*group);
            let bytes = source.residual_bytes(*group)?.len();
            let values = source.channels(*group)?;
            needed.refinement_bytes = add(needed.refinement_bytes, bytes)?;
            needed.materialized_values = add(needed.materialized_values, values)?;
            needed.reconstruction_products = add_products(needed.reconstruction_products,
                (values as u64).checked_mul(rank).ok_or(Error::Overflow)?)?;
            payload_bytes = add(payload_bytes, add(8, bytes)?)?;
        }
        ProbeHelperBudget::default().admits(needed)?;
        // Prevent even a trusted but very large source from causing an oversized
        // comparison allocation. No source values are decoded by this sizing pass.
        if payload_bytes != round.payload().len() || payload_bytes > needed.input_bytes {
            return Err(Error::Binding);
        }
        Ok(needed)
    }

    /// Freeze the exact ORIGINAL member/round/root/salt-limit binding before any
    /// commitment is queued. The existing receiver alone admits and promotes
    /// the disclosed source; no source/view escapes through a probe report.
    pub fn new(port: &HelperPort, round: &SidecarCommitteeRound, source: CheckedLearnedKv,
        probes: BTreeMap<KvRow, Vec<LinearProbe>>, budget: ProbeHelperBudget) -> Result<Self, Error>
    {
        if port.phase() != HelperPhase::AwaitCommit { return Err(Error::WrongState); }
        let member = port.request().member();
        let reservation = Self::required_budget(&source, round, member, &probes)?;
        budget.admits(reservation)?;
        let receiver = SidecarReceiver::new(port, round, source, SidecarReceiveBudget {
            payload_bytes: reservation.input_bytes, request_bytes: MAX_HELPER_FRAME_BYTES,
            residual: KvRefinementBudget {
                encoded_bytes: reservation.refinement_bytes,
                materialized_values: reservation.materialized_values,
                reconstruction_products: reservation.reconstruction_products,
            },
        })?;
        let profile = receiver.input_profile().clone();
        Ok(Self { receiver: Some(receiver), profile, member: member.to_owned(), probes,
            selected: round.selected_groups().to_vec(), reservation,
            status: ProbeHelperStatus::AwaitingInput, work: ProbeHelperWork::default(), report: None })
    }

    pub fn input_profile(&self) -> &InputProfileBinding { &self.profile }
    pub fn status(&self) -> ProbeHelperStatus { self.status }
    pub fn reservation(&self) -> ProbeHelperBudget { self.reservation }
    pub fn work(&self) -> ProbeHelperWork { self.work }
    pub fn report(&self) -> Option<&ProbeHelperReport> { self.report.as_ref() }

    /// Local delivery uses the ORIGINAL wire encoder and decoder too, so it
    /// cannot accidentally omit round/root/salt-limit checks from its binding.
    pub fn evaluate_port(&mut self, port: &HelperPort) -> Result<Verdict, Error> {
        self.start()?;
        let result = wire::encode_request(port).and_then(|bytes| wire::decode_request(&bytes))
            .and_then(|input| self.evaluate_actual(&input));
        self.finish(result)
    }

    /// The original receiver compares the complete request before reconstruction:
    /// round, root, member, salt limit, actual bytes, profile, parts and omissions.
    /// Equality is local binding, not cryptographic origin authentication.
    pub fn evaluate(&mut self, input: &WorkerInput) -> Result<Verdict, Error> {
        self.start()?;
        let result = self.evaluate_actual(input);
        self.finish(result)
    }

    fn start(&mut self) -> Result<(), Error> {
        if self.status != ProbeHelperStatus::AwaitingInput { return Err(Error::WrongState); }
        // A caught unwind or an invalid first input cannot retry this owner.
        self.status = ProbeHelperStatus::Evaluating;
        Ok(())
    }
    fn finish(&mut self, result: Result<ProbeHelperReport, Error>) -> Result<Verdict, Error> {
        match result {
            Ok(report) => {
                let verdict = report.verdict;
                self.report = Some(report); self.status = ProbeHelperStatus::Judged(verdict); Ok(verdict)
            }
            Err(error) => { self.status = ProbeHelperStatus::Failed(error); Err(error) }
        }
    }
    fn evaluate_actual(&mut self, input: &WorkerInput) -> Result<ProbeHelperReport, Error> {
        let receiver = self.receiver.take().ok_or(Error::WrongState)?;
        let received = receiver.receive(input)?;
        let work = received.work();
        self.work.refined_groups = work.disclosed_groups;
        self.work.refinement_bytes = work.residual_bytes;
        self.work.materialized_values = work.materialized_values;
        self.work.refinement_products = work.reconstruction_products;
        let mut observations = Vec::new();
        observations.try_reserve_exact(self.reservation.probes).map_err(|_| Error::Limit)?;
        let mut alarm = false;
        let mut uncertain = false;
        for (row, probes) in &self.probes {
            for probe in probes {
                let remaining_coordinates = self.reservation.probe_coordinates
                    .checked_sub(self.work.probe_coordinates).ok_or(Error::Binding)?;
                let remaining_products = self.reservation.reconstruction_products
                    .checked_sub(add_products(self.work.refinement_products, self.work.probe_products)?)
                    .ok_or(Error::Binding)?;
                let needed = received.probe_work(probe, *row)?;
                if needed.coordinates > remaining_coordinates || needed.reconstruction_products > remaining_products {
                    return Err(Error::Limit);
                }
                // The receiver projects only numerical observations; returning
                // the original LearnedProbeObservation would expose hidden data.
                let observation = received.evaluate_probe(probe, *row, needed)?;
                let work = observation.work();
                self.work.evaluated_probes += 1;
                self.work.probe_coordinates = add(self.work.probe_coordinates, work.coordinates)?;
                self.work.probe_products = add_products(self.work.probe_products, work.reconstruction_products)?;
                alarm |= observation.outcome() == ProbeOutcome::CertifiedAlarm;
                uncertain |= matches!(observation.outcome(), ProbeOutcome::NeedsRefinement | ProbeOutcome::AtThreshold);
                observations.push(observation);
            }
        }
        if self.work.evaluated_probes != self.reservation.probes
            || self.work.probe_coordinates != self.reservation.probe_coordinates
            || add_products(self.work.refinement_products, self.work.probe_products)? > self.reservation.reconstruction_products
        { return Err(Error::Binding); }
        let verdict = if alarm { Verdict::Hold } else if uncertain { Verdict::Abstain } else { Verdict::Allow };
        Ok(ProbeHelperReport { verdict, observations, selected: self.selected.clone(), work: self.work })
    }
}
fn add(a: usize, b: usize) -> Result<usize, Error> { a.checked_add(b).ok_or(Error::Overflow) }
fn add_products(a: u64, b: u64) -> Result<u64, Error> { a.checked_add(b).ok_or(Error::Overflow) }
