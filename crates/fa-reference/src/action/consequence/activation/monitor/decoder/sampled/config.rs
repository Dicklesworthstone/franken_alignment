//! Explicit operator sampling configuration for monitored numerical execution.
//! Seeds are reproducible input data, never generated from time or actor requests.

use super::{MonitoredSampledDecoder, MonitoredDecoder, Sampler, SamplingStart};
use crate::action::consequence::activation::monitor::decoder::config::MonitorConfigError;
use crate::action::consequence::activation::tensor::kv::decoder::DecoderModel;
use crate::action::consequence::activation::tensor::kv::decoder::sampling::SamplingPolicy;
use crate::strict_json::{self, ErrorKind, Json, Limits};
use crate::Error;
use std::fmt;

pub const MAX_SAMPLING_CONFIG_BYTES: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SamplingConfigError {
    Syntax,
    Limit,
    Field(&'static str),
    Sampling(Error),
    Monitor(MonitorConfigError),
}
impl fmt::Display for SamplingConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for SamplingConfigError {}

/// Parsed input only. It owns no running generator and cannot reseed a session.
#[derive(Clone)]
pub struct SamplingConfig { start: SamplingStart }
impl fmt::Debug for SamplingConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SamplingConfig").field("policy", &self.start.policy)
            .field("stream", &self.start.stream).finish_non_exhaustive()
    }
}
impl SamplingConfig {
    /// All fields are mandatory. Vocabulary is also checked against the supplied
    /// model, so a configuration cannot silently change the sampling universe.
    pub fn decode(bytes: &[u8], vocabulary: usize) -> Result<Self, SamplingConfigError> {
        let parsed = strict_json::parse(bytes, Limits {
            max_bytes: MAX_SAMPLING_CONFIG_BYTES, max_depth: 2,
            max_items: 32, max_string_bytes: 64,
        }).map_err(|error| match error.kind {
            ErrorKind::SizeLimit | ErrorKind::DepthLimit | ErrorKind::ItemLimit | ErrorKind::StringLimit => SamplingConfigError::Limit,
            _ => SamplingConfigError::Syntax,
        })?;
        let root = parsed.as_object().ok_or(SamplingConfigError::Field("$"))?;
        let names = ["schema", "id", "generation", "vocabulary", "temperature", "top_k", "top_p", "stream", "seed"];
        if root.len() != names.len() || names.iter().any(|name| !root.contains_key(*name)) {
            return Err(SamplingConfigError::Field("$"));
        }
        if root["schema"].as_str() != Some("fa.decoder-sampling/1") { return Err(SamplingConfigError::Field("schema")); }
        let integer = |field: &'static str| root[field].as_u64().ok_or(SamplingConfigError::Field(field));
        let positive = |field: &'static str| integer(field).and_then(|n| {
            if n == 0 { Err(SamplingConfigError::Field(field)) } else { Ok(n) }
        });
        let count = |field: &'static str| usize::try_from(integer(field)?).map_err(|_| SamplingConfigError::Limit);
        let declared = count("vocabulary")?;
        if declared != vocabulary { return Err(SamplingConfigError::Field("vocabulary")); }
        let policy = SamplingPolicy::new(positive("id")?, positive("generation")?, declared,
            scalar(&root["temperature"], "temperature")?, count("top_k")?, scalar(&root["top_p"], "top_p")?)
            .map_err(SamplingConfigError::Sampling)?;
        Ok(Self { start: SamplingStart { policy, stream: positive("stream")?, seed: integer("seed")? } })
    }
    pub fn start(&self) -> SamplingStart { self.start.clone() }
}

impl MonitoredSampledDecoder {
    /// Validate both immutable input configurations before any token is computed.
    /// The monitor parser creates only an empty original MonitoredDecoder, not a
    /// cached or already advanced session with an unreviewed prefix.
    pub fn from_json(
        model: DecoderModel, stream: u64, monitor_bytes: &[u8], sampling_bytes: &[u8],
    ) -> Result<Self, SamplingConfigError> {
        let config = SamplingConfig::decode(sampling_bytes, model.profile().shape().vocabulary)?;
        let start = config.start;
        let sampler = Sampler::seeded(start.policy, start.stream, start.seed).map_err(SamplingConfigError::Sampling)?;
        let monitored = MonitoredDecoder::from_json(model, stream, monitor_bytes).map_err(SamplingConfigError::Monitor)?;
        Ok(Self { monitored, sampler })
    }
}

fn scalar(value: &Json, field: &'static str) -> Result<f64, SamplingConfigError> {
    let number = match value { Json::Number(number) => number.lexeme().parse::<f64>().ok(), _ => None };
    number.filter(|n| n.is_finite()).ok_or(SamplingConfigError::Field(field))
}
