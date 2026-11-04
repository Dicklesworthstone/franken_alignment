//! Freeze the original nonsecret credential policy, never a secret or permit.
use super::{FileLearnedConfig, Writer, DOMAIN, MAX_CONFIG_BYTES};
use crate::action::consequence::delivery::persistent::observed::credential::FileCredentialPolicy;
use crate::Error;

impl FileLearnedConfig {
    /// Require this exact ORIGINAL credential guard before learned Enable. The
    /// guarded constructor must independently resolve its perimeter registration;
    /// an asserted policy here cannot enable a guard or supply either credential.
    /// Every learned recovery, historical receipt and review-audit reader binds
    /// these fields through the complete independently supplied numerical recipe.
    ///
    /// Rotation and revocation remain the original separate, monotone lifecycle.
    /// Their current generation and revoked state belong to guarded recovery
    /// requirements, not this immutable policy. No secret, live credential permit
    /// or provider-equality assertion enters the recipe or its journal encoding.
    /// Existing unselected recipes keep their bytes and behavior. Other wrappers
    /// retain this selection; wrapper order remains part of recipe identity.
    pub fn with_required_credential_policy(mut self, policy: FileCredentialPolicy)
        -> Result<Self, Error>
    {
        if self.credential.is_some() { return Err(Error::Duplicate); }
        policy.check()?;
        let mut writer = Writer::new(MAX_CONFIG_BYTES);
        writer.raw(DOMAIN)?; writer.u64(0)?; writer.raw(b"FALCRD\0\x01")?;
        writer.blob(&self.bytes)?;
        writer.blob(policy.family.as_bytes())?;
        writer.blob(policy.route.as_bytes())?;
        writer.blob(policy.credential.as_bytes())?;
        writer.u64(policy.profile_generation)?;
        self.bytes = writer.finish().into();
        self.credential = Some(policy);
        Ok(self)
    }

    /// Exact nonsecret requirement, not evidence of credential possession or
    /// permission to publish. Current generation/revocation are independently
    /// supplied and checked by the original guarded recovery path.
    pub fn required_credential_policy(&self) -> Option<&FileCredentialPolicy> {
        self.credential.as_ref()
    }
}
