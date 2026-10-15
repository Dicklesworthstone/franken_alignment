//! Restricted bridge: the existing driver owns the original source, not its role.
use super::{FileTopologySource, FileOversight, JournalError};
use crate::Error;

impl FileTopologySource {
    /// Last source operation, including a recovered native operation floor and
    /// automatic driver checks. This is not the global journal revision.
    pub fn last_operation(&self) -> u64 { self.last_operation }

    pub(in super::super::super) fn check_driver_binding(&self, host: &FileOversight,
        revision: u64) -> Result<(), JournalError>
    {
        self.observer.check_owner(host)?;
        if revision != host.revision() { return Err(Error::Stale.into()); }
        if !host.publication_guard_required()
            || &host.mediation_snapshot()?.graph != self.expected.graph() {
            return Err(Error::Binding.into());
        }
        Ok(())
    }

}

impl super::super::FileMediationObserver {
    /// Bind a NEW source custodian to the exact WITHDRAWN native graph. This
    /// performs no read, update, certification, clock observation or grant.
    /// The caller independently supplies the complete former input contract and
    /// paths for subsequent fresh reads. It must explicitly rebind to a newer
    /// graph AND inventory, then pass the original native checker before work.
    ///
    /// Native update IDs survive recovery. Start above their maximum rather
    /// than restarting at zero and colliding on a later source-loss withdrawal.
    /// Unpersisted checks are not reconstructed: work counts this newly admitted
    /// custodian only, not a recovered lifetime quota or a deployment-wide cap.
    /// The original source keeps its limits, paths and spent work across rebind.
    pub fn bind_withdrawn_topology_source(
        self,
        host: &FileOversight,
        revision: u64,
        topology_path: std::path::PathBuf,
        inventory_path: std::path::PathBuf,
        expected: crate::perimeter_inventory::topology::LoadedAuthorityTopology,
    ) -> Result<FileTopologySource, JournalError> {
        use super::super::{Event, MediationEvent};
        use std::path::Component;

        self.check_owner(host)?;
        if revision != host.revision() { return Err(Error::Stale.into()); }
        for path in [&topology_path, &inventory_path] {
            if !path.is_absolute() || path.file_name().is_none()
                || path.components().any(|part| matches!(part, Component::ParentDir)) {
                return Err(Error::InvalidInput.into());
            }
        }
        if topology_path == inventory_path { return Err(Error::Binding.into()); }
        let current = host.mediation_snapshot()?;
        if current.available || current.accepted.is_some() || host.inspect().stop.is_some() {
            return Err(Error::WrongState.into());
        }
        if !host.publication_guard_required() || &current.graph != expected.graph() {
            return Err(Error::Binding.into());
        }
        // Scan only the bounded ORIGINAL acknowledged events. A receipt map or
        // a caller-selected counter cannot hide a larger historical operation.
        let last_operation = host.events.iter().filter_map(|event| match event {
            Event::Mediation(MediationEvent::Update(update)) => Some(update.operation),
            _ => None,
        }).max().unwrap_or(0);
        if last_operation == u64::MAX { return Err(Error::Overflow.into()); }
        let limits = expected.source_limits();
        Ok(FileTopologySource {
            observer: self, topology_path, inventory_path, expected, limits,
            work: super::TopologySourceWork::default(), last_operation,
            last_rebind: None, last_refusal: None, last_check: None,
        })
    }
}

#[cfg(test)]
mod tests;
