//! Live publication admission retains native evaluation and existing liabilities.
use super::{FileAndonRecovery, FileEvaluatedOversightRoles, FileOversight,
    FileOversightProfile, JournalError, RecoveryReserve, EvaluationProtocol, storage};
use super::super::super::{FileCredentialRegistration, FileGuardSet};
use super::super::super::bootstrap::PreparedGuardedBootstrap;
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::observed::credibility::andon::FileAndonConfig;
use crate::action::consequence::delivery::persistent::requests::PendingReconciliations;
use std::path::Path;

impl FileOversight {
    /// Create a NEW evaluated/accounted domain with an explicitly selected
    /// terminal recovery reserve in its FIRST canonical image. No provisional
    /// unguarded owner escapes. The original reserve law checks placement and
    /// both event/byte limits; no existing domain can be reset or enlarged here.
    /// The legacy create_andon_guarded encoding remains unchanged.
    pub fn create_andon_guarded_reserved(directory: impl AsRef<Path>, profile: FileOversightProfile,
        guards: &FileGuardSet, registration: Option<FileCredentialRegistration<'_>>,
        protocol: EvaluationProtocol, config: &FileAndonConfig, reserve: RecoveryReserve)
        -> Result<(Self, FileEvaluatedOversightRoles), JournalError>
    {
        let prepared = PreparedGuardedBootstrap::prepare_reserved(profile, guards, registration, reserve)?
            .evaluated(protocol)?.with_andon(config)?;
        let store = storage::Store::create(directory.as_ref())?;
        let (host, oversight) = prepared.publish(store)?;
        let roles = FileEvaluatedOversightRoles::provision(&host, oversight);
        Ok((host, roles))
    }
}

impl FileAndonRecovery {
    /// Complete native replay using the ORIGINAL fixed Fence -> Time -> Sweep
    /// transaction and release fresh separately held roles only after its one
    /// canonical acknowledgment. No event is replayed twice and no effect is
    /// resent. Inspect EVERY returned reconciliation: success is not an assertion
    /// that all previous liabilities are resolved. Unknown charges stay charged.
    ///
    /// Full native window/protocol/history/floor checks precede the supplied
    /// clock. All existing capacity, canonical comparison and storage barriers
    /// remain in force. Time/Sweep cannot spend terminal-only capacity. Source
    /// and human approvals remain withdrawn; fresh time is not fresh evidence.
    pub fn finish_reconciled<F>(self, clock: F)
        -> Result<(FileOversight, FileEvaluatedOversightRoles, PendingReconciliations), JournalError>
    where F: FnOnce() -> ElapsedTick {
        let Self { inner, expected, protocol } = self;
        let recovered = inner.finish_reconciled_checked(clock,
            |run| expected.check_replayed(&run.profile, &run.machine, &run.events, &protocol))?;
        let roles = FileEvaluatedOversightRoles::provision(&recovered.owner, recovered.roles);
        Ok((recovered.owner, roles, recovered.outcomes))
    }
}

#[cfg(test)]
mod tests;
