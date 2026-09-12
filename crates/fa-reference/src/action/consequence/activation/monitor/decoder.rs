//! Connect computed decoder residuals to the existing source-checked monitors.
//! This is a synchronous numerical boundary, not an OS sandbox or effect gate.

pub mod config;
pub mod observation;

use super::{MonitorOutcome, RefinementBudget, RefinementMonitor, RefinementReport};
use super::super::tensor::kv::decoder::{
    DecoderBudget, DecoderModel, DecoderProfile, DecoderSession, DecoderStep,
    DecoderWork, MAX_DECODER_PRODUCTS,
};
use crate::Error;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonitoringStatus {
    Ready,
    Held,
    Failed(Error),
}

/// Completed monitor reports only. Decoder work is reported separately, including
/// a computed token whose review holds. These are not wall-clock or RSS bounds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MonitoringWork {
    pub frame_reviews: u64,
    pub encoded_bytes: usize,
    pub probe_coordinates: usize,
    pub codec_coordinates: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerReview {
    pub layer: u64,
    pub report: RefinementReport,
}

/// Complete for the declared residual-only contract iff every layer is quiet.
/// On the first non-quiet layer, subsequent layers are explicitly unreviewed.
/// The generation is a caller-declared policy identity, not authentication.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecoderReview {
    generation: u64,
    stream: u64,
    position: u64,
    required_layers: usize,
    outcome: MonitorOutcome,
    layers: Vec<LayerReview>,
}
impl DecoderReview {
    pub fn generation(&self) -> u64 { self.generation }
    pub fn stream(&self) -> u64 { self.stream }
    pub fn position(&self) -> u64 { self.position }
    pub fn required_layers(&self) -> usize { self.required_layers }
    pub fn unreviewed_layers(&self) -> usize { self.required_layers - self.layers.len() }
    pub fn outcome(&self) -> MonitorOutcome { self.outcome }
    pub fn layers(&self) -> &[LayerReview] { &self.layers }
}

/// The inner step cannot be obtained from a held or failed review. Its logits
/// remain numerical observations: this type has no conversion into a Permit.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::monitor::decoder::ReviewedStep;
/// use fa_reference::action::Permit;
/// fn grant(step: ReviewedStep) -> Permit { step }
/// ```
pub struct ReviewedStep {
    step: DecoderStep,
    review: Rc<DecoderReview>,
}
impl ReviewedStep {
    pub fn step(&self) -> &DecoderStep { &self.step }
    pub fn review(&self) -> &DecoderReview { &self.review }
}
impl fmt::Debug for ReviewedStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReviewedStep").field("position", &self.step.position)
            .field("review", &self.review).finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum MonitoredStep {
    Released(ReviewedStep),
    Held(Rc<DecoderReview>),
}
impl MonitoredStep {
    pub fn review(&self) -> &DecoderReview {
        match self { Self::Released(step) => step.review(), Self::Held(review) => review }
    }
}

/// Owns the original decoder and exactly one fixed monitor per layer residual.
/// It cannot wrap an already advanced session and thereby skip its input history.
/// There is no mutable session, raw checkpoint, unreviewed-logit or reset accessor.
/// A hold is terminal for this object. Numerical errors after admission also latch.
///
/// The trusted caller can independently retain a DecoderModel clone and execute
/// it elsewhere. This API does not isolate that caller or mediate external effects.
/// An alarm is a probe result, not proof of intent; a quiet review is not safety.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::monitor::decoder::MonitoredDecoder;
/// fn bypass(run: &mut MonitoredDecoder) { run.session_mut(); }
/// ```
pub struct MonitoredDecoder {
    session: DecoderSession,
    monitors: BTreeMap<u64, RefinementMonitor>,
    generation: u64,
    stream: u64,
    budget: RefinementBudget,
    work: MonitoringWork,
    status: MonitoringStatus,
    last_review: Option<Rc<DecoderReview>>,
    observation: observation::ObservationWriter,
}
impl fmt::Debug for MonitoredDecoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MonitoredDecoder").field("generation", &self.generation)
            .field("stream", &self.stream).field("position", &self.position())
            .field("status", &self.status).field("work", &self.work).finish_non_exhaustive()
    }
}

impl MonitoredDecoder {
    /// Validate the ENTIRE monitor roster before creating any numerical session.
    /// The shared allowance cannot enlarge an individual monitor's fixed budget.
    pub fn new(
        model: DecoderModel, stream: u64, generation: u64,
        monitors: BTreeMap<u64, RefinementMonitor>, budget: RefinementBudget,
    ) -> Result<Self, Error> {
        if generation == 0 || stream == 0 { return Err(Error::InvalidInput); }
        if monitors.len() != model.profile().shape().layers { return Err(Error::Binding); }
        for layer in 1..=model.profile().shape().layers as u64 {
            let contract = model.residual_contract(layer)?;
            let monitor = monitors.get(&layer).ok_or(Error::Binding)?;
            if monitor.profile() != contract.profile() || monitor.dimensions() != contract.dimensions() {
                return Err(Error::Binding);
            }
        }
        let session = model.session(stream)?;
        let observation = observation::ObservationWriter::new(model.profile().clone(), generation, stream);
        Ok(Self { session, monitors, generation, stream, budget,
            work: MonitoringWork::default(), status: MonitoringStatus::Ready, last_review: None, observation })
    }

    /// Read-only live evidence for a trusted controller, never a permit.
    pub fn observation(&self) -> observation::DecoderObservation { self.observation.observe() }

