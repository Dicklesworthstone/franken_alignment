//! Pin owned-code prediction in the SAME immutable generator recipe.
//! No standalone Enable can be omitted to weaken its first canonical state.
use super::{Error, FileLearnedConfig, Writer, DOMAIN, MAX_CONFIG_BYTES};
use crate::action::consequence::delivery::persistent::observed::consistency::learned::FileLearnedConsistencyConfig;

impl FileLearnedConfig {
    /// Require the exact owned K/V pre-output predictor before the first sample.
    /// Call after installing the original text recipe and mandatory sidecar.
    /// Raw and learned pins are mutually exclusive; optional or supplied-source
    /// prediction cannot masquerade as the required owned-source contract.
    ///
    /// The original Enable reducer validates actual model/tap/dimensions/stream
    /// before canonical creation or attachment acknowledgment. Other recipe
    /// wrappers retain this field and its bytes; wrapper order remains identity.
    pub fn with_required_owned_pre_output_forecast(mut self, forecast: FileLearnedConsistencyConfig)
        -> Result<Self, Error>
    {
        if self.pre_output_forecast.is_some() || self.owned_pre_output_forecast.is_some() {
            return Err(Error::Duplicate);
        }
        if !self.sidecar_required || !self.is_text() || !forecast.uses_owned_generation()
            || !forecast.requires_pre_output_forecast()
            || self.text_stream_profile() != forecast.consistency().stream_message_profile() {
            return Err(Error::Binding);
        }
        let mut writer = Writer::new(MAX_CONFIG_BYTES);
        writer.raw(DOMAIN)?; writer.u64(0)?; writer.raw(b"FALKPRD\x01")?;
        writer.blob(&self.bytes)?;
        writer.blob(forecast.encoded())?;
        self.bytes = writer.finish().into();
        self.owned_pre_output_forecast = Some(forecast);
        Ok(self)
    }

    /// Immutable independently selected input, not an observer, forecast or key.
    pub fn required_owned_pre_output_forecast(&self) -> Option<&FileLearnedConsistencyConfig> {
        self.owned_pre_output_forecast.as_ref()
    }
}

#[cfg(test)]
mod tests;
