//! Forecast before proposal; count the observed action before exact admission.
//!
//! This owns one lifetime process, not a resettable score per action. Missing
//! forecasts, unresolved forecasts and a crossed process cannot grant effects.
//! Host capture, clock and the registered calibration remain trusted inputs.

use super::OversightBroker;
use crate::action::{ActionSpec, ElapsedTick, FrozenAction, MAX_PAYLOAD_BYTES};
use crate::action::consequence::activation::SourceFrame;
use crate::action::consequence::activation::consistency::{
    ErrorBudget, ForecastModel, LikelihoodEvidence, LikelihoodFactor, MAX_SAMPLES, Prediction,
};
use crate::Error;
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct ConsistencyConfig {
    pub model: ForecastModel,
    pub alpha: ErrorBudget,
    pub stream: u64,
    pub max_predictions: usize,
    pub max_prediction_age_ticks: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingForecast {
    attempt: u64,
    actor_revision: u64,
    epoch: u64,
    created_at: ElapsedTick,
    valid_until: ElapsedTick,
    prediction: Prediction,
}

/// An observed proposed category, including proposals later refused or denied.
/// This is NOT a record that the action dispatched or even entered the ledger.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsistencyObservation {
    attempt: u64,
    forecast_actor_revision: u64,
    observed_actor_revision: u64,
    epoch: u64,
    forecast_at: ElapsedTick,
    observed_at: ElapsedTick,
    prediction: Prediction,
    event: bool,
    factor: LikelihoodFactor,
    sample: usize,
    crossed: bool,
}

impl ConsistencyObservation {
    pub fn attempt(&self) -> u64 { self.attempt }
    pub fn prediction(&self) -> &Prediction { &self.prediction }
    pub fn event(&self) -> bool { self.event }
    pub fn factor(&self) -> LikelihoodFactor { self.factor }
    pub fn sample(&self) -> usize { self.sample }
    pub fn crossed(&self) -> bool { self.crossed }
    pub fn policy_epoch(&self) -> u64 { self.epoch }
    pub fn forecast_actor_revision(&self) -> u64 { self.forecast_actor_revision }
    pub fn observed_actor_revision(&self) -> u64 { self.observed_actor_revision }
    pub fn forecast_at(&self) -> ElapsedTick { self.forecast_at }
    pub fn observed_at(&self) -> ElapsedTick { self.observed_at }
}

#[derive(Debug)]
pub(super) struct ConsistencyState {
    config: ConsistencyConfig,
    evidence: LikelihoodEvidence,
    pending: Option<PendingForecast>,
    observations: BTreeMap<u64, ConsistencyObservation>,
    jobs: usize,
    last_sequence: u64,
    coverage_lost: bool,
}

impl OversightBroker {
    /// Freeze the calibration, alternative, event category and lifetime alpha
    /// before any proposal. There is no disable, re-arm or alpha-refresh API.
    pub fn enable_action_consistency(&mut self, config: ConsistencyConfig) -> Result<(), Error> {
        if self.consistency.is_some() { return Err(Error::Duplicate); }
        if !self.inputs.is_empty() || !self.started_rounds.is_empty() || self.inspect().sequence != 0 {
            return Err(Error::WrongState);
        }
        if config.stream == 0 || config.max_predictions == 0 || config.max_prediction_age_ticks == 0 {
            return Err(Error::InvalidInput);
        }
        if config.max_predictions > MAX_SAMPLES { return Err(Error::Limit); }
        if config.model.profile().tenant != self.scope.tenant
            || config.model.profile().model_generation != self.delivery.controller().actor().profile().model_generation
            || config.model.policy_generation() != self.delivery.controller().policy().generation()
        { return Err(Error::Binding); }
        let evidence = LikelihoodEvidence::new(config.alpha);
        self.consistency = Some(ConsistencyState { config, evidence, pending: None,
            observations: BTreeMap::new(), jobs: 0, last_sequence: 0, coverage_lost: false });
        Ok(())
    }

    pub fn action_consistency_required(&self) -> bool { self.consistency.is_some() }

    /// Capture the forecast before accepting any bytes of its action. At most
    /// one forecast is outstanding: callers cannot reorder outcomes, discard a
    /// low-likelihood result, or choose the best of multiple predictor samples.
    /// The metadata position is the current supplied actor prefix's last token.
    /// Real future-token exclusion still needs a qualified capture boundary.
    pub fn forecast_action(
        &mut self, attempt: u64, expected_actor_revision: u64, source: &SourceFrame,
    ) -> Result<Prediction, Error> {
        let state = self.consistency.as_ref().ok_or(Error::Incomplete)?;
        if attempt == 0 { return Err(Error::InvalidInput); }
        if state.observations.contains_key(&attempt) || self.inputs.contains_key(&attempt) {
            return Err(Error::Duplicate);
        }
        if state.pending.is_some() { return Err(Error::WrongState); }
        if state.coverage_lost || state.evidence.crossed() { return Err(Error::Incomplete); }
        if self.actor_revision() != expected_actor_revision { return Err(Error::Stale); }
        let inspection = self.inspect();
        if inspection.suspended { return Err(Error::WrongState); }
        if self.delivery.controller().policy().generation() != state.config.model.policy_generation() {
            return Err(Error::Stale);
        }
        let identity = source.identity();
        let actor = self.delivery.controller().actor();
        if identity.profile != state.config.model.profile() || identity.stream != state.config.stream
            || source.dimensions() != state.config.model.dimensions()
            || identity.position.checked_add(1) != Some(actor.next_position())
            || identity.profile.model_generation != actor.profile().model_generation
        { return Err(Error::Binding); }
        if identity.sequence <= state.last_sequence { return Err(Error::Stale); }
        let now = inspection.ledger.elapsed.ok_or(Error::Incomplete)?;
        let valid_until = ElapsedTick(now.0.checked_add(state.config.max_prediction_age_ticks).ok_or(Error::Overflow)?);
        let state = self.consistency.as_mut().expect("configured consistency lane");
        // A context-valid admitted capture failure cannot leave an older quiet
        // state usable. Keep this gap outside actor checkpoints and policy state.
        if state.jobs >= state.config.max_predictions { state.coverage_lost = true; return Err(Error::Limit); }
        state.jobs += 1;
        state.last_sequence = identity.sequence;
        let prediction = match state.config.model.predict(source) {
            Ok(prediction) => prediction,
            Err(error) => { state.coverage_lost = true; return Err(error); }
        };
        state.pending = Some(PendingForecast { attempt, actor_revision: expected_actor_revision,
            epoch: inspection.ledger.epoch, created_at: now, valid_until, prediction: prediction.clone() });
        Ok(prediction)
    }