    pub fn profile(&self) -> &DecoderProfile { self.session.model().profile() }
    pub fn position(&self) -> u64 { self.session.position() }
    pub fn status(&self) -> MonitoringStatus { self.status }
    pub fn monitoring_work(&self) -> MonitoringWork { self.work }
    pub fn decoder_work(&self) -> DecoderWork { self.session.work() }
    pub fn last_review(&self) -> Option<&DecoderReview> { self.last_review.as_deref() }
    pub fn remaining_budget(&self) -> RefinementBudget {
        RefinementBudget { encoded_bytes: self.budget.encoded_bytes - self.work.encoded_bytes,
            probe_coordinates: self.budget.probe_coordinates - self.work.probe_coordinates }
    }

    /// Complete numerical admission without advancing state. This lets a caller
    /// preflight a prefix plus bounded continuation before computing its first token.
    pub fn estimate(&self, tokens: usize) -> Result<DecoderWork, Error> {
        self.session.model().estimate(self.position() as usize, tokens)
    }

    /// Select only from logits whose preceding residual review was complete and
    /// quiet. The selected token is not returned if its own subsequent review holds.
    pub fn advance_greedy(&mut self, expected_position: u64, budget: DecoderBudget) -> Result<MonitoredStep, Error> {
        self.check_position(expected_position)?;
        let token = self.session.greedy_token()?;
        self.advance(expected_position, token, budget)
    }

    /// Stale position, invalid token and insufficient numerical budget refuse
    /// without work. Once computation starts, any error leaves a failed latch.
    /// A held token may have updated the private KV cache; it is never rolled back
    /// or refunded as if inference had not occurred. No further token can advance.
    pub fn advance(&mut self, expected_position: u64, token: u32, budget: DecoderBudget) -> Result<MonitoredStep, Error> {
        self.check_position(expected_position)?;
        if token as usize >= self.profile().shape().vocabulary { return Err(Error::InvalidInput); }
        let products = self.estimate(1)?.scalar_products()?;
        if budget.scalar_products > MAX_DECODER_PRODUCTS || products > budget.scalar_products {
            return Err(Error::Limit);
        }
        let mut layers = Vec::new();
        layers.try_reserve_exact(self.monitors.len()).map_err(|_| Error::Limit)?;
        self.observation.begin(expected_position)?;
        // Poison first: a caught unwind cannot expose unreviewed decoder logits
        // or let a partially reviewed session continue. No unwind catch is added.
        self.status = MonitoringStatus::Failed(Error::Incomplete);
        self.last_review = None;
        let result = self.execute(expected_position, token, budget, layers);
        if let Err(error) = &result {
            self.status = MonitoringStatus::Failed(*error);
            self.observation.fail();
        }
        result
    }

    fn check_position(&self, position: u64) -> Result<(), Error> {
        if self.status != MonitoringStatus::Ready { return Err(Error::WrongState); }
        if position != self.position() { return Err(Error::Stale); }
        Ok(())
    }

    fn execute(
        &mut self, position: u64, token: u32, budget: DecoderBudget, mut layers: Vec<LayerReview>,
    ) -> Result<MonitoredStep, Error> {
        let step = self.session.advance(position, token, budget)?;
        // Preflight every actual frame before the first monitor can run.
        if step.position != position || step.token != token || step.layers.len() != self.monitors.len() {
            return Err(Error::Binding);
        }
        for (index, observation) in step.layers.iter().enumerate() {
            if observation.layer != index as u64 + 1 { return Err(Error::Binding); }
            let monitor = self.monitors.get(&observation.layer).ok_or(Error::Binding)?;
            let source = observation.residual.source();
            let frame = source.identity();
            if frame.profile != monitor.profile() || source.dimensions() != monitor.dimensions()
                || frame.stream != self.stream || frame.position != position
                || frame.sequence != position.checked_add(1).ok_or(Error::Overflow)?
            { return Err(Error::Binding); }
        }
        let mut outcome = MonitorOutcome::NoAlarm;
        for observation in &step.layers {
            let monitor = &self.monitors[&observation.layer];
            let report = monitor.analyze_with_budget(observation.residual.source(), self.remaining_budget())?;
            let next = MonitoringWork {
                frame_reviews: self.work.frame_reviews.checked_add(1).ok_or(Error::Overflow)?,
                encoded_bytes: self.work.encoded_bytes.checked_add(report.encoded_bytes()).ok_or(Error::Overflow)?,
                probe_coordinates: self.work.probe_coordinates.checked_add(report.probe_coordinates()).ok_or(Error::Overflow)?,
                codec_coordinates: self.work.codec_coordinates.checked_add(report.codec_coordinates()).ok_or(Error::Overflow)?,
            };
            if next.encoded_bytes > self.budget.encoded_bytes || next.probe_coordinates > self.budget.probe_coordinates {
                return Err(Error::Binding);
            }
            self.work = next;
            outcome = report.outcome();
            layers.push(LayerReview { layer: observation.layer, report });
            if outcome != MonitorOutcome::NoAlarm { break; }
        }
        let review = Rc::new(DecoderReview { generation: self.generation, stream: self.stream,
            position, required_layers: self.monitors.len(), outcome, layers });
        self.observation.publish(token, Rc::clone(&review))?;
        self.last_review = Some(Rc::clone(&review));
        if outcome == MonitorOutcome::NoAlarm && review.unreviewed_layers() == 0 {
            self.status = MonitoringStatus::Ready;
            Ok(MonitoredStep::Released(ReviewedStep { step, review }))
        } else {
            self.status = MonitoringStatus::Held;
            Ok(MonitoredStep::Held(review))
        }
    }
}
