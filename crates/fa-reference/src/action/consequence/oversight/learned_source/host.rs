//! Private exact actor synchronization for the broker-owned learned generator.
//! No public cache importer, mutable generator or authority conversion.
use super::{LearnedAvailability, ObservedLearnedGeneration, Shared};
use crate::action::consequence::activation::tensor::kv::model::{
    MODEL_DESCRIPTOR_HEADER_BYTES, MODEL_LAYER_DESCRIPTOR_BYTES,
};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::SAMPLER_SNAPSHOT_BYTES;
use crate::action::consequence::gate::containment::{
    ActorState, RestartProfile, MAX_CACHE_BYTES, MAX_SAMPLER_BYTES, MAX_TOKENS,
};
use crate::Error;
use std::rc::Rc;

impl ObservedLearnedGeneration {
    /// Check the complete originally declared horizon before ownership changes.
    /// Original cache values are binary32. This is the exact canonical payload
    /// length, not peak memory: the numerical cache and actor copy both exist.
    pub(crate) fn check_host_horizon(&self) -> Result<(), Error> {
        let count = self.run.estimate().audited_positions;
        self.check_host_size(count)
    }
    fn check_host_size(&self, count: usize) -> Result<(), Error> {
        let profile = &self.shared.profile;
        if count > MAX_TOKENS || count > profile.shape().context { return Err(Error::Limit); }
        let values = count.checked_mul(self.run.policy().codec().profile().values_per_token())
            .ok_or(Error::Limit)?;
        let headers = profile.shape().layers.checked_mul(MODEL_LAYER_DESCRIPTOR_BYTES)
            .and_then(|n| n.checked_add(MODEL_DESCRIPTOR_HEADER_BYTES)).ok_or(Error::Limit)?;
        let bytes = values.checked_mul(4).and_then(|n| n.checked_add(headers)).ok_or(Error::Limit)?;
        if bytes > MAX_CACHE_BYTES || SAMPLER_SNAPSHOT_BYTES > MAX_SAMPLER_BYTES { return Err(Error::Limit); }
        Ok(())
    }
    pub(crate) fn capture_host_actor(&self, profile: RestartProfile) -> Result<ActorState, Error> {
        self.check_host_size(self.run.accepted_tokens().len())?;
        let cache = self.run.accepted_cache_image()?;
        if cache.descriptor().image_len()? > MAX_CACHE_BYTES { return Err(Error::Limit); }
        let cache = cache.encode()?;
        let sampler = self.run.sampler_state().encode().to_vec();
        let mut tokens = Vec::new();
        tokens.try_reserve_exact(self.run.accepted_tokens().len()).map_err(|_| Error::Limit)?;
        tokens.extend_from_slice(self.run.accepted_tokens());
        ActorState::new(profile, tokens, cache, sampler, self.run.position())
    }
    pub(crate) fn sampled_draws(&self) -> u64 { self.run.sampler_state().draws() }

    /// The original observation may become Ready before actor serialization.
    /// Keep an unwind guard across the whole private, callback-free composition:
    /// failed synchronization must not leave that numerical observation eligible.
    pub(crate) fn guard_host_sync(&self) -> HostSyncGuard {
        HostSyncGuard { shared: Rc::clone(&self.shared), confirmed: false }
    }
}

pub(crate) struct HostSyncGuard { shared: Rc<Shared>, confirmed: bool }
impl HostSyncGuard {
    pub(crate) fn confirm(mut self) { self.confirmed = true; }
}
impl Drop for HostSyncGuard {
    fn drop(&mut self) {
        if !self.confirmed { self.shared.availability.set(LearnedAvailability::Failed); }
    }
}
