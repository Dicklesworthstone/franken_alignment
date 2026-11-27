//! Publish the original evaluated, guarded and learned configuration together.
use super::super::super::{
    FileCredentialRegistration, FileGuardSet, bootstrap::PreparedGuardedBootstrap,
};
use super::{
    EvaluationProtocol, FileEvaluatedOversightRoles, FileLearnedConfig, FileOversight,
    FileOversightProfile, JournalError, check_profile,
};
use crate::Error;
use crate::action::consequence::delivery::persistent::RecoveryReserve;
use crate::action::consequence::delivery::persistent::observed::credibility::andon::FileAndonConfig;
use crate::action::consequence::oversight::joint_credibility::JointPromotionPolicy;
use std::path::Path;

impl FileOversight {
    /// Create one original learned owner with the exact joint promotion policy,
    /// independent evaluation protocol, guards and native Andon accounting in
    /// its first acknowledged image. Configuration and terminal event-capacity
    /// checks precede storage creation; canonical byte admission and the sole
    /// acknowledgment precede role release.
    ///
    /// Joint promotion replays this owner's original independently labelled
    /// cases. It supplies neither labels nor effect authority. Recovery must
    /// independently retain this exact joint policy through `require_joint`.
    /// Recipe-owned held-out activation and pre-output prediction use distinct
    /// role-preserving lanes and cannot be substituted into live evaluation.
    pub fn create_joint_andon_guarded_with_learned_generation_reserved(
        directory: impl AsRef<Path>,
        profile: FileOversightProfile,
        guards: &FileGuardSet,
        registration: Option<FileCredentialRegistration<'_>>,
        protocol: EvaluationProtocol,
        joint: JointPromotionPolicy,
        accounting: &FileAndonConfig,
        config: FileLearnedConfig,
        reserve: RecoveryReserve,
    ) -> Result<(Self, FileEvaluatedOversightRoles), JournalError> {
        if config.required_held_out_joint().is_some()
            || config.required_pre_output_forecast().is_some()
            || config.required_owned_pre_output_forecast().is_some()
        {
            return Err(Error::Binding.into());
        }
        let prepared = super::super::super::learned::prepare_reserved(
            profile,
            guards,
            registration,
            config,
            reserve,
        )?
        .jointly_evaluated(protocol.clone(), joint)?
        .with_andon(accounting)?
        .checked_evaluated(guards, &protocol)?
        .checked_event_capacity()?;
        let (host, oversight) = prepared.create(directory.as_ref())?;
        let roles = FileEvaluatedOversightRoles::provision(&host, oversight);
        Ok((host, roles))
    }

    /// Create one original learned, evaluated and Andon-accounted owner with
    /// terminal recovery capacity in its FIRST acknowledged image. Complete
    /// recipe-owned source and guard checks precede storage creation. Numerical
    /// work, independent labels and human approval still require their original
    /// separately held interfaces; none is fabricated during bootstrap.
    ///
    /// The evaluator cannot serve as a predictor. Recipes requiring a raw or
    /// owned pre-output forecast must use their distinct role-preserving lane.
    /// Existing stores cannot be replaced or retrofitted through this method.
    pub fn create_andon_guarded_with_learned_generation_reserved(
        directory: impl AsRef<Path>,
        profile: FileOversightProfile,
        guards: &FileGuardSet,
        registration: Option<FileCredentialRegistration<'_>>,
        protocol: EvaluationProtocol,
        accounting: &FileAndonConfig,
        config: FileLearnedConfig,
        reserve: RecoveryReserve,
    ) -> Result<(Self, FileEvaluatedOversightRoles), JournalError> {
        if config.required_pre_output_forecast().is_some()
            || config.required_owned_pre_output_forecast().is_some()
        {
            return Err(Error::Binding.into());
        }
        let prepared = super::super::super::learned::prepare_reserved(
            profile,
            guards,
            registration,
            config,
            reserve,
        )?
        .evaluated(protocol.clone())?
        .with_andon(accounting)?
        .checked_evaluated(guards, &protocol)?
        .checked_event_capacity()?;
        let (host, oversight) = prepared.create(directory.as_ref())?;
        let roles = FileEvaluatedOversightRoles::provision(&host, oversight);
        Ok((host, roles))
    }

    /// Install the exact learned recipe, independent evaluation protocol and
    /// every declared guard in the first canonical image. The original prepared
    /// bootstrap validates native transitions before storage is created, and its
    /// sole publication must acknowledge before any owner or role is returned.
    /// No token, fresh observation, accepted identity or effect key is created.
    ///
    /// A recipe-owned policy-only source must match the separately declared
    /// source guard exactly. Only the original learned enable installs it, and
    /// the complete resulting guard inventory is checked before storage exists.
    /// Recipes requiring a separate predictor belong to a predictive role profile.
    pub fn create_evaluated_guarded_with_learned_generation(
        directory: impl AsRef<Path>,
        profile: FileOversightProfile,
        guards: &FileGuardSet,
        registration: Option<FileCredentialRegistration<'_>>,
        protocol: EvaluationProtocol,
        config: FileLearnedConfig,
    ) -> Result<(Self, FileEvaluatedOversightRoles), JournalError> {
        check_profile(guards)?;
        if config.required_pre_output_forecast().is_some()
            || config.required_owned_pre_output_forecast().is_some()
        {
            return Err(Error::Binding.into());
        }
        let mut prepared_guards = guards.clone();
        if let Some(policy) = config.required_policy_source() {
            if guards.source != Some(policy) {
                return Err(Error::Binding.into());
            }
            // The exact learned Enable owns this native policy-only source.
            // Do not install a second ordinary source before that transition.
            prepared_guards.source = None;
        }
        let prepared = PreparedGuardedBootstrap::prepare(profile, &prepared_guards, registration)?
            .learned(config)?
            .evaluated(protocol.clone())?
            .checked_evaluated(guards, &protocol)?;
        let (host, oversight) = prepared.create(directory.as_ref())?;
        let roles = FileEvaluatedOversightRoles::provision(&host, oversight);
        Ok((host, roles))
    }
}
