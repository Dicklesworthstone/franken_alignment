//! Full decoder continuation under scoped edits to a pinned checkpoint's KV.
//! Numerical experiments have no live captures, checkpoint export or authority.

pub mod comparison;
mod continuation;
pub mod quantized;

use super::{DecoderBudget, DecoderCheckpoint, DecoderModel, DecoderWork};
use super::super::experiment::{
    KvBranch, KvCell, KvEdit, KvEditScope, KvExperiment, KvExperimentLimits, KvSide,
    MAX_KV_EDITS_PER_FORK, MAX_KV_RETAINED_EDITS,
};
use crate::Error;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;
use continuation::{Continuation, Prefix};

pub const MAX_DECODER_INTERVENTION_EDITS: usize = MAX_KV_RETAINED_EDITS;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecoderLayerIntervention {
    pub scope: KvEditScope,
    pub edits: Vec<KvEdit>,
}

struct InterventionData {
    source: DecoderCheckpoint,
    id: u64,
    specification: BTreeMap<u64, DecoderLayerIntervention>,
    branches: BTreeMap<u64, KvBranch>,
    proposed_edits: usize,
    effective_edits: usize,
}

/// All layers are admitted before returning a plan. The original immutable
/// parameter object and source values are retained, not reconstructed by IDs.
/// Clones share the plan and baseline; each continuation owns only its suffix.
#[derive(Clone)]
pub struct DecoderIntervention { data: Rc<InterventionData> }
impl fmt::Debug for DecoderIntervention {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecoderIntervention").field("id", &self.id())
            .field("proposed_edits", &self.proposed_edits())
            .field("effective_edits", &self.effective_edits()).finish_non_exhaustive()
    }
}

impl DecoderCheckpoint {
    /// Scope and exact expected scalar bits are checked by the existing sparse
    /// intervention engine. No arbitrary external/edited KV image is admitted.
    /// The global count charges all proposed records, including explicit no-ops.
    pub fn intervene(
        &self, id: u64, layers: BTreeMap<u64, DecoderLayerIntervention>, edit_limit: usize,
    ) -> Result<DecoderIntervention, Error> {
        if id == 0 { return Err(Error::InvalidInput); }
        if edit_limit > MAX_DECODER_INTERVENTION_EDITS { return Err(Error::Limit); }
        if layers.len() > self.model().profile().shape().layers { return Err(Error::Limit); }
        let proposed_edits = layers.values().try_fold(0_usize, |sum, layer| {
            if layer.edits.len() > MAX_KV_EDITS_PER_FORK { return Err(Error::Limit); }
            sum.checked_add(layer.edits.len()).ok_or(Error::Overflow)
        })?;
        if proposed_edits > edit_limit { return Err(Error::Limit); }
        let mut branches = BTreeMap::new();
        let mut effective_edits = 0;
        for (layer, specification) in &layers {
            let image = self.cache().layer(*layer)?.clone();
            let mut builder = KvExperiment::new(id, image, specification.scope, KvExperimentLimits {
                branches: 1, retained_edits: specification.edits.len().max(1), resolution_depth: 1,
            })?;
            let baseline = builder.baseline();
            let branch = builder.fork(1, &baseline, &specification.edits)?;
            effective_edits += specification.edits.iter()
                .filter(|edit| edit.expected_bits != edit.replacement_bits).count();
            branches.insert(*layer, branch);
        }
        Ok(DecoderIntervention { data: Rc::new(InterventionData {
            source: self.clone(), id, specification: layers, branches, proposed_edits, effective_edits,
        }) })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecoderExperimentArm { Control, Intervention }

impl DecoderIntervention {
    pub fn id(&self) -> u64 { self.data.id }
    /// The unedited original checkpoint, not an export of an experiment's state.
    pub fn source(&self) -> &DecoderCheckpoint { &self.data.source }
    pub fn specification(&self) -> &BTreeMap<u64, DecoderLayerIntervention> { &self.data.specification }
    pub fn proposed_edits(&self) -> usize { self.data.proposed_edits }
    pub fn effective_edits(&self) -> usize { self.data.effective_edits }
    pub fn session(&self, arm: DecoderExperimentArm) -> DecoderExperimentSession {
        DecoderExperimentSession { plan: self.clone(), arm, state: Continuation::default() }
    }
}

/// Results remain explicitly experimental even for an unchanged control arm.
/// No TensorCapture, SourceFrame, original DecoderStep or permission is exposed.
#[derive(Clone)]
pub struct DecoderExperimentStep {
    pub experiment: u64,
    pub arm: DecoderExperimentArm,
    pub token: u32,
    pub position: u64,
    pub logits: Rc<[f32]>,
    pub work: DecoderWork,
}
impl fmt::Debug for DecoderExperimentStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecoderExperimentStep").field("experiment", &self.experiment)
            .field("arm", &self.arm).field("token", &self.token).field("position", &self.position)
            .field("vocabulary", &self.logits.len()).finish_non_exhaustive()
    }
}

