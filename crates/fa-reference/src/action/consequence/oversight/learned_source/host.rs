//! Private exact actor synchronization for the broker-owned learned generator.
//! No public cache importer, mutable generator or authority conversion.
use super::{LearnedAvailability, ObservedLearnedGeneration, Prefix, Shared};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::monitored::{
    LearnedGeneration, restart::GenerationKvRestartReceipt,
};
use crate::action::consequence::activation::tensor::kv::model::{
    MODEL_DESCRIPTOR_HEADER_BYTES, MODEL_LAYER_DESCRIPTOR_BYTES,
};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::SAMPLER_SNAPSHOT_BYTES;
use crate::action::consequence::gate::containment::{
    ActorState, RestartProfile, MAX_CACHE_BYTES, MAX_SAMPLER_BYTES, MAX_TOKENS,
};
use crate::Error;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

impl ObservedLearnedGeneration {
    /// Private adoption after the ORIGINAL typed checkpoint's fresh complete
    /// audit and exact restorer. The paired broker still owns the authority
    /// transition. No supplied report or arbitrary advanced run enters here.
    pub(crate) fn from_host_restart(run: LearnedGeneration, receipt: &GenerationKvRestartReceipt,
        prior: &Self) -> Result<Self, Error>
    {
        let restored = receipt.kv().restoration();
        let audit = receipt.kv().audit().ok_or(Error::Incomplete)?.monitoring();
        let count = run.accepted_tokens().len();
        let rows = count.checked_mul(prior.shared.profile.shape().layers)
            .and_then(|n| n.checked_mul(2)).ok_or(Error::Limit)?;
        if count == 0 || restored.position != count as u64 || run.position() != restored.position
            || restored.resumed_stream <= prior.shared.stream
            || run.evaluation_origin() != prior.shared.evaluation_origin
            || receipt.kv().evaluation_origin() != prior.shared.evaluation_origin
            || restored.source.profile() != run.policy().codec().profile()
            || run.policy().codec().profile() != prior.run.policy().codec().profile()
            || run.status() != receipt.status() || run.work() != receipt.historical_work()
            || run.telemetry_work() != receipt.historical_telemetry()
            || run.sampler_state().draws() != receipt.sampler_draws()
            || !audit.complete_quiet() || audit.first_position() != 0
            || audit.end_position() != restored.position || audit.planned_rows() != rows
            || audit.source().descriptor() != &restored.source
            || restored.source.layers().values().any(|layer| {
                layer.stream == 0 || layer.stream > prior.shared.stream
                    || layer.first_position != 0 || layer.first_sequence != 1 || layer.token_count != count
            }) { return Err(Error::Binding); }
        let image = run.accepted_cache_image()?;
        if image.descriptor().layers().values().any(|layer| {
            layer.stream != restored.resumed_stream || layer.first_position != 0
                || layer.first_sequence != 1 || layer.token_count != count
        }) { return Err(Error::Binding); }
        let mut tokens = Vec::new();
        tokens.try_reserve_exact(run.estimate().audited_positions).map_err(|_| Error::Limit)?;
        tokens.extend_from_slice(run.accepted_tokens());
        let shared = Rc::new(Shared { profile: prior.shared.profile.clone(),
            stream: restored.resumed_stream, evaluation_origin: prior.shared.evaluation_origin,
            generation: prior.shared.generation, availability: Cell::new(LearnedAvailability::Empty),
            prefix: RefCell::new(Prefix { tokens, audit: None }) });
        // The full-prefix restart audit names its ORIGINAL stream and stays in
        // the receipt. It is not a latest-position observation for the resumed
        // stream. A NEW original accepted token must produce that evidence.
        let source = Self { run, shared, text: prior.text.clone() };
        source.check_host_horizon()?;
        Ok(source)
    }

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

    // Read-only original state for the durable owner's exact comparison witness.
    // Never returned to an external caller, installed into a run, or made mutable.
    pub(crate) fn original_generation(&self)
        -> &crate::action::consequence::activation::tensor::kv::decoder::sampling::monitored::LearnedGeneration
    {
        &self.run
    }

    /// The original observation may become Ready before actor serialization.
    /// Keep an unwind guard across the whole private, callback-free composition:
    /// failed synchronization must not leave that numerical observation eligible.
    pub(crate) fn guard_host_sync(&self) -> HostSyncGuard {
        HostSyncGuard { shared: Rc::clone(&self.shared), confirmed: false }
    }

    /// Withdraw the old source before the first admitted restart work. A failed
    /// audit, authority refusal, or unwind cannot revive its previous evidence.
    pub(crate) fn guard_host_restart(&self) -> HostSyncGuard {
        self.shared.availability.set(LearnedAvailability::InProgress);
        self.guard_host_sync()
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
