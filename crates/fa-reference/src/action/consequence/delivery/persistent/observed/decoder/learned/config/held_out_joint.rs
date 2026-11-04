//! An exact held-out joint activation contract in the original learned recipe.
use super::{FileLearnedConfig, Writer, DOMAIN, MAX_CONFIG_BYTES};
use crate::action::consequence::delivery::persistent::observed::credibility::held_out_joint::HeldOutJointPolicy;
use crate::Error;

impl FileLearnedConfig {
    /// Require the ORIGINAL held-out joint comparison for every credibility
    /// activation in this learned owner. The complete policy, including resource
    /// bounds, is independently pinned with the existing numerical recipe.
    /// The single native learned Enable installs it before returning authority.
    /// Conflicting marginal, owned-round or preinstalled held-out lanes refuse;
    /// no intermediate unqualified owner or late policy upgrade is introduced.
    ///
    /// Existing configurations retain their exact bytes and behavior. This
    /// disjoint wrapper retains text, source, sidecar, identity and predictor
    /// settings; wrapper order remains part of exact recipe identity. The policy
    /// does not itself supply evidence, authenticate labels, qualify helpers or
    /// grant an effect key. Original activation and both review keys still apply.
    pub fn with_required_held_out_joint(mut self, policy: HeldOutJointPolicy) -> Result<Self, Error> {
        if self.held_out_joint.is_some() { return Err(Error::Duplicate); }
        let mut writer = Writer::new(MAX_CONFIG_BYTES);
        writer.raw(DOMAIN)?;
        writer.u64(0)?;
        writer.raw(b"FALHJP\0\x01")?;
        writer.blob(&self.bytes)?;
        for value in [policy.id(), policy.generation(), policy.minimum_safe_roots(),
            policy.minimum_violation_roots(), policy.maximum_escape_ppm(), policy.maximum_false_stop_ppm(),
            u64::try_from(policy.budget().cases).map_err(|_| Error::Limit)?,
            u64::try_from(policy.budget().member_outcomes).map_err(|_| Error::Limit)?] {
            writer.u64(value)?;
        }
        self.bytes = writer.finish().into();
        self.held_out_joint = Some(policy);
        Ok(self)
    }

    /// Frozen native activation requirement, never a qualification result.
    pub fn required_held_out_joint(&self) -> Option<HeldOutJointPolicy> { self.held_out_joint }
}
