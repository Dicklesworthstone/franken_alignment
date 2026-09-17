//! Crate-private state capture for the owning controller. No effect authority.

pub(crate) mod reset;
pub(crate) mod replay;

use super::MonitoredSampledDecoder;
use super::super::MonitoringStatus;
use crate::action::consequence::activation::{CaptureProfile, SourceFrame};
use crate::action::consequence::activation::identity::{ModelPassport, decoder::DecoderIdentityProbe};
use crate::action::consequence::activation::tensor::kv::decoder::DecoderBudget;
use crate::action::consequence::activation::tensor::kv::model::{
    MODEL_DESCRIPTOR_HEADER_BYTES, MODEL_LAYER_DESCRIPTOR_BYTES,
};
use crate::Error;

pub(crate) struct CapturedState {
    pub tokens: Vec<u32>,
    pub cache: Vec<u8>,
    pub sampler: Vec<u8>,
    pub position: u64,
}

impl MonitoredSampledDecoder {
    pub(crate) fn bind_forecast_residual(&mut self, layer: u64, profile: CaptureProfile,
        dimensions: usize, stream: u64) -> Result<(), Error>
    {
        if self.monitored.forecast_residual.is_some() { return Err(Error::Duplicate); }
        if self.position() != 0 || self.status() != MonitoringStatus::Ready { return Err(Error::WrongState); }
        let contract = self.monitored.session.model().residual_contract(layer)?;
        if contract.profile() != profile || contract.dimensions() != dimensions
            || self.monitored.stream != stream { return Err(Error::Binding); }
        self.monitored.forecast_residual = Some((layer, None));
        Ok(())
    }

    // No public source accessor: only the owning predictor can consume this.
    pub(crate) fn current_forecast_residual(&self, layer: u64) -> Result<SourceFrame, Error> {
        if self.status() != MonitoringStatus::Ready { return Err(Error::Incomplete); }
        self.observation().capture()?;
        let (registered, frame) = self.monitored.forecast_residual.as_ref().ok_or(Error::Incomplete)?;
        if *registered != layer { return Err(Error::Binding); }
        let frame = frame.as_ref().ok_or(Error::Incomplete)?;
        let id = frame.identity();
        if id.stream != self.monitored.stream || id.sequence != self.position()
            || id.position.checked_add(1) != Some(self.position()) { return Err(Error::Stale); }
        Ok(frame.clone())
    }

    /// Share only THIS numerical owner's immutable parameters. The probe gets
    /// fresh caches, not the actor's cache, tokens, logits, random state or rights.
    pub(crate) fn identity_probe(&self, passport: &ModelPassport, sequence: u64,
        budget: DecoderBudget) -> Result<DecoderIdentityProbe, Error>
    {
        DecoderIdentityProbe::new(self.monitored.session.model().clone(), passport, sequence, budget)
    }

    /// Bound the existing canonical cache encoding before copying a whole state.
    /// All scalar buffers in the original decoder are normalized binary32.
    pub(crate) fn check_host_state_size(&self, position: u64, cache_limit: usize, sampler_limit: usize) -> Result<(), Error> {
        let position = usize::try_from(position).map_err(|_| Error::Limit)?;
        let model = self.monitored.session.model();
        if position > model.profile().shape().context { return Err(Error::Limit); }
        let values = position.checked_mul(model.cache_profile().values_per_token()).ok_or(Error::Limit)?;
        let headers = model.profile().shape().layers.checked_mul(MODEL_LAYER_DESCRIPTOR_BYTES)
            .and_then(|n| n.checked_add(MODEL_DESCRIPTOR_HEADER_BYTES)).ok_or(Error::Limit)?;
        let bytes = values.checked_mul(4).and_then(|n| n.checked_add(headers)).ok_or(Error::Limit)?;
        if bytes > cache_limit || self.sampler.snapshot().encode().len() > sampler_limit { return Err(Error::Limit); }
        Ok(())
    }

    /// Only the trusted owning integration can obtain held numerical state.
    /// This is not a public checkpoint or an accessor on an actor handle.
    pub(crate) fn capture_host_state(&self, cache_limit: usize, sampler_limit: usize) -> Result<CapturedState, Error> {
        self.check_host_state_size(self.position(), cache_limit, sampler_limit)?;
        let cache = self.monitored.session.cache_image()?.encode()?;
        let sampler = self.sampler.snapshot().encode().to_vec();
        if cache.len() > cache_limit || sampler.len() > sampler_limit { return Err(Error::Limit); }
        let mut tokens = Vec::new();
        tokens.try_reserve_exact(self.monitored.session.tokens().len()).map_err(|_| Error::Limit)?;
        tokens.extend_from_slice(self.monitored.session.tokens());
        Ok(CapturedState { tokens, cache, sampler, position: self.position() })
    }

    pub(crate) fn fail_host(&mut self, error: Error) {
        self.monitored.status = MonitoringStatus::Failed(error);
        self.monitored.last_review = None;
        if let Some((_, frame)) = &mut self.monitored.forecast_residual { *frame = None; }
        self.monitored.observation.fail();
    }
}
