//! Crate-private state capture for the owning controller. No effect authority.

use super::MonitoredSampledDecoder;
use super::super::MonitoringStatus;
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
        self.monitored.observation.fail();
    }
}
