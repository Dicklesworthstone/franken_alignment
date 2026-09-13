//! Sealed delegation to the two original durable request owners. This does not
//! admit third-party backends, callbacks, replacement reducers or reviewer roles.
use super::{FileDelivery, FileRequestStatus, ActionSpec, Snapshot, JournalError};
use super::super::super::JournalFailure;
use super::super::super::observed::FileOversight;

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::FileDelivery {}
    impl Sealed for super::FileOversight {}
}

/// Request-only backend used by FileActorPort. Implementations are sealed to
/// FileDelivery and FileOversight; their distinct bootstrap and authority rules
/// are not interchangeable. No review, human key, dispatch or publication method
/// belongs to this interface. Accessing a host still requires its supervisor.
pub trait FileRequestHost: sealed::Sealed {
    fn revision(&self) -> u64;
    fn storage_failure(&self) -> Option<&JournalFailure>;
    fn request_status(&self, request: u64) -> Result<FileRequestStatus, JournalError>;
    fn submit_request(&mut self, revision: u64, request: u64, spec: ActionSpec,
        snapshot: Snapshot) -> Result<FileRequestStatus, JournalError>;
    fn cancel_request(&mut self, revision: u64, request: u64) -> Result<(), JournalError>;
}

impl FileRequestHost for FileDelivery {
    fn revision(&self) -> u64 { FileDelivery::revision(self) }
    fn storage_failure(&self) -> Option<&JournalFailure> { FileDelivery::storage_failure(self) }
    fn request_status(&self, request: u64) -> Result<FileRequestStatus, JournalError> {
        FileDelivery::request_status(self, request)
    }
    fn submit_request(&mut self, revision: u64, request: u64, spec: ActionSpec,
        snapshot: Snapshot) -> Result<FileRequestStatus, JournalError>
    { FileDelivery::submit_request(self, revision, request, spec, snapshot) }
    fn cancel_request(&mut self, revision: u64, request: u64) -> Result<(), JournalError> {
        FileDelivery::cancel_request(self, revision, request)
    }
}
impl FileRequestHost for FileOversight {
    fn revision(&self) -> u64 { FileOversight::revision(self) }
    fn storage_failure(&self) -> Option<&JournalFailure> { FileOversight::storage_failure(self) }
    fn request_status(&self, request: u64) -> Result<FileRequestStatus, JournalError> {
        FileOversight::request_status(self, request)
    }
    fn submit_request(&mut self, revision: u64, request: u64, spec: ActionSpec,
        snapshot: Snapshot) -> Result<FileRequestStatus, JournalError>
    { FileOversight::submit_request(self, revision, request, spec, snapshot) }
    fn cancel_request(&mut self, revision: u64, request: u64) -> Result<(), JournalError> {
        FileOversight::cancel_request(self, revision, request)
    }
}
