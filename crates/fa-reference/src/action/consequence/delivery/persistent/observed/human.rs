//! Separate process-local reviewer custody around DURABLE original key events.
//! The embedding host must authenticate the human; this is not a signature scheme.
use super::{FileOversight, JournalError, journal::{Event, HumanDecision}, machine::Transition};
use crate::action::consequence::oversight::human::{HumanRequest, HumanRevocation};
use crate::Error;
use std::rc::Rc;

/// Immutable original review evidence, branded to this writable owner.
/// Cloning this value never copies either approval key.
#[derive(Clone, Debug)]
pub struct FileHumanRequest {
    pub(super) issuer: Rc<()>,
    pub(super) evidence: HumanRequest,
}
impl FileHumanRequest {
    pub fn evidence(&self) -> &HumanRequest { &self.evidence }
}

/// One separately issued second key, not an automatic effect permit.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::FileHumanPermit;
/// fn duplicate(key: FileHumanPermit) { let _ = key.clone(); }
/// ```
#[derive(Debug)]
pub struct FileHumanPermit {
    pub(super) issuer: Rc<()>,
    pub(super) request: u64,
    pub(super) attempt: u64,
}
impl FileHumanPermit {
    pub fn request(&self) -> u64 { self.request }
    pub fn attempt(&self) -> u64 { self.attempt }
}

/// Returned once by trusted create/open, not recoverable from the owner, a
/// request, a journal, or an approved status. Provision to the separate reviewer.
/// Reopening creates a NEW role brand only after the recovery fence commits.
/// All old role/key brands are refused, even for the same configured reviewer ID.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::FileHumanReviewer;
/// fn duplicate(role: FileHumanReviewer) { let _ = role.clone(); }
/// ```
#[derive(Debug)]
pub struct FileHumanReviewer {
    pub(super) issuer: Rc<()>,
    pub(super) reviewer: u64,
}
impl FileHumanReviewer {
    pub fn reviewer_id(&self) -> u64 { self.reviewer }

    fn check(&self, host: &FileOversight, request: Option<&FileHumanRequest>) -> Result<(), JournalError> {
        if !Rc::ptr_eq(&self.issuer, &host.issuer)
            || request.is_some_and(|request| !Rc::ptr_eq(&request.issuer, &host.issuer))
        { return Err(Error::Binding.into()); }
        Ok(())
    }

    /// The host must explicitly admit a current clock tick first. Success is
    /// returned only after the ORIGINAL approval transition is durably committed.
    /// An ambiguous storage failure returns no second key.
    pub fn approve(&self, host: &mut FileOversight, revision: u64, request: &FileHumanRequest) -> Result<FileHumanPermit, JournalError> {
        self.check(host, Some(request))?;
        host.transact(revision, Event::Human(request.evidence.id(), HumanDecision::Approve))?;
        Ok(FileHumanPermit { issuer: Rc::clone(&self.issuer), request: request.evidence.id(), attempt: request.evidence.attempt() })
    }

    /// Withdrawal never refunds an automatic reservation or proves that an
    /// already-dispatched effect did not execute. Use the original stop/drain
    /// protocol for outstanding effects. No fresh clock is needed to withdraw.
    pub fn reject(&self, host: &mut FileOversight, revision: u64, request: &FileHumanRequest) -> Result<(), JournalError> {
        self.check(host, Some(request))?;
        host.transact(revision, Event::Human(request.evidence.id(), HumanDecision::Reject))?;
        Ok(())
    }
    pub fn revoke(&self, host: &mut FileOversight, revision: u64, request: &FileHumanRequest) -> Result<(), JournalError> {
        self.check(host, Some(request))?;
        host.transact(revision, Event::Human(request.evidence.id(), HumanDecision::Revoke))?;
        Ok(())
    }
    pub fn revoke_all(&self, host: &mut FileOversight, revision: u64) -> Result<HumanRevocation, JournalError> {
        self.check(host, None)?;
        match host.transact(revision, Event::RevokeHumans)? {
            Transition::HumansRevoked(receipt) => Ok(receipt), _ => unreachable!("human withdrawal transition"),
        }
    }
}
