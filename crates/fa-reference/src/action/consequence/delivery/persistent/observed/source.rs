//! File observations feeding the ORIGINAL mandatory captured-state gate.
//! The file producer's version floor is durable; a saved observation is not live
//! after recovery. Reads, journals and trusted clocks remain a reference profile.
mod codec;
mod replacement;
pub mod publisher;
#[cfg(test)]
mod interruption_tests;
pub use replacement::FileSourceReplacement;
pub(super) use codec::{read, write};

use super::{Event, FileOversight, JournalError, Transition, BaseEvent};
use super::journal::HumanDecision;
use crate::action::ElapsedTick;
use crate::action::consequence::oversight::evidence_source::{EvidenceError, EvidenceFile, EvidenceIdentity, EvidenceSnapshot};
use crate::action::consequence::oversight::policy_state::{StateCaptureStatus, StateFreshness, StateLimits, StateSource};
use crate::Error;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileSourcePolicy { pub source: StateSource, pub limits: StateLimits, pub freshness: StateFreshness }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileSourceStatus {
    pub policy: FileSourcePolicy,
    pub producer: Option<EvidenceIdentity>,
    pub semantic_epoch: Option<u64>,
    pub capture: StateCaptureStatus,
    pub last_refusal: Option<Error>,
    pub interrupted: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileSourceError {
    Journal(JournalError),
    Refused(Error),
    Read { error: EvidenceError, withdrawal: Option<JournalError> },
}
impl From<JournalError> for FileSourceError { fn from(error: JournalError) -> Self { Self::Journal(error) } }
impl std::fmt::Display for FileSourceError { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{self:?}") } }
impl std::error::Error for FileSourceError {}

#[derive(Clone)]
pub(super) enum SourceEvent { Enable(FileSourcePolicy), Observe(Rc<EvidenceSnapshot>, ElapsedTick), Withdraw, Replace(FileSourceReplacement) }

impl FileOversight {
    pub fn enable_file_source(&mut self, revision: u64, policy: FileSourcePolicy) -> Result<(), JournalError> {
        self.transact(revision, Event::Source(SourceEvent::Enable(policy)))?; Ok(())
    }
    pub fn file_source_required(&self) -> bool { self.machine.file_source_status().is_some() }
    pub fn file_source_status(&self) -> Option<FileSourceStatus> {
        self.machine.file_source_status().map(|mut status| { status.interrupted = self.source_interrupted; status })
    }
    pub fn withdraw_file_source(&mut self, revision: u64) -> Result<(), JournalError> {
        if revision != self.revision() { return Err(Error::Stale.into()); }
        if !self.file_source_required() { return Err(Error::WrongState.into()); }
        self.source_interrupted = true;
        self.transact(revision, Event::Source(SourceEvent::Withdraw))?; Ok(())
    }
    /// Admission closes before entering the reader, including across a caught
    /// unwind. Only an acknowledged observation or withdrawal can reopen it.
    /// The timestamp is the trusted observation START, not a lease extension
    /// obtained by timestamping the completion of a slow read.
    pub fn refresh_file_source<S: EvidenceFile + ?Sized>(&mut self, revision: u64,
        source: &mut S, observed_at: ElapsedTick) -> Result<Rc<EvidenceSnapshot>, FileSourceError>
    {
        self.refresh_source_with(revision, || source.read_evidence(), observed_at)
    }

    // Private seam for the sealed readers and causal interruption tests. There
    // is deliberately no public callback API that could substitute cached data.
    fn refresh_source_with<F>(&mut self, revision: u64, read: F, observed_at: ElapsedTick)
        -> Result<Rc<EvidenceSnapshot>, FileSourceError>
    where F: FnOnce() -> Result<Rc<EvidenceSnapshot>, EvidenceError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable.into()); }
        if revision != self.revision() { return Err(JournalError::from(Error::Stale).into()); }
        if !self.file_source_required() { return Err(JournalError::from(Error::WrongState).into()); }
        if self.source_interrupted { self.withdraw_file_source(revision)?; }
        let revision = self.revision();
        // Do not wait for an Err: unwinding through the read, encoding, replay
        // or storage path must not leave the previous source eligible.
        self.source_interrupted = true;
        let captured = match read() {
            Ok(captured) => captured,
            Err(error) => {
                let withdrawal = self.withdraw_file_source(revision).err();
                return Err(FileSourceError::Read { error, withdrawal });
            }
        };
        let applied = match self.transact(revision, Event::Source(SourceEvent::Observe(Rc::clone(&captured), observed_at))) {
            Ok(applied) => applied,
            Err(error) => { self.source_interrupted = true; return Err(FileSourceError::Journal(error)); }
        };
        match applied {
            Transition::SourceObserved(Ok(_)) => Ok(captured),
            Transition::SourceObserved(Err(error)) => Err(FileSourceError::Refused(error)),
            _ => unreachable!("source observation transition"),
        }
    }

    pub(super) fn check_source_admission(&self, event: &Event) -> Result<(), Error> {
        self.check_source_admission_at(self.source_interrupted, event)
    }

    // A private canonical candidate uses exactly the same admission law, but
    // its own acknowledged-prefix interruption state. No public bypass exists.
    pub(super) fn check_source_admission_at(&self, interrupted: bool, event: &Event) -> Result<(), Error> {
        if !interrupted { return Ok(()); }
        // Independent identity measurement/containment cannot publish effects
        // or clear this source latch, even when the identity result is Matching.
        if matches!(event,
            Event::Source(SourceEvent::Withdraw | SourceEvent::Replace(_) | SourceEvent::Observe(..))
            | Event::Identity(_)
            // Qualification loss does not acknowledge or repair the source.
            | Event::Credibility(super::credibility::CredibilityEvent::WithdrawHeldOut(_))
            | Event::Decoder(super::decoder::DecoderEvent::CancelGeneration { .. })
            | Event::Decoder(super::decoder::DecoderEvent::Checkpoint(
                super::decoder::checkpoint::CheckpointRequest::Reset { .. }, _))
            | Event::CredentialRotate(_) | Event::CredentialRevoke(_)
            | Event::Core(BaseEvent::Time(_) | BaseEvent::Cancel(_) | BaseEvent::Fence
                | BaseEvent::Stop(_) | BaseEvent::StopProgress(_) | BaseEvent::ReplacePolicy(_)
                | BaseEvent::Reconcile(_) | BaseEvent::Seal(_) | BaseEvent::Sweep)
            | Event::InputsUnavailable(..) | Event::RevokeHumans | Event::ActorReset(..)
            | Event::Human(_, HumanDecision::Reject | HumanDecision::Revoke))
        { Ok(()) } else { Err(Error::Incomplete) }
    }

    pub(super) fn source_operation_committed(&mut self, event: &Event) {
        // A committed refusal has already withdrawn the original source and
        // review/key eligibility in Machine::observe_source. Clearing this latch
        // therefore cannot turn that refusal into a permitting snapshot.
        if Self::source_operation_acknowledges(event) {
            self.source_interrupted = false;
        }
    }

    pub(super) fn source_operation_acknowledges(event: &Event) -> bool {
        matches!(event, Event::Source(SourceEvent::Withdraw | SourceEvent::Replace(_) | SourceEvent::Observe(..)) | Event::Core(BaseEvent::Fence))
    }
}
