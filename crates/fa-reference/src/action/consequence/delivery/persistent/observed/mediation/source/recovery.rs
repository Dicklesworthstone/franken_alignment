//! Reconnect separately recovered observer custody without reviving an old cut.
use super::{FileMediationObserver, FileOversight, FileTopologySource, JournalError,
    LoadedAuthorityTopology, TopologySourceWork, TOPOLOGY_REFRESH_READ_BYTES};
use crate::Error;
use std::path::PathBuf;

/// Independently retained source context and spent work, NOT recovery authority.
/// Keep this alongside (not derive it from) the journal recovery requirements.
/// A stale or jointly rolled-back copy cannot prove the newest source allowance.
/// The newer complete input is selected independently, never learned from a
/// failed read. Supplied paths stay fixed; previous and next source limits and
/// checker allowance must match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopologySourceRecovery {
    pub topology_path: PathBuf,
    pub inventory_path: PathBuf,
    pub previous: LoadedAuthorityTopology,
    pub next: LoadedAuthorityTopology,
    pub work: TopologySourceWork,
    pub last_operation: u64,
}

impl FileTopologySource {
    /// Capture continuity requirements, not a file write or another observer.
    /// The caller still needs separately provisioned recovery custody. Retain
    /// the latest result independently; this does not persist work counters.
    pub fn recovery_requirements(&self, next: LoadedAuthorityTopology) -> TopologySourceRecovery {
        TopologySourceRecovery {
            topology_path: self.topology_path.clone(), inventory_path: self.inventory_path.clone(),
            previous: self.expected.clone(), next, work: self.work, last_operation: self.last_operation,
        }
    }
}

impl FileMediationObserver {
    /// Reconnect a source only AFTER native recovery/loss withdrew coverage.
    /// Check independent previous input, paths, spent allowance and operation
    /// continuity before the original topology update. Both graph and inventory
    /// generations must advance. No files are read and no cut is certified here.
    ///
    /// The returned source retains old work and the rebind operation. A fresh
    /// original refresh must certify the newer actual files before admission;
    /// reinstall it in the driver for its final-publication source check. All
    /// native update/replay/poisoning rules apply. This is not a mandatory-source
    /// deployment flag, a budget reset, automatic retry or an effect resend.
    pub fn resume_topology_source(self, host: &mut FileOversight, revision: u64,
        operation: u64, recovery: TopologySourceRecovery) -> Result<FileTopologySource, JournalError>
    {
        // Reuse the native withdrawn-source admission, including stop/owner
        // checks and the maximum operation in acknowledged journal history.
        let mut source = self.bind_withdrawn_topology_source(host, revision,
            recovery.topology_path, recovery.inventory_path, recovery.previous)?;
        if recovery.last_operation < recovery.work.checks { return Err(Error::Binding.into()); }
        let floor = source.last_operation.max(recovery.last_operation);
        if operation <= floor { return Err(Error::Stale.into()); }
        // Leave an ID for the mandatory next actual source observation.
        if operation == u64::MAX { return Err(Error::Overflow.into()); }
        let limits = source.limits;
        if recovery.work.checks.checked_mul(TOPOLOGY_REFRESH_READ_BYTES)
            != Some(recovery.work.reserved_read_bytes) {
            return Err(Error::Binding.into());
        }
        if recovery.work.checks >= limits.checks
            || recovery.work.reserved_read_bytes.checked_add(TOPOLOGY_REFRESH_READ_BYTES)
                .is_none_or(|bytes| bytes > limits.read_bytes) {
            return Err(Error::Limit.into());
        }
        // Restore only the independently retained accounting, never a cut.
        // Native admission above prevents a retained floor from hiding a larger
        // recorded operation. Rebind still owns all generation/profile checks.
        source.work = recovery.work;
        source.last_operation = floor;
        source.rebind(host, revision, operation, recovery.next)?;
        Ok(source)
    }
}

#[cfg(test)]
mod tests;
