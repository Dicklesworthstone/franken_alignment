//! File observations feeding the ORIGINAL mandatory captured-state gate.
//! The file producer's version floor is durable; a saved observation is not live
//! after recovery. Reads, journals and trusted clocks remain a reference profile.
mod codec;
pub(super) use codec::{read, write};

use super::{Event, FileOversight, JournalError, Transition, BaseEvent};
use super::journal::HumanDecision;
use crate::action::ElapsedTick;
use crate::action::consequence::oversight::evidence_source::{
    EvidenceError, EvidenceFile, EvidenceIdentity, EvidenceSnapshot,
};
use crate::action::consequence::oversight::policy_state::{
    StateCaptureStatus, StateFreshness, StateLimits, StateSource,
};
use crate::Error;
use std::rc::Rc;

/// Irreversible pre-proposal configuration, including a fixed observation-age
/// ceiling. The source incarnation and file PRODUCER generation are different.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileSourcePolicy {
    pub source: StateSource,
    pub limits: StateLimits,
    pub freshness: StateFreshness,
}

/// Historical source accounting. Eligibility is checked by the original gate,
/// not by a cached status or a client-provided complete flag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileSourceStatus {
    pub policy: FileSourcePolicy,
    pub producer: Option<EvidenceIdentity>,
    pub semantic_epoch: Option<u64>,
    pub capture: StateCaptureStatus,
    pub last_refusal: Option<Error>,
    /// This live owner observed a source operation that it could not persist.
    /// Historical capture.closed may still be Some, but admission is blocked.
    pub interrupted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileSourceError {
    Journal(JournalError),
    /// The attempted observation was journaled, including withdrawal, before
    /// this original validation error was returned. It cannot revive old input.
    Refused(Error),
    /// A reader failure and failure to persist its withdrawal are independent.
    /// None means withdrawal committed, not that the read was successful.
    Read { error: EvidenceError, withdrawal: Option<JournalError> },
}
impl From<JournalError> for FileSourceError {
    fn from(error: JournalError) -> Self { Self::Journal(error) }
}
impl std::fmt::Display for FileSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for FileSourceError {}

#[derive(Clone)]
pub(super) enum SourceEvent {
    Enable(FileSourcePolicy),
    Observe(Rc<EvidenceSnapshot>, ElapsedTick),
    Withdraw,
}

impl FileOversight {
    /// Enable the original leased PolicyStateCapture before any proposal. This
    /// also enables the existing first-publication guard: source loss must not
    /// be bypassed between dispatch and the first externally visible mutation.
    /// No disable, age-widening, source-switching or quota-reset operation exists.
    pub fn enable_file_source(&mut self, revision: u64, policy: FileSourcePolicy) -> Result<(), JournalError> {
        self.transact(revision, Event::Source(SourceEvent::Enable(policy)))?;
        Ok(())
    }

    pub fn file_source_required(&self) -> bool { self.machine.file_source_status().is_some() }

    /// This may lag the canonical file after a failed replacement. It supplies
    /// no approval, source writer, fresh snapshot or proof of external coverage.
    pub fn file_source_status(&self) -> Option<FileSourceStatus> {
        self.machine.file_source_status().map(|mut status| {
            status.interrupted = self.source_interrupted;
            status
        })
    }

    pub fn withdraw_file_source(&mut self, revision: u64) -> Result<(), JournalError> {
        if revision != self.revision() { return Err(Error::Stale.into()); }
        if !self.file_source_required() { return Err(Error::WrongState.into()); }
        // Observation withdrawal is known locally even when the journal cannot
        // accept it. Only acknowledged withdrawal/fencing can clear this latch.
        self.source_interrupted = true;
        self.transact(revision, Event::Source(SourceEvent::Withdraw))?;
        Ok(())
    }

    /// Open/read through an ORIGINAL sealed file reader, then persist the exact
    /// observation through the native state writer. Only acknowledged success
    /// returns the captured data. The generation/semantic floors are checked
    /// against retained journal state, even for a newly constructed file reader.
    ///
    /// observed_at is a trusted tick sampled BEFORE the file read. Using the
    /// read-start bound is conservative: read/parse/commit latency consumes the
    /// fixed lease instead of extending it. The next consumer must advance its
    /// own clock before checking permission. This is not the producer's timestamp.
    pub fn refresh_file_source<S: EvidenceFile + ?Sized>(&mut self, revision: u64,
        source: &mut S, observed_at: ElapsedTick) -> Result<Rc<EvidenceSnapshot>, FileSourceError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable.into()); }
        if revision != self.revision() { return Err(JournalError::from(Error::Stale).into()); }
        if !self.file_source_required() { return Err(JournalError::from(Error::WrongState).into()); }
        if self.source_interrupted { self.withdraw_file_source(revision)?; }
        let revision = self.revision();
        let captured = match source.read_evidence() {
            Ok(captured) => captured,
            Err(error) => {
                let withdrawal = self.withdraw_file_source(revision).err();
                return Err(FileSourceError::Read { error, withdrawal });
            }
        };
        let applied = match self.transact(revision, Event::Source(SourceEvent::Observe(Rc::clone(&captured), observed_at))) {
            Ok(applied) => applied,
            Err(error) => {
                // Includes pre-write byte/event/allocation refusal, not only I/O.
                // No synthetic I/O failure is invented for a capacity condition.
                self.source_interrupted = true;
                return Err(FileSourceError::Journal(error));
            }
        };
        match applied {
            Transition::SourceObserved(Ok(_)) => Ok(captured),
            Transition::SourceObserved(Err(error)) => Err(FileSourceError::Refused(error)),
            _ => unreachable!("source observation transition"),
        }
    }

    pub(super) fn check_source_admission(&self, event: &Event) -> Result<(), Error> {
        if !self.source_interrupted { return Ok(()); }
        // Known source loss cannot be forgotten by the next speculative replay.
        // Original withdrawal, clock, cancellation and settlement remain usable.
        // This allowlist grants no effect, key, helper review or source renewal.
        if matches!(event,
            Event::Source(SourceEvent::Withdraw)
            | Event::Core(BaseEvent::Time(_) | BaseEvent::Cancel(_) | BaseEvent::Fence
                | BaseEvent::Stop(_) | BaseEvent::StopProgress(_) | BaseEvent::ReplacePolicy(_)
                | BaseEvent::Reconcile(_) | BaseEvent::Seal(_) | BaseEvent::Sweep)
            | Event::InputsUnavailable(..) | Event::RevokeHumans | Event::ActorReset(..)
            | Event::Human(_, HumanDecision::Reject | HumanDecision::Revoke))
        { Ok(()) } else { Err(Error::Incomplete) }
    }

    pub(super) fn source_operation_committed(&mut self, event: &Event) {
        if matches!(event, Event::Source(SourceEvent::Withdraw) | Event::Core(BaseEvent::Fence)) {
            self.source_interrupted = false;
        }
    }
}
