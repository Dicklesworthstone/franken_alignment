//! Exact bootstrap binding for bounded source-certified forecast precision.
//! Stop -> progressive -> timing -> message -> original source configuration.
use super::{Error, FileConsistencyConfig, Reader, Writer, MAX_CONFIG_BYTES};
use crate::action::consequence::activation::consistency::progressive::{
    ProgressiveForecastPolicy, MAX_PROGRESSIVE_FORECAST_BYTES,
};

pub(super) const DOMAIN: &[u8; 8] = b"FACPRED\x06";

impl FileConsistencyConfig {
    /// Freeze the initial precision, fixed refinement stride, maximum precision
    /// and cumulative encoded-byte cap. Installed once by the ORIGINAL bootstrap;
    /// runtime callers cannot widen the cap after observing an ambiguous score.
    /// Works with supplied/owned sources, stream categories, required pre-output
    /// timing and terminal stop, without changing their original enforcement.
    pub fn with_progressive_forecast(self, policy: ProgressiveForecastPolicy) -> Result<Self, Error> {
        if self.progressive_forecast_policy().is_some() { return Err(Error::Duplicate); }
        if let Some(stop) = self.terminal_stop_policy() {
            let inner = super::stopping::parts(&self.bytes)?.0;
            return Self::from_bytes(inner)?.with_progressive_forecast(policy)?.with_terminal_stop(stop);
        }
        let mut writer = Writer::new(MAX_CONFIG_BYTES);
        writer.raw(DOMAIN)?; writer.blob(&self.bytes)?;
        writer.u8(policy.initial_bits())?; writer.u8(policy.refinement_bits())?;
        writer.u8(policy.maximum_bits())?; writer.count(policy.max_encoded_bytes())?;
        Self::from_bytes(&writer.finish())
    }
    pub fn progressive_forecast_policy(&self) -> Option<ProgressiveForecastPolicy> {
        let bytes = self.without_stop();
        bytes.starts_with(DOMAIN).then(|| parts(bytes).expect("validated progressive configuration").1)
    }
    pub(super) fn without_progressive(&self) -> &[u8] {
        let bytes = self.without_stop();
        if bytes.starts_with(DOMAIN) { parts(bytes).expect("validated progressive configuration").0 }
        else { bytes }
    }
}

// Recursion cannot be smuggled through a precision wrapper. Other original
// wrappers retain their own exact shapes, and the original byte cap is unchanged.
pub(super) fn parts(bytes: &[u8]) -> Result<(&[u8], ProgressiveForecastPolicy), Error> {
    let mut reader = Reader::new(bytes);
    if reader.take(8)? != DOMAIN { return Err(Error::Binding); }
    let inner = reader.blob(MAX_CONFIG_BYTES)?;
    if !inner.starts_with(super::DOMAIN) && !inner.starts_with(super::HOSTED_DOMAIN)
        && !inner.starts_with(super::MESSAGE_DOMAIN) && !inner.starts_with(super::temporal::DOMAIN) {
        return Err(Error::Binding);
    }
    let policy = ProgressiveForecastPolicy::new(reader.u8()?, reader.u8()?, reader.u8()?,
        reader.count(MAX_PROGRESSIVE_FORECAST_BYTES)?)?;
    reader.end()?;
    Ok((inner, policy))
}

#[cfg(test)]
mod tests;
