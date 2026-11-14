//! Consume the original complete work checkpoint through the original driver.
//! No parallel accounting format, source custodian or recovery engine is added.
use super::{FileOversight, JournalError, LoadedAuthorityTopology, TopologyCheckpoint};
use crate::action::consequence::delivery::persistent::observed::{Event,
    driver::FileSupervisedDriver,
    mediation::{FileMediationObserver, source::{TopologyRefresh, TopologySourceRecovery}}};
use crate::Error;
use std::{path::PathBuf, rc::Rc};

/// Independently selected next files and complete next contract. The original
/// context and counters come from the checkpoint, never manual reconstruction.
pub struct TopologyCheckpointResume {
    pub topology_path: PathBuf,
    pub inventory_path: PathBuf,
    pub next: LoadedAuthorityTopology,
}

impl FileSupervisedDriver {
    /// Capture this driver's original source and healthy acknowledged owner at
    /// the selected revision. The supplied bytes remain independently retained
    /// original documents; the native checkpoint verifies their full equality.
    pub fn topology_checkpoint(&self, revision: u64, topology: &[u8], inventory: &[u8])
        -> Result<TopologyCheckpoint, JournalError>
    {
        let host = self.supervisor().host()?;
        if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
        if host.revision() != revision { return Err(Error::Stale.into()); }
        self.topology().ok_or(Error::Missing)?.checkpoint(&host, topology, inventory)
    }

    /// Consume saved expenditure only after native guarded recovery. Select the
    /// checkpoint's anchor and floors during that recovery as well: this check
    /// precedes source reinstatement, not an already completed recovery fence.
    ///
    /// A genuine history successor may contain later source work. Any topology
    /// event after the checkpoint refuses stale accounting; recovery fences,
    /// time observations and reconciliation may follow it without resetting work.
    /// The native resume path still requires newer graph/inventory, unchanged
    /// limits, room for two checks and source installation BEFORE fresh file I/O.
    /// Read failure preserves withdrawal and spent work. No old cut or keys return.
    pub fn resume_topology_checkpoint(&mut self, revision: u64, operation: u64,
        observer: FileMediationObserver, checkpoint: &TopologyCheckpoint,
        selection: TopologyCheckpointResume) -> Result<TopologyRefresh, JournalError>
    {
        {
            let host = self.supervisor().host()?;
            check_history(&host, revision, checkpoint)?;
        }
        self.resume_topology(revision, operation, observer, TopologySourceRecovery {
            topology_path: selection.topology_path, inventory_path: selection.inventory_path,
            previous: checkpoint.expected().clone(), next: selection.next,
            work: checkpoint.work(), last_operation: checkpoint.last_operation(),
        })
    }
}

impl FileMediationObserver {
    /// Carry independently retained source custody across exactly one original
    /// recovery Fence, before this observer is consumed by a live source.
    ///
    /// The SAME healthy owner and separately issued observer are required.
    /// Observer custody cannot coexist with a bound source, so RAM-only source
    /// operations cannot disappear behind an unchanged journal. The original
    /// complete prefix, graph, availability and floors are checked; no Time,
    /// source, numerical, policy, effect or second Fence may follow the saved cut.
    ///
    /// Original documents, work and last operation remain unchanged. Only the
    /// acknowledged history and native floors advance, with coverage withdrawn.
    /// This performs no I/O, replay, source check or mutation and issues no role.
    /// Keep the old pair until both independent successor exports succeed.
    ///
    /// The host alone cannot bypass the separately held observer:
    ///
    /// ```compile_fail,E0624
    /// use fa_reference::action::consequence::delivery::persistent::observed::{
    ///     FileOversight, driver::topology::checkpoint::TopologyCheckpoint,
    /// };
    /// fn bypass(saved: &TopologyCheckpoint, host: &FileOversight) {
    ///     let _ = saved.after_recovery_fence(host);
    /// }
    /// ```
    pub fn recovered_topology_checkpoint(
        &self,
        host: &FileOversight,
        checkpoint: &TopologyCheckpoint,
    ) -> Result<TopologyCheckpoint, JournalError> {
        if host.storage_failure().is_some() {
            return Err(JournalError::Unavailable);
        }
        if !Rc::ptr_eq(&self.issuer, &host.issuer) {
            return Err(Error::Binding.into());
        }
        if host.mediation_snapshot()?.accepted.is_some() {
            return Err(Error::Binding.into());
        }
        checkpoint.after_recovery_fence(host)
    }
}

fn check_history(host: &FileOversight, revision: u64, checkpoint: &TopologyCheckpoint)
    -> Result<(), JournalError>
{
    if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
    if host.revision() != revision { return Err(Error::Stale.into()); }
    let minimum = checkpoint.minimum();
    let state = host.inspect();
    if state.control.sequence < minimum.control_sequence
        || state.control.ledger.epoch < minimum.authority_epoch { return Err(Error::Stale.into()); }
    checkpoint.history_anchor().check(&host.profile, host.store.identity(), &host.events)?;
    // The original prefix checker has already established this slice is in range.
    let after = usize::try_from(checkpoint.history_anchor().revision()).map_err(|_| Error::Limit)?;
    if host.events[after..].iter().any(|event| matches!(event, Event::Mediation(_))) {
        return Err(Error::Stale.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
