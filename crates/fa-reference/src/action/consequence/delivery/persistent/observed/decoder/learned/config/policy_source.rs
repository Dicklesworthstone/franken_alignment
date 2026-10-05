//! Independent bootstrap binding for the ORIGINAL leased policy-state source.
//! Learned evidence still owns helper input; policy files may contain no context.
use super::{FileLearnedConfig, Writer, DOMAIN, MAX_CONFIG_BYTES};
use super::super::super::super::source::{FileSourcePolicy, SourceEvent};
use crate::action::consequence::oversight::policy_state::PolicyStateCapture;
use crate::action::Purpose;
use crate::Error;

impl FileLearnedConfig {
    /// Require both the original learned sidecar AND the original durable policy
    /// source. Call after with_required_sidecar, before installing this recipe.
    /// Neither a live owner nor archive bytes can add, remove or weaken the mode.
    /// The source identity, event/byte bounds and read-start lease are part of the
    /// independently supplied exact recovery recipe. No new writer is exposed.
    pub fn with_required_policy_source(mut self, policy: FileSourcePolicy) -> Result<Self, Error> {
        if self.policy_source.is_some() { return Err(Error::Duplicate); }
        if !self.sidecar_required || self.text.is_none() || policy.source.scope.purpose != Purpose::Effect {
            return Err(Error::Binding);
        }
        // Use the original capture constructor's bounds, not another validator.
        // These empty validation-only roles are dropped; the actual owner gets
        // its own single writer when Machine installs the complete recipe.
        let _ = PolicyStateCapture::new(policy.source, policy.limits)?;
        let mut writer = Writer::new(MAX_CONFIG_BYTES);
        writer.raw(DOMAIN)?;
        writer.u64(0)?;
        writer.raw(b"FALPOLI\x01")?;
        writer.blob(&self.bytes)?;
        super::super::super::super::source::write(&mut writer, &SourceEvent::Enable(policy))?;
        self.bytes = writer.finish().into();
        self.policy_source = Some(policy);
        Ok(self)
    }

    /// Immutable bootstrap contract. Current writer generations and observation
    /// eligibility are in the ORIGINAL file_source_status, not in this recipe.
    pub fn required_policy_source(&self) -> Option<FileSourcePolicy> { self.policy_source }
}

impl super::super::super::super::FileOversight {
    /// True only for the independently bootstrapped learned policy-only role.
    /// file_source_required remains true too: all original source admission,
    /// interruption, replacement, recovery and publication gates still apply.
    pub fn policy_only_file_source_required(&self) -> bool {
        self.machine.policy_only_file_source()
    }
}

#[cfg(test)]
mod tests;
