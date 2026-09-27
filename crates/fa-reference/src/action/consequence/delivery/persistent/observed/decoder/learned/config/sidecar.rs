//! Bind the original mandatory sidecar gate to independently retained bootstrap.
use super::{FileLearnedConfig, Writer, DOMAIN, MAX_CONFIG_BYTES};
use crate::Error;

impl FileLearnedConfig {
    /// Require the original source-bound sidecar path at every permitting gate.
    /// This consumes an uninstalled recipe, not a live owner. No disable or late
    /// reconfiguration operation is supplied. Numeric and text recipes both work.
    /// The disjoint wrapper retains EVERY byte of the original recipe; all
    /// existing recovery APIs must match it before replay or role provisioning.
    pub fn with_required_sidecar(mut self) -> Result<Self, Error> {
        if self.sidecar_required { return Err(Error::Duplicate); }
        let mut writer = Writer::new(MAX_CONFIG_BYTES);
        writer.raw(DOMAIN)?;
        // Numeric recipes have a nonzero generation at this offset; the other
        // registered wrapper uses FALTEXT, so neither can match this profile.
        writer.u64(0)?;
        writer.raw(b"FALSIDE\x01")?;
        writer.blob(&self.bytes)?;
        self.bytes = writer.finish().into();
        self.sidecar_required = true;
        Ok(self)
    }

    /// Frozen configuration, not evidence that a particular action was reviewed.
    pub fn requires_sidecar(&self) -> bool { self.sidecar_required }
}
