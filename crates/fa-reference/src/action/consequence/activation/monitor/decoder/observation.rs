//! Live, read-only evidence from one actual monitored numerical owner.
//! Historical evidence is not currentness, authority, or producer authentication.

use super::{DecoderProfile, DecoderReview, MonitorOutcome};
use crate::Error;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecoderAvailability { Empty, Computing, Ready, Held, Failed, Closed }

struct Prefix {
    tokens: Vec<u32>,
    review: Option<Rc<DecoderReview>>,
}
struct Shared {
    issuer: Rc<()>,
    profile: DecoderProfile,
    generation: u64,
    stream: u64,
    availability: Cell<DecoderAvailability>,
    prefix: RefCell<Prefix>,
}

/// Trusted observer only. Cloning preserves the same source, not a new writer.
/// It cannot accept caller-supplied tokens, reports or a replacement owner.
#[derive(Clone)]
pub struct DecoderObservation { shared: Rc<Shared> }
impl fmt::Debug for DecoderObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecoderObservation").field("profile", self.profile())
            .field("stream", &self.stream()).field("generation", &self.generation())
            .field("availability", &self.availability()).finish_non_exhaustive()
    }
}

/// An immutable copy of a fully reviewed original-token prefix and its final
/// all-layer report. Tokens are copied only on capture, not on every inference
/// step. Clones share that copy. All preceding steps were also compulsory quiet
/// reviews in the same non-resettable owner; this is not a trace of their scores.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::monitor::decoder::observation::DecoderEvidence;
/// use fa_reference::action::Permit;
/// fn grant(evidence: DecoderEvidence) -> Permit { evidence }
/// ```
#[derive(Clone)]
pub struct DecoderEvidence {
    issuer: Rc<()>,
    profile: DecoderProfile,
    tokens: Rc<[u32]>,
    review: Rc<DecoderReview>,
}
impl fmt::Debug for DecoderEvidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecoderEvidence").field("profile", &self.profile)
            .field("next_position", &self.next_position()).field("generation", &self.generation())
            .finish_non_exhaustive()
    }
}
impl DecoderEvidence {
    pub fn profile(&self) -> &DecoderProfile { &self.profile }
    pub fn tokens(&self) -> &[u32] { &self.tokens }
    pub fn next_position(&self) -> u64 { self.tokens.len() as u64 }
    pub fn generation(&self) -> u64 { self.review.generation() }
    pub fn stream(&self) -> u64 { self.review.stream() }
    pub fn review(&self) -> &DecoderReview { &self.review }
}
impl DecoderObservation {
    pub fn profile(&self) -> &DecoderProfile { &self.shared.profile }
    pub fn generation(&self) -> u64 { self.shared.generation }
    pub fn stream(&self) -> u64 { self.shared.stream }
    pub fn availability(&self) -> DecoderAvailability { self.shared.availability.get() }

    /// Capture only a currently live, completely quiet prefix. No raw logits,
    /// mutable session, sampler, KV cache or control authority are exposed.
    pub fn capture(&self) -> Result<DecoderEvidence, Error> {
        self.check_ready()?;
        let prefix = self.shared.prefix.try_borrow().map_err(|_| Error::Incomplete)?;
        let review = prefix.review.as_ref().ok_or(Error::Incomplete)?;
        let mut tokens = Vec::new();
        tokens.try_reserve_exact(prefix.tokens.len()).map_err(|_| Error::Limit)?;
        tokens.extend_from_slice(&prefix.tokens);
        Ok(DecoderEvidence { issuer: Rc::clone(&self.shared.issuer), profile: self.shared.profile.clone(),
            tokens: tokens.into(), review: Rc::clone(review) })
    }

    /// Original source AND original last review must still be current. Matching
    /// numeric identities, token counts, logits or probe outcomes cannot import
    /// another owner's evidence. No token vector is copied during validation.
    pub fn validate(&self, evidence: &DecoderEvidence) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.shared.issuer, &evidence.issuer) { return Err(Error::Binding); }
        self.check_ready()?;
        let prefix = self.shared.prefix.try_borrow().map_err(|_| Error::Incomplete)?;
        if !prefix.review.as_ref().is_some_and(|current| Rc::ptr_eq(current, &evidence.review))
            || prefix.tokens.len() != evidence.tokens.len()
        { return Err(Error::Stale); }
        Ok(())
    }

    fn check_ready(&self) -> Result<(), Error> {
        if self.availability() != DecoderAvailability::Ready { return Err(Error::Incomplete); }
        Ok(())
    }
}

