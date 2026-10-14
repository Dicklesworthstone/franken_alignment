//! Restricted bridge: the existing driver owns the original source, not its role.
use super::{FileTopologySource, FileOversight, JournalError};
use crate::Error;

impl FileTopologySource {
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
