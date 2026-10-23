//! First-image learned prediction with the complete separate guard-role bundle.
use super::{EvaluationProtocol, FileConsistencyConfig, FileCredentialRegistration,
    FileGuardSet, FileOversight, FileOversightProfile, FilePredictiveRoles, JournalError, storage};
use super::super::{learned, mediated::predictive::FileMediatedPredictor};
use super::super::super::{consistency::learned::FileLearnedConsistencyConfig,
    decoder::learned::FileLearnedConfig};
use crate::action::consequence::delivery::persistent::RecoveryReserve;
use std::path::Path;

impl FileOversight {
    /// Atomically install the exact recipe-owned raw predictor, all original
    /// guards and terminal reserve before creating storage or exposing roles.
    /// The optional evaluator remains independent; no source observation,
    /// identity match, forecast, committee vote or human key is supplied here.
    pub fn create_predictive_guarded_with_learned_generation_reserved(
        directory: impl AsRef<Path>, profile: FileOversightProfile,
        guards: &FileGuardSet, registration: Option<FileCredentialRegistration<'_>>,
        prediction: FileConsistencyConfig, evaluation: Option<EvaluationProtocol>,
        config: FileLearnedConfig, reserve: RecoveryReserve,
    ) -> Result<(Self, FilePredictiveRoles), JournalError> {
        create(directory.as_ref(), profile, guards, registration,
            FileMediatedPredictor::Raw(prediction), evaluation, config, reserve)
    }

    /// The owned K/V counterpart retains exact source mode, representation and
    /// lifetime work ceilings, and returns both prediction and identity custody.
    /// Supplied captures and standalone predictors cannot replace the required
    /// configuration installed by the original learned Enable.
    pub fn create_predictive_guarded_with_owned_learned_consistency_reserved(
        directory: impl AsRef<Path>, profile: FileOversightProfile,
        guards: &FileGuardSet, registration: Option<FileCredentialRegistration<'_>>,
        prediction: FileLearnedConsistencyConfig, evaluation: Option<EvaluationProtocol>,
        config: FileLearnedConfig, reserve: RecoveryReserve,
    ) -> Result<(Self, FilePredictiveRoles), JournalError> {
        create(directory.as_ref(), profile, guards, registration,
            FileMediatedPredictor::Owned(prediction), evaluation, config, reserve)
    }
}

fn create(directory: &Path, profile: FileOversightProfile, guards: &FileGuardSet,
    registration: Option<FileCredentialRegistration<'_>>, prediction: FileMediatedPredictor,
    evaluation: Option<EvaluationProtocol>, config: FileLearnedConfig, reserve: RecoveryReserve)
    -> Result<(FileOversight, FilePredictiveRoles), JournalError>
{
    prediction.check_recipe(&config)?;
    let mut prepared = learned::prepare_reserved(profile, guards, registration, config, reserve)?;
    if let Some(protocol) = &evaluation { prepared = prepared.evaluated(protocol.clone())?; }
    let prepared = prepared.checked_predictive(guards, evaluation.as_ref(), &prediction)?;
    let (host, oversight) = prepared.publish(storage::Store::create(directory)?)?;
    let roles = FilePredictiveRoles::provision(&host, oversight, evaluation.is_some());
    Ok((host, roles))
}
