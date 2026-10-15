//! Compose existing topology, source and terminal capacity in the FIRST image.
//! The operator consumer must still obtain the native independent cut check.
use super::{AuthorityGraph, EvaluationProtocol, FileConsistencyConfig, FileCredentialRegistration,
    FileGuardSet, FileMediatedRoles, FileOversight, FileOversightProfile, JournalError,
    PreparedGuardedBootstrap, storage};
use crate::action::consequence::delivery::persistent::RecoveryReserve;
use crate::action::consequence::delivery::publication_gate::PublicationLimits;
use std::path::Path;

/// Independently selected bootstrap data, not a cut certificate or effect key.
/// The plain witness gate is optional; a change-feed, joint-evaluation or snapshot
/// publication profile must use its own complete bootstrap, never drop its gates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileMediatedBootstrap {
    pub topology: AuthorityGraph,
    pub reserve: RecoveryReserve,
    pub publication: Option<PublicationLimits>,
}

impl FileOversight {
    /// Publish the ORIGINAL guarded profile, recovery reserve and uncertified
    /// topology together. No intermediate unreserved/unguarded owner escapes.
    /// All native admission runs BEFORE creating storage. The first replacement
    /// either acknowledges the complete configuration or returns no roles.
    ///
    /// Optional plain publication witnesses retain their original meaning and
    /// cannot coexist with stream bootstrap. Credential registration, source,
    /// numerical, prediction and evaluation checks remain native. This does not
    /// enable a topology watcher, authenticate the inventory, or certify a cut.
    pub fn create_mediated_guarded_reserved(
        directory: impl AsRef<Path>,
        profile: FileOversightProfile,
        guards: &FileGuardSet,
        registration: Option<FileCredentialRegistration<'_>>,
        bootstrap: FileMediatedBootstrap,
        prediction: Option<FileConsistencyConfig>,
        evaluation: Option<EvaluationProtocol>,
    ) -> Result<(Self, FileMediatedRoles), JournalError> {
        let predicted = prediction.is_some();
        let evaluated = evaluation.is_some();
        let prepared = prepare(profile, guards, registration, bootstrap, prediction, evaluation)?;
        let store = storage::Store::create(directory.as_ref())?;
        let (host, oversight) = prepared.publish(store)?;
        let roles = FileMediatedRoles::provision(&host, oversight, predicted, evaluated);
        Ok((host, roles))
    }
}

// One production preparation path, also used with the original Store fault seam.
// No public importer, user-supplied event list or replacement writer is added.
fn prepare(
    profile: FileOversightProfile,
    guards: &FileGuardSet,
    registration: Option<FileCredentialRegistration<'_>>,
    bootstrap: FileMediatedBootstrap,
    prediction: Option<FileConsistencyConfig>,
    evaluation: Option<EvaluationProtocol>,
) -> Result<PreparedGuardedBootstrap, JournalError> {
    let mut prepared = match bootstrap.publication {
        Some(publication) => PreparedGuardedBootstrap::prepare_publication_reserved(
            profile, guards, registration, bootstrap.reserve, publication)?,
        None => PreparedGuardedBootstrap::prepare_reserved(
            profile, guards, registration, bootstrap.reserve)?,
    };
    if let Some(protocol) = evaluation { prepared = prepared.evaluated(protocol)?; }
    if let Some(config) = prediction { prepared = prepared.predictive(config)?; }
    prepared.mediated(bootstrap.topology)
}

#[cfg(test)]
mod tests;