/// Owns new numerical KV rows, not a ModelKvCapture. Existing prefix arrays are
/// shared read-only; resolving an intervention never recaptures it as actor data.
/// A failed advance publishes no token, layer, logits or successful-work count.
/// Allocator aborts are outside this Result-level transactional guarantee.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderSession, experiment::DecoderExperimentSession};
/// fn activate(experiment: DecoderExperimentSession) -> DecoderSession { experiment }
/// ```
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderCheckpoint, experiment::DecoderExperimentSession};
/// fn checkpoint(experiment: DecoderExperimentSession) -> DecoderCheckpoint { experiment }
/// ```
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::DecoderExperimentStep;
/// use fa_reference::action::consequence::activation::SourceFrame;
/// fn relabel(step: DecoderExperimentStep) -> SourceFrame { step }
/// ```
pub struct DecoderExperimentSession {
    plan: DecoderIntervention,
    arm: DecoderExperimentArm,
    state: Continuation,
}
impl fmt::Debug for DecoderExperimentSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecoderExperimentSession").field("experiment", &self.plan.id())
            .field("arm", &self.arm).field("position", &self.position()).finish_non_exhaustive()
    }
}
impl DecoderExperimentSession {
    pub fn plan(&self) -> &DecoderIntervention { &self.plan }
    pub fn arm(&self) -> DecoderExperimentArm { self.arm }
    fn prefix(&self) -> InterventionPrefix<'_> { InterventionPrefix { plan: &self.plan, arm: self.arm } }
    pub fn position(&self) -> u64 { self.state.position(&self.prefix()) }
    /// Newly consumed original token IDs only; the prefix belongs to source().
    pub fn continuation_tokens(&self) -> &[u32] { self.state.tokens() }
    pub fn work(&self) -> DecoderWork { self.state.work() }
    /// The old checkpoint logits are deliberately NOT installed: editing a KV
    /// prefix does not recompute its previous output. Supply a first token, then
    /// inspect logits or choose a subsequent token from newly computed outputs.
    pub fn logits(&self) -> Result<&[f32], Error> { self.state.logits() }
    pub fn greedy_token(&self) -> Result<u32, Error> { self.state.greedy_token() }
    pub fn advance_greedy(&mut self, expected_position: u64, budget: DecoderBudget) -> Result<DecoderExperimentStep, Error> {
        if expected_position != self.position() { return Err(Error::Stale); }
        let token = self.greedy_token()?;
        self.advance(expected_position, token, budget)
    }
    pub fn advance(&mut self, expected_position: u64, token: u32, budget: DecoderBudget) -> Result<DecoderExperimentStep, Error> {
        let prefix = InterventionPrefix { plan: &self.plan, arm: self.arm };
        let step = self.state.advance(&prefix, expected_position, token, budget)?;
        Ok(DecoderExperimentStep { experiment: self.plan.id(), arm: self.arm, token: step.token,
            position: step.position, logits: step.logits, work: step.work })
    }

    /// Scalar inspection is experimental data, not a live capture or KV image.
    pub fn bits(&self, layer: u64, cell: KvCell) -> Result<u32, Error> { self.state.bits(&self.prefix(), layer, cell) }
}
struct InterventionPrefix<'a> { plan: &'a DecoderIntervention, arm: DecoderExperimentArm }
impl Prefix for InterventionPrefix<'_> {
    fn model(&self) -> &DecoderModel { self.plan.source().model() }
    fn stream(&self) -> u64 { self.plan.source().stream() }
    fn len(&self) -> usize { self.plan.source().tokens().len() }
    fn bits(&self, layer: u64, cell: KvCell) -> Result<u32, Error> {
        if self.arm == DecoderExperimentArm::Intervention {
            if let Some(branch) = self.plan.data.branches.get(&layer) { return branch.bits(cell); }
        }
        let row = self.plan.source().cache().layer(layer)?.token(cell.position)?;
        let frame = match cell.side { KvSide::Key => row.key(), KvSide::Value => row.value() };
        let index = cell.head * self.model().profile().head_width() + cell.channel;
        frame.words.get(index).copied().ok_or(Error::Missing)
    }
}
