//! Train complete learned-cache probe rosters from the original decoder's K/V.
//! Original token execution and immutable SourceFrames feed the existing corpus,
//! optimizer, calibrator and exact scorer. No model, score or authority substitute.
mod campaign;
mod export;
pub use campaign::{KvCampaignBudget, KvCampaignWork, KvDecoderCampaign, KvTapCampaign};
pub use export::{KvMonitorSettings, MAX_KV_MONITOR_JSON_BYTES};

use super::{LabelledPrefix, MAX_CAPTURE_TOKENS};
use super::super::{CaseLabel, CaseOrigin, ClassCounts, DataSplit, ProbeCorpus, SealedCorpus,
    MAX_CORPUS_CASES, MAX_CORPUS_COORDINATES};
use crate::action::consequence::activation::monitor::learned::model::KvTap;
use crate::action::consequence::activation::tensor::kv::{experiment::KvSide,
    decoder::{DecoderBudget, DecoderModel, DecoderProfile, MAX_DECODER_PRODUCTS}};
use crate::Error;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KvCaptureWork {
    pub cases: usize,
    pub original_tokens: usize,
    /// Retained final-token K/V coordinates across all cases, layers and sides.
    pub kv_coordinates: usize,
    pub scalar_products: u64,
}

/// Whole-cohort admission, charged before numerical work. Failure never restores
/// it. Retained coordinates exclude the original decoder's temporary full cache;
/// that cache remains bounded by its original model/context limits.
#[derive(Debug)]
pub struct KvCaptureBudget { remaining: KvCaptureWork }
impl KvCaptureBudget {
    pub fn new(limits: KvCaptureWork) -> Result<Self, Error> {
        if limits.cases > MAX_CORPUS_CASES || limits.original_tokens > MAX_CAPTURE_TOKENS
            || limits.kv_coordinates > MAX_CORPUS_COORDINATES
            || limits.scalar_products > MAX_DECODER_PRODUCTS { return Err(Error::Limit); }
        Ok(Self { remaining: limits })
    }
    pub fn remaining(&self) -> KvCaptureWork { self.remaining }
    fn admit(&mut self, work: KvCaptureWork) -> Result<(), Error> {
        let current = self.remaining;
        let next = KvCaptureWork {
            cases: current.cases.checked_sub(work.cases).ok_or(Error::Limit)?,
            original_tokens: current.original_tokens.checked_sub(work.original_tokens).ok_or(Error::Limit)?,
            kv_coordinates: current.kv_coordinates.checked_sub(work.kv_coordinates).ok_or(Error::Limit)?,
            scalar_products: current.scalar_products.checked_sub(work.scalar_products).ok_or(Error::Limit)?,
        };
        self.remaining = next;
        Ok(())
    }
}

/// One complete, immutable K/V population. Case task is the original decoder
/// stream; lineage is the separately declared experiment origin. Labels and
/// origin declarations are not authenticated by numerical execution.
pub struct KvDecoderCorpus {
    model: DecoderModel,
    cases: Rc<BTreeMap<CaseOrigin, LabelledPrefix>>,
    taps: BTreeMap<KvTap, SealedCorpus>,
    work: KvCaptureWork,
}
impl fmt::Debug for KvDecoderCorpus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KvDecoderCorpus").field("profile", self.model.profile())
            .field("work", &self.work).finish_non_exhaustive()
    }
}
impl KvDecoderCorpus {
    /// Validate the entire population, including every tail token, before any
    /// inference. Distinct task/lineage IDs and distinct exact histories are
    /// required across all three splits. Near-duplicate independence is not proved.
    pub fn estimate(model: &DecoderModel, cases: &[LabelledPrefix]) -> Result<KvCaptureWork, Error> {
        if cases.is_empty() { return Err(Error::InvalidInput); }
        if cases.len() > MAX_CORPUS_CASES { return Err(Error::Limit); }
        let shape = model.profile().shape();
        let kv_coordinates = cases.len().checked_mul(model.cache_profile().values_per_token())
            .ok_or(Error::Overflow)?;
        if kv_coordinates > MAX_CORPUS_COORDINATES { return Err(Error::Limit); }
        let mut tasks = BTreeSet::new();
        let mut lineages = BTreeSet::new();
        let mut histories = BTreeSet::new();
        let mut counts: BTreeMap<DataSplit, ClassCounts> = BTreeMap::new();
        let mut work = KvCaptureWork { cases: cases.len(), kv_coordinates, ..KvCaptureWork::default() };
        for case in cases {
            if case.origin.task == 0 || case.origin.lineage == 0 || case.tokens.is_empty() {
                return Err(Error::InvalidInput);
            }
            if !tasks.insert(case.origin.task) || !lineages.insert(case.origin.lineage)
                || !histories.insert(case.tokens.as_slice()) { return Err(Error::Duplicate); }
            if case.tokens.len() > shape.context { return Err(Error::Limit); }
            if case.tokens.iter().any(|token| *token as usize >= shape.vocabulary) { return Err(Error::InvalidInput); }
            work.original_tokens = work.original_tokens.checked_add(case.tokens.len()).ok_or(Error::Overflow)?;
            work.scalar_products = work.scalar_products.checked_add(model.estimate(0, case.tokens.len())?.scalar_products()?)
                .ok_or(Error::Overflow)?;
            if work.original_tokens > MAX_CAPTURE_TOKENS || work.scalar_products > MAX_DECODER_PRODUCTS {
                return Err(Error::Limit);
            }
            let count = counts.entry(case.split).or_default();
            match case.label { CaseLabel::Benign => count.benign += 1, CaseLabel::Violation => count.violation += 1 }
        }
        if counts.len() != 3 || counts.values().any(|count| count.benign == 0 || count.violation == 0) {
            return Err(Error::Incomplete);
        }
        Ok(work)
    }

