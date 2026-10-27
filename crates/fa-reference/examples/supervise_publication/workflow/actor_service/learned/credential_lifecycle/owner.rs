//! Private adapter for normal and operation-bound maintenance. This trait never
//! exposes publication, generation or an unguarded original owner.
use super::{FileOversight, JournalError, Request};
use fa_reference::action::consequence::delivery::{
    credential_broker::CredentialChangeReceipt,
    persistent::observed::{
        credential::FileCredentialStatus,
        driver::topology::checkpoint::TopologyCheckpoint,
        guarded::{
            anchored::FileHistoryAnchor, mediated::credential::FileCredentialMaintenanceOwner,
        },
    },
};

pub(super) trait Host {
    fn revision(&self) -> u64;
    fn credential_status(&self) -> Result<Option<FileCredentialStatus>, JournalError>;
    fn storage_failed(&self) -> bool;
    fn history_anchor_after(
        &self,
        previous: &FileHistoryAnchor,
    ) -> Result<FileHistoryAnchor, JournalError>;
    fn apply(&mut self, request: Request) -> Result<CredentialChangeReceipt, JournalError>;
    fn checkpoint(&self, saved: &TopologyCheckpoint) -> Result<TopologyCheckpoint, JournalError>;
}

impl Host for FileOversight {
    fn revision(&self) -> u64 {
        self.revision()
    }
    fn credential_status(&self) -> Result<Option<FileCredentialStatus>, JournalError> {
        self.credential_status()
    }
    fn storage_failed(&self) -> bool {
        self.storage_failure().is_some()
    }
    fn history_anchor_after(
        &self,
        previous: &FileHistoryAnchor,
    ) -> Result<FileHistoryAnchor, JournalError> {
        self.history_anchor_after(previous)
    }
    fn apply(&mut self, request: Request) -> Result<CredentialChangeReceipt, JournalError> {
        request.apply(self)
    }
    fn checkpoint(&self, saved: &TopologyCheckpoint) -> Result<TopologyCheckpoint, JournalError> {
        saved.after_credential_lifecycle(self)
    }
}

impl Host for FileCredentialMaintenanceOwner {
    fn revision(&self) -> u64 {
        self.revision()
    }
    fn credential_status(&self) -> Result<Option<FileCredentialStatus>, JournalError> {
        self.credential_status()
    }
    fn storage_failed(&self) -> bool {
        self.storage_failure().is_some()
    }
    fn history_anchor_after(
        &self,
        previous: &FileHistoryAnchor,
    ) -> Result<FileHistoryAnchor, JournalError> {
        self.history_anchor_after(previous)
    }
    fn apply(&mut self, _request: Request) -> Result<CredentialChangeReceipt, JournalError> {
        // This native owner already binds the one preflighted exact operation.
        self.apply()
    }
    fn checkpoint(&self, _saved: &TopologyCheckpoint) -> Result<TopologyCheckpoint, JournalError> {
        self.topology_checkpoint()
    }
}
