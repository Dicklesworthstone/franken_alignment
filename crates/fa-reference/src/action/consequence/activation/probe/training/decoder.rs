//! Original-token residual capture and complete, three-split probe campaigns.
//! Reuses the decoder, corpus, optimizer and exact scorer. No serving authority.

mod campaign;
mod export;
pub mod plan;
pub mod trajectory;
pub use export::MonitorExport;
pub use campaign::{CampaignBudget, CampaignWork, DecoderCampaign, LayerCampaign, LayerPolicy};

use super::{CaseLabel, CaseOrigin, ClassCounts, DataSplit, ProbeCorpus, SealedCorpus,
    MAX_CORPUS_CASES, MAX_CORPUS_COORDINATES};
use crate::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderModel, DecoderProfile, MAX_DECODER_PRODUCTS,
};
use crate::Error;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

pub const MAX_CAPTURE_TOKENS: usize = 262_144;

/// One selected final-token residual per layer and declared independent origin.
/// Labels and task/lineage independence are supplied, not inferred from tokens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LabelledPrefix {
    pub origin: CaseOrigin,
    pub split: DataSplit,
    pub label: CaseLabel,
    pub tokens: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CaptureWork {
    pub cases: usize,
    pub original_tokens: usize,
    pub residual_coordinates: usize,
    pub scalar_products: u64,
}

/// Persistent admission allowance. Charged before the first numerical operation;
/// a failed admitted run does not restore it. Charges bound planned work, not RSS
/// or actual completed operations on a failed run. No hidden per-case renewal.
#[derive(Debug)]
pub struct CaptureBudget { remaining: CaptureWork }
impl CaptureBudget {
    pub fn new(limits: CaptureWork) -> Result<Self, Error> {
        if limits.cases > MAX_CORPUS_CASES || limits.original_tokens > MAX_CAPTURE_TOKENS
            || limits.residual_coordinates > MAX_CORPUS_COORDINATES
            || limits.scalar_products > MAX_DECODER_PRODUCTS { return Err(Error::Limit); }
        Ok(Self { remaining: limits })
    }
    pub fn remaining(&self) -> CaptureWork { self.remaining }
    fn admit(&mut self, work: CaptureWork) -> Result<(), Error> {
        let current = self.remaining;
        let next = CaptureWork {
            cases: current.cases.checked_sub(work.cases).ok_or(Error::Limit)?,
            original_tokens: current.original_tokens.checked_sub(work.original_tokens).ok_or(Error::Limit)?,
            residual_coordinates: current.residual_coordinates.checked_sub(work.residual_coordinates).ok_or(Error::Limit)?,
            scalar_products: current.scalar_products.checked_sub(work.scalar_products).ok_or(Error::Limit)?,
        };
        self.remaining = next;
        Ok(())
    }
}

/// Immutable all-layer corpus computed by the supplied model. Original prefixes
/// remain available for audit; neither scores nor operator-generated activations
/// can be supplied instead. The profile declares identity, not weight authenticity.
pub struct DecoderCorpus {
    model: DecoderModel,
    profile: DecoderProfile,
    cases: Rc<BTreeMap<CaseOrigin, LabelledPrefix>>,
    layers: BTreeMap<u64, SealedCorpus>,
    work: CaptureWork,
}
impl fmt::Debug for DecoderCorpus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecoderCorpus").field("profile", &self.profile)
            .field("work", &self.work).finish_non_exhaustive()
    }
}
impl DecoderCorpus {
    /// Validate all token IDs, contexts, origins and class/split denominators.
    /// The declared task ID is the decoder stream; no stream is reused by a case.
    pub fn estimate(model: &DecoderModel, cases: &[LabelledPrefix]) -> Result<CaptureWork, Error> {
        if cases.is_empty() { return Err(Error::InvalidInput); }
        if cases.len() > MAX_CORPUS_CASES { return Err(Error::Limit); }
        let shape = model.profile().shape();
        let residual_coordinates = cases.len().checked_mul(shape.layers)
            .and_then(|n| n.checked_mul(shape.hidden)).ok_or(Error::Overflow)?;
        if residual_coordinates > MAX_CORPUS_COORDINATES { return Err(Error::Limit); }
        let mut tasks = BTreeSet::new();
        let mut lineages = BTreeSet::new();
        let mut counts: BTreeMap<DataSplit, ClassCounts> = BTreeMap::new();
        let mut work = CaptureWork { cases: cases.len(), residual_coordinates, ..CaptureWork::default() };
        for case in cases {
            if case.origin.task == 0 || case.origin.lineage == 0 || case.tokens.is_empty() {
                return Err(Error::InvalidInput);
            }
            if !tasks.insert(case.origin.task) || !lineages.insert(case.origin.lineage) { return Err(Error::Duplicate); }
            if case.tokens.len() > shape.context { return Err(Error::Limit); }
            work.original_tokens = work.original_tokens.checked_add(case.tokens.len()).ok_or(Error::Overflow)?;
            if work.original_tokens > MAX_CAPTURE_TOKENS { return Err(Error::Limit); }
            if case.tokens.iter().any(|id| *id as usize >= shape.vocabulary) { return Err(Error::InvalidInput); }
            work.scalar_products = work.scalar_products.checked_add(model.estimate(0, case.tokens.len())?.scalar_products()?)
                .ok_or(Error::Overflow)?;
            if work.scalar_products > MAX_DECODER_PRODUCTS { return Err(Error::Limit); }
            let count = counts.entry(case.split).or_default();
            match case.label { CaseLabel::Benign => count.benign += 1, CaseLabel::Violation => count.violation += 1 }
        }
        if counts.len() != 3 || counts.values().any(|count| count.benign == 0 || count.violation == 0) {
            return Err(Error::Incomplete);
        }
        Ok(work)
    }

