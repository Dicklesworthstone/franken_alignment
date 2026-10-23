//! Bind mandatory numerical identity provenance into the independent recipe.
use super::{FileLearnedConfig, Writer, DOMAIN, MAX_CONFIG_BYTES};
use crate::action::consequence::delivery::persistent::observed::{
    guarded::FileIdentityRequirement, identity::{self, IdentityEvent},
};
use crate::Error;
use std::rc::Rc;

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

    /// Require original-model identity computation AND this exact independent
    /// passport/policy registration. Every learned reader binds the full recipe,
    /// including historical audits which do not separately take a guard set.
    /// The original guard must already be installed, with no prior challenges,
    /// when the native learned Enable applies. No match, observer or authority
    /// is manufactured by this immutable registration.
    ///
    /// This selects the computed-only mode itself; call it instead of the older
    /// boolean-only modifier. Either prior selection refuses as a duplicate.
    /// Existing boolean-only recipes keep their bytes and behavior. Other recipe
    /// wrappers retain this requirement; complete wrapper order remains identity.
    pub fn with_required_computed_identity_registration(mut self, requirement: FileIdentityRequirement)
        -> Result<Self, Error>
    {
        if self.computed_identity_required { return Err(Error::Duplicate); }
        let mut writer = Writer::new(MAX_CONFIG_BYTES);
        writer.raw(DOMAIN)?; writer.u64(0)?; writer.raw(b"FALCIR\0\x01")?;
        writer.blob(&self.bytes)?;
        // Reuse the original complete, bounded registration encoding, including
        // passport stimuli/intervals, manifest commitments and policy validation.
        identity::write(&mut writer, &IdentityEvent::Enable(
            Rc::new(requirement.passport.clone()), requirement.policy))?;
        self.bytes = writer.finish().into();
        self.computed_identity_required = true;
        self.computed_identity_registration = Some(requirement);
        Ok(self)
    }

    /// Exact frozen registration, not a measurement or live eligibility claim.
    pub fn required_computed_identity_registration(&self) -> Option<&FileIdentityRequirement> {
        self.computed_identity_registration.as_ref()
    }
}