    /// Execute each original prefix once, then retain its actual final K/V
    /// SourceFrames. No flattened array or caller-selected tap subset is accepted.
    pub fn capture(model: &DecoderModel, id: u64, generation: u64, cases: &[LabelledPrefix],
        budget: &mut KvCaptureBudget) -> Result<Self, Error>
    {
        if id == 0 || generation == 0 { return Err(Error::InvalidInput); }
        let work = Self::estimate(model, cases)?;
        let assignments: BTreeMap<_, _> = cases.iter().map(|case| (case.origin, case.split)).collect();
        let mut builders = BTreeMap::new();
        for (layer, contract) in model.cache_profile().layers() {
            for (side, tensor) in [(KvSide::Key, contract.keys()), (KvSide::Value, contract.values())] {
                builders.insert(KvTap { layer: *layer, side }, ProbeCorpus::new(id, generation,
                    tensor.profile(), tensor.dimensions(), assignments.clone())?);
            }
        }
        budget.admit(work)?;
        let cases: BTreeMap<_, _> = cases.iter().map(|case| (case.origin, case.clone())).collect();
        let mut products = 0_u64;
        let mut captured = 0_usize;
        for (origin, case) in &cases {
            let session = model.recompute(origin.task, &case.tokens, DecoderBudget {
                scalar_products: model.estimate(0, case.tokens.len())?.scalar_products()?,
            })?;
            products = products.checked_add(session.work().scalar_products()?).ok_or(Error::Overflow)?;
            let cache = session.cache_image()?;
            let position = case.tokens.len() as u64 - 1;
            if cache.profile() != model.cache_profile() || cache.len() != case.tokens.len()
                || session.tokens() != case.tokens { return Err(Error::Binding); }
            for (tap, corpus) in &mut builders {
                let token = cache.layer(tap.layer)?.token(position)?;
                let source = match tap.side { KvSide::Key => token.key(), KvSide::Value => token.value() };
                let frame = source.identity();
                if frame.stream != origin.task || frame.position != position
                    || frame.sequence != case.tokens.len() as u64 { return Err(Error::Binding); }
                captured = captured.checked_add(source.dimensions()).ok_or(Error::Overflow)?;
                corpus.capture(*origin, case.label, source.clone())?;
            }
        }
        if products != work.scalar_products || captured != work.kv_coordinates { return Err(Error::Binding); }
        let taps = builders.into_iter().map(|(tap, corpus)| Ok((tap, corpus.seal()?)))
            .collect::<Result<BTreeMap<_, _>, Error>>()?;
        Ok(Self { model: model.clone(), cases: Rc::new(cases), taps, work })
    }

    pub fn model(&self) -> &DecoderModel { &self.model }
    pub fn profile(&self) -> &DecoderProfile { self.model.profile() }
    pub fn cases(&self) -> &BTreeMap<CaseOrigin, LabelledPrefix> { &self.cases }
    pub fn taps(&self) -> &BTreeMap<KvTap, SealedCorpus> { &self.taps }
    pub fn work(&self) -> KvCaptureWork { self.work }
}
