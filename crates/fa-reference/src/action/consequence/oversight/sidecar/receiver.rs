//! Interpret only the source-checked numerical evidence actually sent to a helper.
//! A trusted local source binding is required; wire bytes cannot certify their
//! own error bounds. No raw cache, undisclosed residual or permit is exported.

use super::{SidecarCommitteeRound, encode_payload};
use crate::action::consequence::activation::FrameIdentity;
use crate::action::consequence::activation::monitor::learned::MAX_LEARNED_MONITOR_COORDINATES;
use crate::action::consequence::activation::probe::{LinearProbe, ProbeIdentity, ProbeOutcome, ScoreInterval};
use crate::action::consequence::activation::probe::learned::{
    CheckedLearnedKv, KvGroup, KvRefinementBudget, KvRow, LearnedKvView, LearnedProbeWork,
    MAX_CHECKED_KV_BYTES, MAX_CHECKED_KV_PRODUCTS,
};
use crate::action::consequence::activation::tensor::kv::model::MAX_MODEL_KV_VALUES;
use crate::action::consequence::oversight::helper_workers::{HelperPort, wire::{
    WorkerInput, MAX_HELPER_FRAME_BYTES, decode_request, encode_request,
}};
use crate::full_input::{InputProfileBinding, MAX_SUBMITTED_BYTES};
use crate::Error;
use std::collections::BTreeSet;
use std::fmt;

/// Whole packet and aggregate exact-disclosure admission. The original wire
/// encoder additionally has its own fixed frame cap. Logical counts are not RSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SidecarReceiveBudget {
    pub payload_bytes: usize,
    pub request_bytes: usize,
    pub residual: KvRefinementBudget,
}
impl Default for SidecarReceiveBudget {
    fn default() -> Self {
        Self { payload_bytes: MAX_SUBMITTED_BYTES, request_bytes: MAX_HELPER_FRAME_BYTES,
            residual: KvRefinementBudget::default() }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SidecarReceiveWork {
    pub payload_bytes: usize,
    pub request_bytes: usize,
    pub disclosed_groups: usize,
    pub residual_bytes: usize,
    pub materialized_values: usize,
    pub reconstruction_products: u64,
}

/// Trusted provisioning from one ORIGINAL port, its exact sidecar packet and
/// already checked source. This has no standalone byte importer or source getter.
/// It is consumed by receive, and cannot be retuned to buy another residual.
/// Equal bytes establish a local binding, not remote producer authentication.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::sidecar::receiver::SidecarReceiver;
/// fn hidden(receiver: SidecarReceiver) { receiver.source(); }
/// ```
pub struct SidecarReceiver {
    expected: WorkerInput,
    source: CheckedLearnedKv,
    selected: Vec<KvGroup>,
    admitted: SidecarReceiveWork,
}
impl fmt::Debug for SidecarReceiver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SidecarReceiver").field("member", &self.expected.member())
            .field("round", &self.expected.round()).field("admitted", &self.admitted)
            .finish_non_exhaustive()
    }
}
impl SidecarReceiver {
    pub fn new(port: &HelperPort, packet: &SidecarCommitteeRound, source: CheckedLearnedKv,
        budget: SidecarReceiveBudget) -> Result<Self, Error>
    {
        if budget.payload_bytes > MAX_SUBMITTED_BYTES || budget.request_bytes > MAX_HELPER_FRAME_BYTES
            || budget.residual.encoded_bytes > MAX_CHECKED_KV_BYTES
            || budget.residual.materialized_values > MAX_MODEL_KV_VALUES
            || budget.residual.reconstruction_products > MAX_CHECKED_KV_PRODUCTS {
            return Err(Error::Limit);
        }
        let request = port.request();
        if packet.input().views().get(request.member()) != Some(request.view()) { return Err(Error::Binding); }
        let selected: BTreeSet<_> = packet.selected_groups().iter().copied().collect();
        if selected.len() != packet.selected_groups().len() { return Err(Error::Duplicate); }
        let mut work = SidecarReceiveWork { payload_bytes: 24_usize
            .checked_add(source.report().base_encoded_bytes).ok_or(Error::Overflow)?,
            disclosed_groups: selected.len(), ..SidecarReceiveWork::default() };
        // Size EVERY selected block before encoding or materializing any group.
        for group in &selected {
            let bytes = source.residual_bytes(*group)?.len();
            let values = source.channels(*group)?;
            work.payload_bytes = work.payload_bytes.checked_add(8)
                .and_then(|n| n.checked_add(bytes)).ok_or(Error::Overflow)?;
            work.residual_bytes = work.residual_bytes.checked_add(bytes).ok_or(Error::Overflow)?;
            work.materialized_values = work.materialized_values.checked_add(values).ok_or(Error::Overflow)?;
        }
        work.reconstruction_products = (work.materialized_values as u64)
            .checked_mul(source.image().codec().policy().rank() as u64).ok_or(Error::Overflow)?;
        if work.payload_bytes > budget.payload_bytes || work.residual_bytes > budget.residual.encoded_bytes
            || work.materialized_values > budget.residual.materialized_values
            || work.reconstruction_products > budget.residual.reconstruction_products { return Err(Error::Limit); }
        if encode_payload(&source, &selected)? != packet.payload() { return Err(Error::Binding); }
        // Reuse BOTH original wire functions. Structural equality later includes
        // root, round, member, salt limit, all submitted bytes, parts and profile.
        let frame = encode_request(port)?;
        work.request_bytes = frame.len();
        if work.request_bytes > budget.request_bytes { return Err(Error::Limit); }
        let expected = decode_request(&frame)?;
        Ok(Self { expected, source, selected: selected.into_iter().collect(), admitted: work })
    }
    pub fn input_profile(&self) -> &InputProfileBinding { self.expected.actual_input().input_profile() }
    pub fn admitted_work(&self) -> SidecarReceiveWork { self.admitted }

