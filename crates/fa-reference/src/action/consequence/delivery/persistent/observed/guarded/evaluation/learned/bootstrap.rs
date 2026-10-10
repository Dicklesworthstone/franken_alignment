//! Publish the original evaluated, guarded and learned configuration together.
use super::{EvaluationProtocol, FileEvaluatedOversightRoles, FileLearnedConfig,
    FileOversight, FileOversightProfile, JournalError, check_profile};
use super::super::super::{FileCredentialRegistration, FileGuardSet,
    bootstrap::PreparedGuardedBootstrap};
use super::super::super::super::storage;
use crate::Error;
use std::path::Path;

impl FileOversight {
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
        directory: impl AsRef<Path>, profile: FileOversightProfile,
        guards: &FileGuardSet, registration: Option<FileCredentialRegistration<'_>>,
        protocol: EvaluationProtocol, config: FileLearnedConfig,
    ) -> Result<(Self, FileEvaluatedOversightRoles), JournalError> {
        check_profile(guards)?;
        if config.required_pre_output_forecast().is_some()
            || config.required_owned_pre_output_forecast().is_some()
        { return Err(Error::Binding.into()); }
        let mut prepared_guards = guards.clone();
        if let Some(policy) = config.required_policy_source() {
            if guards.source != Some(policy) { return Err(Error::Binding.into()); }
            // The exact learned Enable owns this native policy-only source.
            // Do not install a second ordinary source before that transition.
            prepared_guards.source = None;
        }
        let prepared = PreparedGuardedBootstrap::prepare(profile, &prepared_guards, registration)?
            .learned(config)?.evaluated(protocol.clone())?
            .checked_evaluated(guards, &protocol)?;
        let store = storage::Store::create(directory.as_ref())?;
        let (host, oversight) = prepared.publish(store)?;
        let roles = FileEvaluatedOversightRoles::provision(&host, oversight);
        Ok((host, roles))
    }
}
