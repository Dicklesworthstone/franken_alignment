//! Opt in to source successors without changing the original forecast contract.
use super::{Error, FileLearnedConfig, Writer, DOMAIN, MAX_CONFIG_BYTES};

impl FileLearnedConfig {
    /// Follow only this original learned owner's successfully restored streams.
    /// Call after pinning either required pre-output predictor. Its calibration,
    /// event domain, origin stream and lifetime ceilings remain unchanged.
    ///
    /// This is a distinct, independently retained recipe. Legacy histories keep
    /// their exact stream checks; an old acknowledged forecast refusal must not
    /// become a successful forecast merely because recovery uses newer code.
    /// No live source-rebinding or predictor-rearming operation is exposed.
    ///
    /// Original reset must return to an active prefix BEFORE prompt completion.
    /// A fresh accepted prefill token must establish current source evidence
    /// before forecasting. Pending/lost forecasts and abandoned sampled work
    /// remain unavailable; this grants no fresh work or likelihood budget.
    /// Cumulative text streams retain their separate, unsupported reset protocol.
    pub fn with_forecast_reset_successors(mut self) -> Result<Self, Error> {
        if self.forecast_reset_successors { return Err(Error::Duplicate); }
        if self.text_stream_profile().is_some()
            || (self.pre_output_forecast.is_none() && self.owned_pre_output_forecast.is_none())
        { return Err(Error::Binding); }
        let mut writer = Writer::new(MAX_CONFIG_BYTES);
        writer.raw(DOMAIN)?; writer.u64(0)?; writer.raw(b"FALFRST\x01")?;
        writer.blob(&self.bytes)?;
        self.bytes = writer.finish().into();
        self.forecast_reset_successors = true;
        Ok(self)
    }

    /// Immutable source-lineage selection, not a live role or restart permission.
    pub fn follows_forecast_reset_successors(&self) -> bool {
        self.forecast_reset_successors
    }
}
