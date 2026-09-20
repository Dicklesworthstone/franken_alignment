//! Durable nonpermitting observations of a valid but lagging producer image.
//! Only the original journal acknowledgment makes this result usable as history.
use super::{FileCaptureError, FileCaptureIdentity, FilePublicationCapture, PublicationInputFile};
use super::{Event, FileOversight, JournalError, JournalFailure, JournalIo, Machine, Transition, WitnessEvent, journal};
use crate::action::ActionState;
use crate::action::consequence::delivery::publication_gate::changes::PublicationCaptureOutcome;
use crate::Error;
use std::rc::Rc;

/// Acknowledged observation, not an effect key or an exact-validation receipt.
/// Deferred carries no current input or fresh lease. Installed is still one-use
/// evidence and can be invalidated before the original authorization boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileCaptureObservation {
    pub identity: FileCaptureIdentity,
    pub outcome: PublicationCaptureOutcome,
}

impl FileOversight {
    /// Reread a cut-bound source before dispatch. A valid lagging image is
    /// durably recorded as Deferred instead of forcing fenced recovery. No
    /// current input is installed and the original reservation/review stays put.
    /// Retry requires a NEW withdrawal/read; neither generation nor time is
    /// synthesized. Legacy sources and already dispatched attempts refuse.
    ///
    /// Withdrawal commits before opening the concrete file. Read/decode failure
    /// is an inner error with inputs unavailable. Admission or persistence error
    /// after a successful read quarantines the owner exactly like strict capture;
    /// no candidate outcome or high-water mark is returned as acknowledged.
    pub fn refresh_publication_from_file_or_defer(&mut self, revision: u64, attempt: u64,
        source: &PublicationInputFile)
        -> Result<Result<FileCaptureObservation, FileCaptureError>, JournalError>
    {
        let expected = self.begin_deferrable_publication_capture(revision, attempt, source.source())?;
        let capture = match source.read_capture() { Ok(capture) => capture, Err(error) => return Ok(Err(error)) };
        Ok(Ok(self.finish_publication_capture_or_defer(attempt, expected, capture)?))
    }

    // Shared with the original supervised provider around BOTH its reads.
    pub(in super::super::super) fn begin_deferrable_publication_capture(&mut self,
        revision: u64, attempt: u64, source: u64) -> Result<u64, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        if !matches!(self.inspect().control.ledger.stages.get(&attempt),
            Some(ActionState::Reviewing | ActionState::Authorized)) { return Err(Error::WrongState.into()); }
        self.machine.broker.publication_input_cut(attempt)?.ok_or(Error::Binding)?;
        self.begin_publication_capture(revision, attempt, source)
    }

    pub(in super::super::super) fn finish_publication_capture_or_defer(&mut self,
        attempt: u64, expected: u64, capture: FilePublicationCapture)
        -> Result<FileCaptureObservation, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if self.machine.broker.publication_input_revision(attempt)? != expected { return Err(Error::Stale.into()); }
        let identity = capture.identity();
        let event = Event::PublicationWitness(WitnessEvent::CapturedOrDefer(attempt, expected, Rc::new(capture)));
        self.check_source_admission(&event)?;
        self.fault = Some(JournalFailure { operation: JournalIo::Stage,
            kind: std::io::ErrorKind::Other, replacement_may_be_visible: false });
        if self.events.len() >= self.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        let bytes = journal::encode_appended(&self.profile, self.store.identity(), &self.events, &event)?;
        let mut candidate = Machine::replay(&self.profile, &self.events)?;
        let result = candidate.apply(&event)?;
        let revision = match &result { Transition::Inputs(revision) => *revision,
            _ => return Err(Error::Binding.into()) };
        // Project the result from the original candidate after ALL its semantic
        // gates, including request refresh. No caller-supplied outcome is trusted.
        let state = candidate.broker.publication_source(attempt)?.ok_or(Error::Incomplete)?;
        let cut = candidate.broker.publication_input_cut(attempt)?.ok_or(Error::Incomplete)?;
        if state.capture_pending || state.source != identity.source || state.generation != identity.generation {
            return Err(Error::Binding.into());
        }
        let outcome = match (state.fresh, cut.last.through >= cut.required_through) {
            (true, true) => PublicationCaptureOutcome::Installed { revision },
            (false, false) => PublicationCaptureOutcome::Deferred {
                revision, observed: cut.last, required_through: cut.required_through,
            },
            _ => return Err(Error::Binding.into()),
        };
        self.persist_candidate(event, bytes, candidate, result)?;
        Ok(FileCaptureObservation { identity, outcome })
    }
}

#[cfg(test)]
mod storage_tests;