    /// Compare the full received ORIGINAL request before reconstructing anything.
    /// Only packet-selected blocks are promoted with the original exact refiner.
    /// Failure cannot return a partially materialized numerical view.
    pub fn receive(self, input: &WorkerInput) -> Result<ReceivedSidecar, Error> {
        if input != &self.expected { return Err(Error::Binding); }
        let mut view = self.source.view();
        let mut bytes = 0_usize;
        let mut values = 0_usize;
        let mut products = 0_u64;
        for group in self.selected {
            let block = self.source.verify_residual(group, self.source.residual_bytes(group)?)?;
            let receipt = view.refine(view.revision(), &block, KvRefinementBudget {
                encoded_bytes: self.admitted.residual_bytes.checked_sub(bytes).ok_or(Error::Binding)?,
                materialized_values: self.admitted.materialized_values.checked_sub(values).ok_or(Error::Binding)?,
                reconstruction_products: self.admitted.reconstruction_products.checked_sub(products).ok_or(Error::Binding)?,
            })?;
            bytes = bytes.checked_add(receipt.encoded_bytes).ok_or(Error::Overflow)?;
            values = values.checked_add(receipt.materialized_values).ok_or(Error::Overflow)?;
            products = products.checked_add(receipt.reconstruction_products).ok_or(Error::Overflow)?;
        }
        if (bytes, values, products) != (self.admitted.residual_bytes,
            self.admitted.materialized_values, self.admitted.reconstruction_products) { return Err(Error::Binding); }
        Ok(ReceivedSidecar { input: self.expected, view, work: self.admitted })
    }
}

/// This receiver intentionally does NOT expose LearnedKvView or the original
/// LearnedProbeObservation: either would expose its source's hidden residuals.
/// Only the existing exact score, its identity, interval and work cross out.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::sidecar::receiver::ReceivedSidecar;
/// fn hidden(received: ReceivedSidecar) { received.view(); }
/// ```
pub struct ReceivedSidecar { input: WorkerInput, view: LearnedKvView, work: SidecarReceiveWork }
impl fmt::Debug for ReceivedSidecar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReceivedSidecar").field("round", &self.input.round())
            .field("member", &self.input.member()).field("work", &self.work).finish_non_exhaustive()
    }
}
impl ReceivedSidecar {
    pub fn input(&self) -> &WorkerInput { &self.input }
    pub fn work(&self) -> SidecarReceiveWork { self.work }
    pub fn disclosed_groups(&self) -> impl Iterator<Item = KvGroup> + '_ { self.view.refined_groups() }
    pub fn probe_work(&self, probe: &LinearProbe, row: KvRow) -> Result<LearnedProbeWork, Error> {
        probe.learned_work(&self.view, row)
    }
    /// This call has its own allowance. Higher-level evaluation conserves a
    /// whole-inventory allowance; repeated explicit calls are not free or cached.
    pub fn evaluate_probe(&self, probe: &LinearProbe, row: KvRow, budget: LearnedProbeWork)
        -> Result<DisclosedProbe, Error>
    {
        if budget.coordinates > MAX_LEARNED_MONITOR_COORDINATES
            || budget.reconstruction_products > MAX_CHECKED_KV_PRODUCTS { return Err(Error::Limit); }
        let work = self.probe_work(probe, row)?;
        if work.coordinates > budget.coordinates || work.reconstruction_products > budget.reconstruction_products {
            return Err(Error::Limit);
        }
        let observation = probe.evaluate_learned(&self.view, row)?;
        if observation.work() != work { return Err(Error::Binding); }
        Ok(DisclosedProbe { frame: observation.frame(), row: observation.row(),
            probe: observation.probe(), interval: observation.interval().clone(),
            outcome: observation.outcome(), work })
    }
}

/// A numerical measurement, not a complete action judgment or a permit. All
/// fields originate in the original exact accumulator; no source/view escapes.
#[derive(Clone, Debug)]
pub struct DisclosedProbe {
    frame: FrameIdentity,
    row: KvRow,
    probe: ProbeIdentity,
    interval: ScoreInterval,
    outcome: ProbeOutcome,
    work: LearnedProbeWork,
}
impl DisclosedProbe {
    pub fn frame(&self) -> FrameIdentity { self.frame }
    pub fn row(&self) -> KvRow { self.row }
    pub fn probe(&self) -> ProbeIdentity { self.probe }
    pub fn interval(&self) -> &ScoreInterval { &self.interval }
    pub fn outcome(&self) -> ProbeOutcome { self.outcome }
    pub fn work(&self) -> LearnedProbeWork { self.work }
}