    /// Report a real capture gap even when constructing a SourceFrame failed.
    /// It cannot be erased by actor reset, identity reuse or an ordinary recapture.
    /// This finite profile requires separately governed recovery, not re-arming.
    pub fn consistency_unavailable(&mut self) -> Result<(), Error> {
        self.consistency.as_mut().ok_or(Error::Incomplete)?.coverage_lost = true;
        Ok(())
    }

    pub fn consistency_evidence(&self) -> Result<&LikelihoodEvidence, Error> {
        Ok(&self.consistency.as_ref().ok_or(Error::Incomplete)?.evidence)
    }
    pub fn consistency_observation(&self, attempt: u64) -> Result<&ConsistencyObservation, Error> {
        self.consistency.as_ref().ok_or(Error::Incomplete)?.observations.get(&attempt).ok_or(Error::Missing)
    }
    pub fn pending_forecast(&self) -> Result<Option<u64>, Error> {
        Ok(self.consistency.as_ref().ok_or(Error::Incomplete)?.pending.as_ref().map(|p| p.attempt))
    }
    pub fn consistency_coverage_lost(&self) -> Result<bool, Error> {
        Ok(self.consistency.as_ref().ok_or(Error::Incomplete)?.coverage_lost)
    }

    /// Called by propose before the exact policy and endpoint-specific admission.
    /// Count a valid observed category even when later policy/coverage/quota checks
    /// refuse the action. This intentional evidence mutation is not rolled back
    /// by a failed proposal. A consumed forecast/attempt is never reused.
    pub(super) fn observe_predicted_action(&mut self, attempt: u64, spec: &ActionSpec) -> Result<(), Error> {
        let Some(state) = self.consistency.as_ref() else { return Ok(()); };
        if state.observations.contains_key(&attempt) { return Err(Error::Duplicate); }
        if state.coverage_lost || state.evidence.crossed() { return Err(Error::Incomplete); }
        let pending = state.pending.as_ref().ok_or(Error::Incomplete)?;
        if attempt != pending.attempt || spec.scope != self.scope { return Err(Error::Binding); }
        if !spec.required_witnesses.is_empty() { return Err(Error::InvalidInput); }
        // Bound the only remaining variable-sized field before cloning it for
        // the existing structural validator. Caller-owned bytes are not trusted.
        if spec.payload.len() > MAX_PAYLOAD_BYTES { return Err(Error::Limit); }
        FrozenAction::freeze(spec.clone())?;
        let inspection = self.inspect();
        let now = inspection.ledger.elapsed.ok_or(Error::Incomplete)?;
        let actor_revision = self.actor_revision();
        if now < pending.created_at || now >= pending.valid_until || now >= spec.deadline
            || spec.policy_epoch != pending.epoch || inspection.ledger.epoch != pending.epoch
            || actor_revision < pending.actor_revision
            || self.delivery.controller().policy().generation() != state.config.model.policy_generation()
            || self.delivery.controller().actor().next_position() <= pending.prediction.observation().frame().position
        { return Err(Error::Stale); }
        if inspection.suspended { return Err(Error::WrongState); }
        let forecast = pending.prediction.forecast();
        let event = state.config.model.event(&spec.payload);
        let state = self.consistency.as_mut().expect("configured consistency lane");
        let factor = match state.evidence.observe(forecast, event) {
            Ok(factor) => factor,
            Err(error) => { state.coverage_lost = true; return Err(error); }
        };
        let pending = state.pending.take().expect("validated pending forecast");
        state.observations.insert(attempt, ConsistencyObservation {
            attempt, forecast_actor_revision: pending.actor_revision, observed_actor_revision: actor_revision,
            epoch: pending.epoch, forecast_at: pending.created_at, observed_at: now,
            prediction: pending.prediction, event, factor, sample: state.evidence.samples(), crossed: state.evidence.crossed(),
        });
        Ok(())
    }

    pub(super) fn check_consistency(&self, attempt: u64) -> Result<(), Error> {
        let Some(state) = &self.consistency else { return Ok(()); };
        if state.coverage_lost || state.pending.is_some() || state.evidence.crossed() { return Err(Error::Incomplete); }
        let observation = state.observations.get(&attempt).ok_or(Error::Incomplete)?;
        if observation.epoch != self.inspect().ledger.epoch || observation.observed_actor_revision != self.actor_revision()
            || self.delivery.controller().policy().generation() != state.config.model.policy_generation()
        { return Err(Error::Stale); }
        Ok(())
    }
}
