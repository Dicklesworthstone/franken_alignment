//! Bind the original terminal-stop policy without changing any prior encoding.
use super::{Error, FileConsistencyConfig, Reader, Writer, MAX_CONFIG_BYTES};
use crate::action::consequence::oversight::consistency::ConsistencyStopPolicy;

pub(super) const DOMAIN: &[u8; 8] = b"FACPRED\x04";

impl FileConsistencyConfig {
    /// Immutable bootstrap data, not a live switch or a fresh likelihood budget.
    /// The same original policy handles threshold crossings AND coverage loss.
    /// Compose in any order with hosted residuals and message-prefix semantics.
    pub fn with_terminal_stop(self, policy: ConsistencyStopPolicy) -> Result<Self, Error> {
        if self.terminal_stop_policy().is_some() { return Err(Error::Duplicate); }
        let mut writer = Writer::new(MAX_CONFIG_BYTES);
        writer.raw(DOMAIN)?; writer.blob(&self.bytes)?;
        for value in [policy.id(), policy.generation(), policy.operation()] { writer.u64(value)?; }
        Self::from_bytes(&writer.finish())
    }

    pub fn terminal_stop_policy(&self) -> Option<ConsistencyStopPolicy> {
        self.bytes.starts_with(DOMAIN).then(|| parts(&self.bytes).expect("validated stop configuration").1)
    }

    pub(super) fn without_stop(&self) -> &[u8] {
        if self.bytes.starts_with(DOMAIN) { parts(&self.bytes).expect("validated stop configuration").0 }
        else { &self.bytes }
    }
}

// Version four wraps exactly ONE original v1/v2/v3 configuration. A nested stop
// wrapper or a stop wrapper inside a message wrapper is never canonical.
pub(super) fn parts(bytes: &[u8]) -> Result<(&[u8], ConsistencyStopPolicy), Error> {
    let mut reader = Reader::new(bytes);
    if reader.take(8)? != DOMAIN { return Err(Error::Binding); }
    let inner = reader.blob(MAX_CONFIG_BYTES)?;
    if !inner.starts_with(super::DOMAIN) && !inner.starts_with(super::HOSTED_DOMAIN)
        && !inner.starts_with(super::MESSAGE_DOMAIN) { return Err(Error::Binding); }
    let policy = ConsistencyStopPolicy::new(reader.u64()?, reader.u64()?, reader.u64()?)?;
    reader.end()?;
    Ok((inner, policy))
}

#[cfg(test)]
mod tests;
