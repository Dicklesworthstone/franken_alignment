//! Canonical opt-in timing requirement, not another forecast or likelihood process.
use super::{Error, FileConsistencyConfig, Reader, Writer, MAX_CONFIG_BYTES};

pub(super) const DOMAIN: &[u8; 8] = b"FACPRED\x05";

impl FileConsistencyConfig {
    /// Require a request-bound prompt forecast before the first continuation
    /// attempt in a learned TEXT owner. Register the hosted residual first and
    /// install before any numerical work. Ordinary and cooperative generation,
    /// as well as semantic journal replay, then enforce the same requirement.
    ///
    /// For a matching stream profile, this forecasts the generated message;
    /// finish has its own later forecast after the original append receipt.
    /// Old configurations and the optional pre-output handle are unchanged.
    pub fn with_pre_output_forecast(self) -> Result<Self, Error> {
        if self.requires_pre_output_forecast() { return Err(Error::Duplicate); }
        if self.hosted_residual_layer().is_none() { return Err(Error::Binding); }
        if let Some(policy) = self.terminal_stop_policy() {
            let inner = super::stopping::parts(&self.bytes)?.0;
            return Self::from_bytes(inner)?.with_pre_output_forecast()?.with_terminal_stop(policy);
        }
        let mut writer = Writer::new(MAX_CONFIG_BYTES);
        writer.raw(DOMAIN)?; writer.blob(&self.bytes)?;
        Self::from_bytes(&writer.finish())
    }

    pub fn requires_pre_output_forecast(&self) -> bool {
        self.without_stop().starts_with(DOMAIN)
    }

    pub(super) fn without_temporal(&self) -> &[u8] {
        let bytes = self.without_stop();
        if bytes.starts_with(DOMAIN) { parts(bytes).expect("validated temporal configuration") }
        else { bytes }
    }
}

// One v2 hosted configuration, optionally wrapped by v3 message semantics.
// Stop remains outermost. Reject recursion, nested timing modes, supplied-frame
// profiles, suffixes and missing data before coefficients are allocated.
pub(super) fn parts(bytes: &[u8]) -> Result<&[u8], Error> {
    let mut reader = Reader::new(bytes);
    if reader.take(8)? != DOMAIN { return Err(Error::Binding); }
    let inner = reader.blob(MAX_CONFIG_BYTES)?;
    reader.end()?;
    let base = if inner.starts_with(super::MESSAGE_DOMAIN) { super::message_parts(inner)?.0 }
        else { inner };
    if !base.starts_with(super::HOSTED_DOMAIN) { return Err(Error::Binding); }
    Ok(inner)
}

#[cfg(test)]
mod tests;
