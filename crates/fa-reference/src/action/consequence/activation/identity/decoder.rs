//! Execute registered identity stimuli through the ORIGINAL dense decoder.
//! Each advance computes at most one complete token, in a private fresh cache.
//! It is a cooperative boundary, not a preemptive or wall-clock work guarantee.

use super::{ModelPassport, SourceFrame};
use crate::action::consequence::activation::FrameIdentity;
use crate::action::consequence::activation::tensor::TensorCaptureReceipt;
use crate::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderModel, DecoderSession, MAX_DECODER_PRODUCTS,
};
use crate::Error;
use std::fmt;
use std::rc::Rc;

/// Loop terms are NOT FLOPs or elapsed time. An entered but failed token reports
/// its admitted product bound separately from successfully completed computation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IdentityProbeWork {
    pub planned_tokens: u64,
    pub planned_scalar_products: u64,
    pub entered_tokens: u64,
    pub entered_scalar_product_bound: u64,
    pub completed_tokens: u64,
    pub completed_scalar_products: u64,
    pub measured_anchors: usize,
    /// Cumulative logical bytes emitted, not live allocation or peak memory.
    pub measurement_bytes: usize,
}

/// Source produced by this decoder execution, not an asserted identity verdict.
/// The original tensor receipt retains its token sequence; source.sequence is
/// the independently assigned MEASUREMENT sequence used by the identity gate.
/// Neither sequence is proof of authentication or freshness on its own.
#[derive(Clone)]
pub struct DecoderIdentityMeasurement {
    anchor: u64,
    source: SourceFrame,
    capture: TensorCaptureReceipt,
}
impl DecoderIdentityMeasurement {
    pub fn anchor(&self) -> u64 { self.anchor }
    pub fn source(&self) -> &SourceFrame { &self.source }
    pub fn capture(&self) -> &TensorCaptureReceipt { &self.capture }
}
impl fmt::Debug for DecoderIdentityMeasurement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecoderIdentityMeasurement").field("anchor", &self.anchor)
            .field("identity", &self.source.identity()).finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum IdentityProbeProgress { Advanced, Measured(Box<DecoderIdentityMeasurement>), Complete }

/// Shares only immutable parameters. Every anchor starts from its ORIGINAL
/// tokens and an empty cache, never the actor's active cache or random state.
/// All tokens, residual taps and the WHOLE multi-anchor budget are preflighted.
/// Manifest digests and correspondence to a deployed model remain host inputs.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::identity::decoder::DecoderIdentityProbe;
/// use fa_reference::action::Permit;
/// fn authorize(probe: DecoderIdentityProbe) -> Permit { probe }
/// ```
pub struct DecoderIdentityProbe {
    model: DecoderModel,
    passport: ModelPassport,
    sequence: u64,
    // Native anchor ID and actual registered model layer; never guessed tap IDs.
    plan: Vec<(u64, u64)>,
    next: usize,
    session: Option<DecoderSession>,
    work: IdentityProbeWork,
    failure: Option<Error>,
}
impl fmt::Debug for DecoderIdentityProbe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecoderIdentityProbe").field("work", &self.work)
            .field("failure", &self.failure).finish_non_exhaustive()
    }
}
impl DecoderIdentityProbe {
    pub fn new(model: DecoderModel, passport: &ModelPassport, measurement_sequence: u64,
        budget: DecoderBudget) -> Result<Self, Error>
    {
        if measurement_sequence == 0 { return Err(Error::InvalidInput); }
        if budget.scalar_products > MAX_DECODER_PRODUCTS { return Err(Error::Limit); }
        let id = model.profile().identity();
        let manifest = passport.manifest();
        if id.tenant != manifest.tenant || id.model != manifest.model
            || id.model_generation != manifest.model_generation
            || id.tokenizer_generation != manifest.tokenizer_generation
        { return Err(Error::Binding); }
        let shape = model.profile().shape();
        let mut plan = Vec::new();
        plan.try_reserve_exact(passport.anchors().len()).map_err(|_| Error::Limit)?;
        let mut work = IdentityProbeWork::default();
        for anchor in passport.anchors().values() {
            if anchor.stimulus().iter().any(|token| *token as usize >= shape.vocabulary) {
                return Err(Error::InvalidInput);
            }
            let mut selected = None;
            for layer in 1..=shape.layers as u64 {
                let contract = model.residual_contract(layer)?;
                if contract.profile() == anchor.profile() && contract.dimensions() == anchor.dimensions() {
                    selected = Some(layer);
                    break;
                }
            }
            let layer = selected.ok_or(Error::Binding)?;
            let cost = model.estimate(0, anchor.stimulus().len())?;
            work.planned_tokens = work.planned_tokens.checked_add(cost.tokens).ok_or(Error::Overflow)?;
            work.planned_scalar_products = work.planned_scalar_products
                .checked_add(cost.scalar_products()?).ok_or(Error::Overflow)?;
            if work.planned_scalar_products > budget.scalar_products { return Err(Error::Limit); }
            plan.push((anchor.id(), layer));
        }
        Ok(Self { model, passport: passport.clone(), sequence: measurement_sequence,
            plan, next: 0, session: None, work, failure: None })
    }

