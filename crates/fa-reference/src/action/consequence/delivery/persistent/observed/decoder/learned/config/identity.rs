//! Bind mandatory numerical identity provenance into the independent recipe.
use super::{FileLearnedConfig, Writer, DOMAIN, MAX_CONFIG_BYTES};
use crate::Error;

impl FileLearnedConfig {
    /// Require original-model computation and comparison-witness verification
    /// for identity measurements. The existing passport/policy guard must be
    /// installed before this recipe, with no prior identity challenges.
    ///
    /// The original identity gate still owns matching, expiry, installation and
    /// containment. This option removes the manual-frame fallback; it does not
    /// make manifest commitments authentic or fingerprints universally decisive.
    /// Existing optional recipes keep their exact bytes. Text, sidecar and stop
    /// settings are retained; wrapper order remains part of recipe identity.
    ///
    /// ```compile_fail,E0599
    /// use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
    /// fn downgrade(owner: &mut FileOversight) { owner.disable_computed_identity(); }
    /// ```
    pub fn with_required_computed_identity(mut self) -> Result<Self, Error> {
        if self.computed_identity_required { return Err(Error::Duplicate); }
        let mut writer = Writer::new(MAX_CONFIG_BYTES);
        writer.raw(DOMAIN)?;
        writer.u64(0)?;
        writer.raw(b"FALCID\0\x01")?;
        writer.blob(&self.bytes)?;
        self.bytes = writer.finish().into();
        self.computed_identity_required = true;
        Ok(self)
    }

    /// Frozen admission policy, not an assertion that an identity check passed.
    pub fn requires_computed_identity(&self) -> bool { self.computed_identity_required }
}