/// The only writer is private to the monitored inference owner. Cell invalidation
/// precedes fallible execution and also works on Drop without borrowing Prefix.
pub(super) struct ObservationWriter { shared: Rc<Shared> }
impl ObservationWriter {
    pub(super) fn new(profile: DecoderProfile, generation: u64, stream: u64) -> Self {
        Self { shared: Rc::new(Shared { issuer: Rc::new(()), profile, generation, stream,
            availability: Cell::new(DecoderAvailability::Empty),
            prefix: RefCell::new(Prefix { tokens: Vec::new(), review: None }) }) }
    }
    pub(super) fn observe(&self) -> DecoderObservation { DecoderObservation { shared: Rc::clone(&self.shared) } }

    pub(super) fn begin(&self, position: u64) -> Result<(), Error> {
        if !matches!(self.shared.availability.get(), DecoderAvailability::Empty | DecoderAvailability::Ready) {
            return Err(Error::WrongState);
        }
        let mut prefix = self.shared.prefix.try_borrow_mut().map_err(|_| Error::Incomplete)?;
        if position != prefix.tokens.len() as u64 { return Err(Error::Stale); }
        if prefix.tokens.len() >= self.shared.profile.shape().context { return Err(Error::Limit); }
        prefix.tokens.try_reserve(1).map_err(|_| Error::Limit)?;
        self.shared.availability.set(DecoderAvailability::Computing);
        Ok(())
    }

    pub(super) fn publish(&self, token: u32, review: Rc<DecoderReview>) -> Result<(), Error> {
        if self.shared.availability.get() != DecoderAvailability::Computing { return Err(Error::WrongState); }
        let mut prefix = self.shared.prefix.try_borrow_mut().map_err(|_| Error::Incomplete)?;
        let shape = self.shared.profile.shape();
        if token as usize >= shape.vocabulary || review.position() != prefix.tokens.len() as u64
            || review.stream() != self.shared.stream || review.generation() != self.shared.generation
            || review.required_layers() != shape.layers
        { return Err(Error::Binding); }
        let quiet = review.outcome() == MonitorOutcome::NoAlarm && review.unreviewed_layers() == 0;
        prefix.tokens.push(token);
        prefix.review = Some(review);
        self.shared.availability.set(if quiet { DecoderAvailability::Ready } else { DecoderAvailability::Held });
        Ok(())
    }
    pub(super) fn fail(&self) { self.shared.availability.set(DecoderAvailability::Failed); }
}
impl Drop for ObservationWriter {
    fn drop(&mut self) { self.shared.availability.set(DecoderAvailability::Closed); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::consequence::activation::tensor::kv::decoder::{DecoderIdentity, DecoderShape};
    fn writer() -> ObservationWriter {
        ObservationWriter::new(DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2,
            model_generation: 3, tokenizer_generation: 4, profile_generation: 5 },
            DecoderShape { vocabulary: 2, hidden: 2, intermediate: 2, layers: 1,
                query_heads: 1, cache_heads: 1, context: 2 }, 0.00001, 10000.0).unwrap(), 7, 8)
    }
    #[test]
    fn admitted_work_is_unavailable_before_any_fallible_numerical_operation() {
        let writer = writer(); let source = writer.observe();
        writer.begin(0).unwrap();
        assert_eq!(source.availability(), DecoderAvailability::Computing);
        assert!(matches!(source.capture(), Err(Error::Incomplete)));
        writer.fail();
        assert_eq!(source.availability(), DecoderAvailability::Failed);
        assert_eq!(writer.begin(0), Err(Error::WrongState));
    }
    #[test]
    fn drop_withdraws_even_while_an_internal_prefix_borrow_is_held() {
        let writer = writer(); let source = writer.observe();
        let _borrow = source.shared.prefix.borrow();
        drop(writer);
        assert_eq!(source.availability(), DecoderAvailability::Closed);
        assert!(matches!(source.capture(), Err(Error::Incomplete)));
    }
}
