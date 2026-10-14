//! Restricted bridge: the existing driver owns the original source, not its role.
use super::{FileTopologySource, FileOversight, JournalError};
use crate::Error;

impl FileTopologySource {
    /// Last operation consumed by THIS custodian, including automatic driver
    /// checks. This is not the global journal revision or another owner's ID.
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