    pub fn passport(&self) -> &ModelPassport { &self.passport }
    pub fn measurement_sequence(&self) -> u64 { self.sequence }
    pub fn work(&self) -> IdentityProbeWork { self.work }
    pub fn failure(&self) -> Option<Error> { self.failure }
    pub fn complete(&self) -> bool { self.failure.is_none() && self.next == self.plan.len() }

    /// No inference after completion or terminal failure. Earlier measurements
    /// remain historical; a failure never becomes a successful prefix result.
    pub fn advance(&mut self) -> Result<IdentityProbeProgress, Error> {
        if let Some(error) = self.failure { return Err(error); }
        if self.complete() { return Ok(IdentityProbeProgress::Complete); }
        let result = self.advance_inner();
        if let Err(error) = result { self.failure = Some(error); self.session = None; }
        result
    }

    fn advance_inner(&mut self) -> Result<IdentityProbeProgress, Error> {
        let (id, layer) = self.plan[self.next];
        let anchor = &self.passport.anchors()[&id];
        if self.session.is_none() { self.session = Some(self.model.session(anchor.stream())?); }
        let session = self.session.as_mut().expect("private anchor session");
        let position = usize::try_from(session.position()).map_err(|_| Error::Limit)?;
        let token = *anchor.stimulus().get(position).ok_or(Error::WrongState)?;
        let products = self.model.estimate(position, 1)?.scalar_products()?;
        let entered = self.work.entered_scalar_product_bound.checked_add(products).ok_or(Error::Overflow)?;
        if entered > self.work.planned_scalar_products { return Err(Error::Limit); }
        self.work.entered_tokens += 1;
        self.work.entered_scalar_product_bound = entered;
        let step = session.advance(position as u64, token, DecoderBudget { scalar_products: products })?;
        self.work.completed_tokens += 1;
        self.work.completed_scalar_products += products;
        if session.position() < anchor.stimulus().len() as u64 { return Ok(IdentityProbeProgress::Advanced); }
        let residual = &step.layers.iter().find(|row| row.layer == layer).ok_or(Error::Missing)?.residual;
        let original = residual.source();
        if original.identity().profile != anchor.profile() || original.identity().stream != anchor.stream()
            || original.identity().position != position as u64 || original.dimensions() != anchor.dimensions()
        { return Err(Error::Binding); }
        // A new measurement of actual computed bits, not a relabeled cached
        // passport value. Keep the ORIGINAL capture receipt alongside the source.
        let source = SourceFrame {
            identity: FrameIdentity { sequence: self.sequence, ..original.identity() },
            words: Rc::clone(&original.words), binding: Rc::new(()),
        };
        let measurement = DecoderIdentityMeasurement { anchor: id, capture: residual.receipt().clone(), source };
        self.work.measured_anchors += 1;
        self.work.measurement_bytes += measurement.source.raw_bytes();
        self.session = None;
        self.next += 1;
        Ok(IdentityProbeProgress::Measured(Box::new(measurement)))
    }
}
