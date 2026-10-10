//! Pin the ORIGINAL predictor and required timing in the independent recipe.
//! Recovery compares these bytes before constructing the configured runtime.
mod owned;
mod reset;
use super::{FileLearnedConfig, Writer, DOMAIN, MAX_CONFIG_BYTES};
use crate::action::consequence::delivery::persistent::observed::{FileOversight,
    FileOversightProfile, FileHumanReviewer, JournalError,
    consistency::{FileConsistencyConfig, FileConsistencyObserver}};
use crate::Error;
use std::path::Path;
use std::rc::Rc;

impl FileLearnedConfig {
    /// Require the exact original pre-output predictor alongside this learned
    /// text recipe and its mandatory sidecar. No predictor is inferred from the
    /// journal at recovery. Coefficients, probability table, event domain, age,
    /// lifetime budget and optional stop policy are bound by their original bytes.
    ///
    /// Call after with_required_sidecar. Raw text and stream-message domains
    /// must agree exactly. Actual model/layer/stream and authority checks remain
    /// in the original bootstrap reducer, before the first storage image exists.
    /// Other recipe wrappers retain this requirement; wrapper order is identity.
    pub fn with_required_pre_output_forecast(mut self, forecast: FileConsistencyConfig)
        -> Result<Self, Error>
    {
        if self.pre_output_forecast.is_some() || self.owned_pre_output_forecast.is_some() { return Err(Error::Duplicate); }
        if !self.sidecar_required || !self.is_text() || !forecast.requires_pre_output_forecast()
            || self.text_stream_profile() != forecast.stream_message_profile() {
            return Err(Error::Binding);
        }
        let mut writer = Writer::new(MAX_CONFIG_BYTES);
        writer.raw(DOMAIN)?; writer.u64(0)?; writer.raw(b"FALPRED\x01")?;
        writer.blob(&self.bytes)?;
        writer.blob(forecast.encoded())?;
        self.bytes = writer.finish().into();
        self.pre_output_forecast = Some(forecast);
        Ok(self)
    }

    /// Immutable required configuration, not a live observer or permission.
    pub fn required_pre_output_forecast(&self) -> Option<&FileConsistencyConfig> {
        self.pre_output_forecast.as_ref()
    }
}

impl FileOversight {
    /// Create a learned TEXT owner with the independently pinned pre-output
    /// predictor already enforced in the FIRST canonical image. The original
    /// text/stream constructors validate and persist the complete configuration;
    /// this entry point additionally returns the separate observer after success.
    /// No intermediate generic owner or live role-reissue API is exposed.
    ///
    /// The config selects raw text versus its exact cumulative stream profile,
    /// and either the pinned raw-residual or owned learned-K/V predictor.
    /// No clock, forecast, sampled output, request or effect key is invented.
    /// Keep observer and human reviewer separate from actor/helper custody.
    ///
    /// Recovery still uses open_with_learned_generation with this exact recipe.
    /// It does not return another observer or revive pending forecast coverage.
    /// Legacy pair-returning constructors also enforce a pinned requirement, but
    /// deliberately do not issue its observer; use this constructor to receive it.
    pub fn create_with_pre_output_forecast(directory: impl AsRef<Path>,
        profile: FileOversightProfile, config: FileLearnedConfig)
        -> Result<(Self, FileHumanReviewer, FileConsistencyObserver), JournalError>
    {
        if config.required_pre_output_forecast().is_none()
            && config.required_owned_pre_output_forecast().is_none() { return Err(Error::Binding.into()); }
        let (host, reviewer) = if config.text_stream_profile().is_some() {
            Self::create_with_learned_text_stream(directory, profile, config)?
        } else {
            Self::create_with_learned_text(directory, profile, config)?
        };
        // The original first-image replacement acknowledged BOTH requirements.
        // This marker supplies only observer custody, never the reviewer's key.
        let observer = FileConsistencyObserver { issuer: Rc::clone(&host.issuer) };
        Ok((host, reviewer, observer))
    }

    /// Install the same pinned recipe in an existing original owner after any
    /// separately governed bootstrap prerequisites have been installed. This is
    /// the FIRST numerical installation, not an observer getter or reissuer.
    /// Original enable admission, source checks and persistence run unchanged.
    /// Failure returns no observer; a duplicate install cannot recover a lost one.
    pub fn enable_learned_generation_with_pre_output_forecast(&mut self, revision: u64,
        config: FileLearnedConfig) -> Result<FileConsistencyObserver, JournalError>
    {
        if config.required_pre_output_forecast().is_none()
            && config.required_owned_pre_output_forecast().is_none() { return Err(Error::Binding.into()); }
        self.enable_learned_generation(revision, config)?;
        Ok(FileConsistencyObserver { issuer: Rc::clone(&self.issuer) })
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod lifecycle_tests;
