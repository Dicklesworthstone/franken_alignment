//! Native text provenance and independent evaluation in one first image.
//! Configuration is not evidence, a label, an inference result or permission.
use super::{
    ByteBpe, CredibilityEvent, Event, FileDecoderConfig, FileOversight, FileOversightProfile,
    GeneratedSourceProfile, InitialStore, JournalError, Path, Prepared,
};
use crate::Error;
use crate::action::consequence::delivery::persistent::observed::{
    credibility::andon::FileAndonConfig,
    guarded::{FileCampaignRequirement, evaluation::FileEvaluatedOversightRoles},
};
use crate::action::consequence::delivery::persistent::recovery_capacity::{Admission, Class};
use crate::action::consequence::oversight::{
    credibility::EvaluationProtocol, joint_credibility::JointPromotionPolicy,
};

/// Independently chosen bootstrap inputs for the original native evaluator.
/// The protocol is mandatory; campaign governance, joint promotion and Andon
/// accounting are explicit opt-ins. Absence never means "accept or reconstruct a policy from storage".
/// These inputs carry no assessment, accumulated credit or role authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileGeneratedEvaluationConfig {
    protocol: EvaluationProtocol,
    joint: Option<JointPromotionPolicy>,
    andon: Option<FileAndonConfig>,
    campaigns: Option<FileCampaignRequirement>,
}

impl FileGeneratedEvaluationConfig {
    pub fn new(protocol: EvaluationProtocol) -> Self {
        Self {
            protocol,
            joint: None,
            andon: None,
            campaigns: None,
        }
    }

    /// Select the existing native joint gate without replacing the marginal
    /// protocol or its whole-corpus qualification requirements.
    pub fn with_joint(mut self, policy: JointPromotionPolicy) -> Result<Self, Error> {
        if self.joint.is_some() {
            return Err(Error::Duplicate);
        }
        self.joint = Some(policy);
        Ok(self)
    }

    /// Select new native accounting. The original reducer validates identity,
    /// protocol and windows before any storage or role exists. Existing history
    /// must use the separately guarded recovery path; creation never imports it.
    pub fn with_andon(mut self, config: FileAndonConfig) -> Result<Self, Error> {
        if self.andon.is_some() {
            return Err(Error::Duplicate);
        }
        self.andon = Some(config);
        Ok(self)
    }

    /// Select the original independent policy-campaign guard in the same first
    /// image as native provenance and evaluation. Its complete bounds are
    /// validated by the original campaign reducer before storage exists. This
    /// does not approve a campaign, label a case or grant an effect permit.
    pub fn with_campaigns(mut self, requirement: FileCampaignRequirement) -> Result<Self, Error> {
        if self.campaigns.is_some() {
            return Err(Error::Duplicate);
        }
        self.campaigns = Some(requirement);
        Ok(self)
    }

    pub fn campaign_requirement(&self) -> Option<FileCampaignRequirement> {
        self.campaigns
    }

    pub fn protocol(&self) -> &EvaluationProtocol {
        &self.protocol
    }

    pub fn joint_policy(&self) -> Option<JointPromotionPolicy> {
        self.joint
    }

    pub fn andon(&self) -> Option<&FileAndonConfig> {
        self.andon.as_ref()
    }
}

// This private prepared value can only arise after native evaluation and all
// selected accounting transitions have passed. A source-only preparation cannot
// call its publication method and acquire an evaluator role.
struct PreparedEvaluation {
    prepared: Prepared,
}

impl Prepared {
    fn evaluated(
        mut self,
        config: FileGeneratedEvaluationConfig,
    ) -> Result<PreparedEvaluation, JournalError> {
        let count = self
            .events
            .len()
            .checked_add(1 + usize::from(config.andon.is_some()))
            .ok_or(Error::Overflow)?;
        if count > self.profile.delivery.limits.events {
            return Err(Error::Limit.into());
        }
        if let Some(reserve) = self.events.iter().find_map(|event| match event {
            Event::Core(super::BaseEvent::ReserveRecovery(reserve)) => Some(*reserve),
            _ => None,
        }) {
            // The original admission law validates reserve dimensions and the
            // complete event count before storage exists. Canonical encoding
            // still checks every actual byte against the final store identity.
            Admission::new(self.profile.delivery.limits).record(
                Class::Install(reserve),
                count,
                0,
            )?;
        }
        let event = Event::Credibility(match config.joint {
            Some(policy) => CredibilityEvent::EnableJoint(config.protocol.clone(), policy),
            None => CredibilityEvent::Enable(config.protocol.clone()),
        });
        self.machine.apply(&event)?;
        self.events.push(event);
        if let Some(andon) = &config.andon {
            let event = Event::Credibility(CredibilityEvent::EnableAndon(andon.clone()));
            self.machine.apply(&event)?;
            self.events.push(event);
        }
        if self.machine.credibility_contract() != Some(&config.protocol)
            || self.machine.broker.joint_credibility_policy() != config.joint
            || self.machine.andon_contract() != config.andon.as_ref()
        {
            return Err(Error::Binding.into());
        }
        Ok(PreparedEvaluation { prepared: self })
    }
}

impl PreparedEvaluation {
    fn publish_evaluated(
        self,
        destination: InitialStore<'_>,
    ) -> Result<(FileOversight, FileEvaluatedOversightRoles), JournalError> {
        let (host, human) = self.prepared.publish(destination)?;
        let roles = FileEvaluatedOversightRoles::from_published(&host, human);
        Ok((host, roles))
    }
}

impl FileOversight {
    /// Atomically create the original source-bound native text stream with its
    /// independent evaluation protocol and explicitly selected joint/Andon
    /// accounting and independent campaign governance. The first canonical
    /// image includes native provenance, decoder, tokenizer, source, publication
    /// guards, reserve, campaigns and evaluation. A selected governor is returned
    /// separately in the original oversight role bundle after acknowledgment.
    /// No source-only or unaccounted intermediate owner can escape.
    ///
    /// Native configuration, protocol, reserve dimensions and event capacity
    /// and the exact cost of the final path-bound encoding are checked before
    /// creating storage. The sole replacement must acknowledge before the owner,
    /// human reviewer or independent evaluator is returned. A failed or uncertain
    /// initial write returns no role even when its complete image reached disk.
    ///
    /// Owned-round joint promotion is compatible with this evaluation protocol.
    /// Held-out joint qualification is a separate, exclusive native lane:
    /// `GeneratedSourcePublication::Joint` is rejected, never downgraded.
    ///
    /// This performs no inference, capture, clock read, assessment or promotion.
    /// Native text admission, full-input congress, the separate human key and
    /// final publication witnesses remain mandatory. Each role belongs with its
    /// separate custodian. Recovery requires independent retained contracts and
    /// history; this entry point cannot restore old evaluator custody.
    pub fn create_generated_text_stream_from_source_evaluated(
        directory: impl AsRef<Path>,
        profile: FileOversightProfile,
        decoder: FileDecoderConfig,
        tokenizer: ByteBpe,
        selected: GeneratedSourceProfile,
        evaluation: FileGeneratedEvaluationConfig,
    ) -> Result<(Self, FileEvaluatedOversightRoles), JournalError> {
        Prepared::with_campaigns(profile, decoder, &tokenizer, selected, evaluation.campaigns)?
            .evaluated(evaluation)?
            .publish_evaluated(InitialStore::Create(directory.as_ref()))
    }
}

#[cfg(test)]
mod tests;