    /// All-layer capture is one admitted operation. Numerical failures return no
    /// partial corpus. Only each prefix's actual final residual is retained, but
    /// EVERY original token executes through the original causal decoder first.
    pub fn capture(model: &DecoderModel, id: u64, generation: u64, cases: &[LabelledPrefix],
        budget: &mut CaptureBudget) -> Result<Self, Error>
    {
        if id == 0 || generation == 0 { return Err(Error::InvalidInput); }
        let work = Self::estimate(model, cases)?;
        let assignments: BTreeMap<_, _> = cases.iter().map(|case| (case.origin, case.split)).collect();
        let mut builders = BTreeMap::new();
        for layer in 1..=model.profile().shape().layers as u64 {
            let contract = model.residual_contract(layer)?;
            builders.insert(layer, ProbeCorpus::new(id, generation, contract.profile(),
                contract.dimensions(), assignments.clone())?);
        }
        let cases: BTreeMap<_, _> = cases.iter().map(|case| (case.origin, case.clone())).collect();
        budget.admit(work)?;
        let mut products = 0_u64;
        for (origin, case) in &cases {
            let mut session = model.session(origin.task)?;
            let mut final_step = None;
            for (position, token) in case.tokens.iter().copied().enumerate() {
                let scalar_products = model.estimate(position, 1)?.scalar_products()?;
                final_step = Some(session.advance(position as u64, token, DecoderBudget { scalar_products })?);
            }
            products = products.checked_add(session.work().scalar_products()?).ok_or(Error::Overflow)?;
            let step = final_step.ok_or(Error::Incomplete)?;
            if step.layers.len() != builders.len() || step.position + 1 != case.tokens.len() as u64 {
                return Err(Error::Binding);
            }
            for (index, observation) in step.layers.into_iter().enumerate() {
                if observation.layer != index as u64 + 1 { return Err(Error::Binding); }
                let source = observation.residual.source();
                let identity = source.identity();
                if identity.stream != origin.task || identity.position != step.position
                    || identity.sequence != case.tokens.len() as u64 { return Err(Error::Binding); }
                builders.get_mut(&observation.layer).ok_or(Error::Binding)?
                    .capture(*origin, case.label, source.clone())?;
            }
        }
        if products != work.scalar_products { return Err(Error::Binding); }
        let layers = builders.into_iter().map(|(layer, corpus)| Ok((layer, corpus.seal()?)))
            .collect::<Result<BTreeMap<_, _>, Error>>()?;
        Ok(Self { model: model.clone(), profile: model.profile().clone(), cases: Rc::new(cases), layers, work })
    }

    pub fn profile(&self) -> &DecoderProfile { &self.profile }
    pub fn work(&self) -> CaptureWork { self.work }
    pub fn cases(&self) -> &BTreeMap<CaseOrigin, LabelledPrefix> { &self.cases }
    pub fn layers(&self) -> &BTreeMap<u64, SealedCorpus> { &self.layers }
}
