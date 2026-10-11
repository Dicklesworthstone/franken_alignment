//! Exact portable comparison for an independently provisioned learned monitor.
//! Parameters are comparison bytes, never a model/tokenizer/codec importer.
use super::{KvDecoderCampaign, KvMonitorSettings, MAX_KV_MONITOR_JSON_BYTES};
use crate::action::consequence::activation::{
    monitor::decoder::sampled::generation::tokenizer::ByteBpe,
    tensor::kv::{decoder::{DecoderModel, monitoring::LearnedDecoderPolicy,
        sampling::replay::archive::{encode_monitor_binding, verify_monitor_binding}},
        model::learned::LearnedKvCodec},
};
use crate::Error;

/// The durable learned profile's current 16 MiB ceiling. Larger exact bindings
/// refuse rather than replacing complete parameters with a weaker identifier.
pub const MAX_KV_MONITOR_ARTIFACT_BYTES: usize = 16 * 1024 * 1024;
const DOMAIN: &[u8; 8] = b"FAKVMON2";
const HEADER_BYTES: usize = 32;

/// Structurally admitted, borrowed comparison data. It does not own a decoder,
/// tokenizer, codec, trained campaign, live policy or effect authority. Callers
/// retain/provision those independently and verify before constructing a run.
///
/// This is not a signature or proof of training: an adversary replacing both
/// this artifact and independently trusted assets is outside the equality claim.
#[derive(Debug)]
pub struct KvMonitorArtifact<'a> {
    monitor_json: &'a [u8],
    tokenizer: &'a [u8],
    binding: &'a [u8],
}
impl<'a> KvMonitorArtifact<'a> {
    pub fn is_encoded(bytes: &[u8]) -> bool { bytes.starts_with(DOMAIN) }

    /// Check complete framing and bounds without allocating model-sized storage
    /// or accepting saved parameters as executable objects. V1 JSON remains a
    /// separate legacy format, not an automatically upgraded artifact.
    pub fn decode(bytes: &'a [u8], max_bytes: usize) -> Result<Self, Error> {
        if max_bytes > MAX_KV_MONITOR_ARTIFACT_BYTES || bytes.len() > max_bytes {
            return Err(Error::Limit);
        }
        if bytes.len() < HEADER_BYTES { return Err(Error::Incomplete); }
        if !Self::is_encoded(bytes) { return Err(Error::InvalidInput); }
        let mut sizes = [0_usize; 3];
        for (index, size) in sizes.iter_mut().enumerate() {
            let start = 8 + index * 8;
            *size = usize::try_from(u64::from_be_bytes(bytes[start..start + 8]
                .try_into().map_err(|_| Error::Incomplete)?)).map_err(|_| Error::Limit)?;
            if *size == 0 { return Err(Error::InvalidInput); }
            if *size > max_bytes { return Err(Error::Limit); }
        }
        if sizes[0] > MAX_KV_MONITOR_JSON_BYTES { return Err(Error::Limit); }
        let json_end = HEADER_BYTES.checked_add(sizes[0]).ok_or(Error::Limit)?;
        let tokenizer_end = json_end.checked_add(sizes[1]).ok_or(Error::Limit)?;
        let end = tokenizer_end.checked_add(sizes[2]).ok_or(Error::Limit)?;
        if end != bytes.len() { return Err(Error::Binding); }
        Ok(Self { monitor_json: &bytes[HEADER_BYTES..json_end],
            tokenizer: &bytes[json_end..tokenizer_end], binding: &bytes[tokenizer_end..] })
    }

    /// Unchanged v1 provisioning schema; its coefficients/settings are not an
    /// independent authentication of the training or calibration campaign.
    pub fn monitor_json(&self) -> &'a [u8] { self.monitor_json }

    /// Compare the actual independently supplied assets and policy, including
    /// every unused weight, exact tokenizer semantics, codec basis/provenance,
    /// probe coefficient/threshold and operational cap. No token executes and
    /// no journal, sampler, cache, budget or permission is installed or changed.
    pub fn verify(&self, model: &DecoderModel, tokenizer: &ByteBpe,
        policy: &LearnedDecoderPolicy) -> Result<(), Error>
    {
        if !tokenizer.binds(model.profile()) { return Err(Error::Binding); }
        if self.monitor_json != super::export::encode_monitor_json(policy,
            MAX_KV_MONITOR_JSON_BYTES)?.as_slice() { return Err(Error::Binding); }
        if self.tokenizer != tokenizer.to_bytes()?.as_slice() { return Err(Error::Binding); }
        verify_monitor_binding(self.binding, model, policy)
    }
}

impl KvDecoderCampaign {
    /// Export a complete accepted campaign under its ORIGINAL immutable model.
    /// Tokenizer and codec are independently supplied and checked; a failed tap
    /// cannot be omitted and no replacement coefficients may be passed here.
    ///
    /// FAKVMON2 contains the unchanged monitor JSON, original canonical tokenizer
    /// bytes and the shared original replay model/policy encoding. It grants no
    /// live promotion or authentication. Retain the trusted artifact and asset
    /// provenance independently. The size limit counts emitted bytes, not RSS.
    pub fn monitor_artifact(&self, tokenizer: &ByteBpe, codec: &LearnedKvCodec,
        settings: &KvMonitorSettings, max_bytes: usize) -> Result<Vec<u8>, Error>
    {
        if max_bytes > MAX_KV_MONITOR_ARTIFACT_BYTES { return Err(Error::Limit); }
        if !tokenizer.binds(self.model().profile()) { return Err(Error::Binding); }
        let policy = self.policy(codec.clone(), settings)?;
        let monitor = self.monitor_json(codec, settings, MAX_KV_MONITOR_JSON_BYTES)?;
        let tokenizer = tokenizer.to_bytes()?;
        let prefix = HEADER_BYTES.checked_add(monitor.len())
            .and_then(|size| size.checked_add(tokenizer.len())).ok_or(Error::Limit)?;
        let remaining = max_bytes.checked_sub(prefix).ok_or(Error::Limit)?;
        let binding = encode_monitor_binding(self.model(), &policy, remaining)?;
        let total = prefix.checked_add(binding.len()).ok_or(Error::Limit)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(total).map_err(|_| Error::Limit)?;
        bytes.extend_from_slice(DOMAIN);
        for size in [monitor.len(), tokenizer.len(), binding.len()] {
            bytes.extend_from_slice(&u64::try_from(size).map_err(|_| Error::Limit)?.to_be_bytes());
        }
        bytes.extend_from_slice(&monitor);
        bytes.extend_from_slice(&tokenizer);
        bytes.extend_from_slice(&binding);
        Ok(bytes)
    }
}
