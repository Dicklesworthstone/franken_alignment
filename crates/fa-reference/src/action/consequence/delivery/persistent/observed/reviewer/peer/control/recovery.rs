//! Authenticated admission withdrawal on the SAME fully verified recovery cursor.
//! No live owner, reviewer role, settlement clock or ordinary recovery Fence is
//! created while waiting for an explicit nonce-bound Stop request.
use super::{Error, JournalError, ReviewerError, ReviewerExpectation, StopBinding,
    StopControlPhase, StopControlProgress, StopControlReceipt, transient};
use super::wire::{OFFER_BYTES, RECEIPT_BYTES};
use crate::action::consequence::delivery::StopRequest;
use crate::action::consequence::delivery::persistent::observed::shutdown::recovery::{
    FileAdmissionStoppedHistory, FileStopRecovery,
};
use std::{fmt, io::{self, Read, Write}};

/// A fixed Stop selected from one fully replayed original learned history.
/// Its creating wrapper checks its COMPLETE independent guard contract before
/// transferring the original exclusive lock and native reducer. This value can
/// only be consumed by the Stop transport, or dropped without changing storage.
/// It exposes neither the weaker recovery cursor nor a live owner or role.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::control::FileRecoveryStop;
/// fn bypass(target: FileRecoveryStop) { target.into_inner().finish(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::control::FileRecoveryStop;
/// fn role(target: &FileRecoveryStop) { target.roles(); }
/// ```
#[must_use = "serve this fixed Stop target or drop it without changing storage"]
pub struct FileRecoveryStop {
    recovery: FileStopRecovery,
    expected: ReviewerExpectation,
    request: StopRequest,
}
impl FileRecoveryStop {
    pub(in crate::action::consequence::delivery::persistent::observed)
    fn from_verified(recovery: FileStopRecovery, expected: ReviewerExpectation,
        request: StopRequest) -> Self
    {
        Self { recovery, expected, request }
    }
    pub fn expectation(&self) -> ReviewerExpectation { self.expected }
    pub fn request(&self) -> StopRequest { self.request }
}
impl fmt::Debug for FileRecoveryStop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileRecoveryStop").field("expected", &self.expected)
            .field("request", &self.request).finish_non_exhaustive()
    }
}

/// The exact native result and original-history anchor remain available after
/// a lost network reply. Admission acknowledgement never implies settlement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StopRecoveryApplication {
    pub result: Result<FileAdmissionStoppedHistory, JournalError>,
    pub receipt: StopControlReceipt,
}

/// One independently authenticated, fixed-size Stop exchange on the original
/// recovery cursor. Generic streams require bounded/nonblocking I/O and peer
/// authentication supplied by the embedding supervisor. The Linux verified
/// socket conversion enforces peer identity before runnable protocol I/O.
///
/// There is no mutation callback, weaker-cursor extraction, clock or role getter.
/// Only the complete exact request applies the original admission Stop consumer.
/// Failure before application drops the held cursor without a cleanup or write;
/// failure afterward retains the native acknowledgement for operator recovery.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::control::StopRecoveryConnection;
/// fn owner(channel: StopRecoveryConnection<std::io::Cursor<Vec<u8>>>) { channel.into_owner(); }
/// ```
#[must_use = "poll the bounded exchange or drop it without applying an unrequested Stop"]
pub struct StopRecoveryConnection<S> {
    stream: Option<S>,
    target: Option<FileRecoveryStop>,
    binding: StopBinding,
    phase: StopControlPhase,
    output: [u8; RECEIPT_BYTES],
    output_len: usize,
    written: usize,
    input: [u8; OFFER_BYTES],
    received: usize,
    application: Option<StopRecoveryApplication>,
    failure: Option<ReviewerError>,
}
impl<S> StopRecoveryConnection<S> {
    pub fn phase(&self) -> StopControlPhase { self.phase }
    pub fn application(&self) -> Option<&StopRecoveryApplication> { self.application.as_ref() }
    pub fn failure(&self) -> Option<&ReviewerError> { self.failure.as_ref() }

    /// Move only immutable native result evidence. The socket and any unapplied
    /// cursor are dropped; no raw transport, live owner or reviewer can escape.
    pub fn into_application(self) -> Option<StopRecoveryApplication> { self.application }
}
impl<S: Read + Write> StopRecoveryConnection<S> {
    pub fn new(target: FileRecoveryStop, stream: S, session: [u8; 32])
        -> Result<Self, ReviewerError>
    {
        let binding = StopBinding { expected: target.expected,
            operation: target.request.operation, session };
        let offer = binding.offer()?;
        let mut output = [0; RECEIPT_BYTES];
        output[..OFFER_BYTES].copy_from_slice(&offer);
        Ok(Self { stream: Some(stream), target: Some(target), binding,
            phase: StopControlPhase::Offering, output, output_len: OFFER_BYTES, written: 0,
            input: [0; OFFER_BYTES], received: 0, application: None, failure: None })
    }

    /// One bounded read/write per call. Only a COMPLETE exact original request
    /// enters the native Stop/Fence consumer. Applying is latched before moving
    /// the cursor: even a caught unwind cannot reapply it. No clock is acquired
    /// and no drain, settlement or ordinary recovery Fence is manufactured.
    pub fn step(&mut self) -> Result<StopControlProgress, ReviewerError> {
        if let Some(error) = &self.failure { return Err(error.clone()); }
        let result = self.step_inner();
        if let Err(error) = &result {
            self.failure = Some(error.clone());
            self.phase = StopControlPhase::Failed;
            self.stream = None;
            self.target = None;
        }
        result
    }
    fn step_inner(&mut self) -> Result<StopControlProgress, ReviewerError> {
        match self.phase {
            StopControlPhase::Offering | StopControlPhase::SendingReceipt => self.send(),
            StopControlPhase::AwaitingRequest => {
                let offered = OFFER_BYTES - self.received;
                match self.stream.as_mut().ok_or(Error::WrongState)?
                    .read(&mut self.input[self.received..]) {
                    Ok(0) => return Err(ReviewerError::Io(io::ErrorKind::UnexpectedEof)),
                    Ok(count) if count <= offered => self.received += count,
                    Ok(_) => return Err(Error::InvalidInput.into()),
                    Err(error) if transient(error.kind()) => return Ok(StopControlProgress::Blocked),
                    Err(error) => return Err(ReviewerError::Io(error.kind())),
                }
                if self.received != OFFER_BYTES { return Ok(StopControlProgress::Progress); }
                if self.input != self.binding.request()? { return Err(Error::Binding.into()); }
                self.phase = StopControlPhase::Applying;
                let target = self.target.take().ok_or(Error::WrongState)?;
                let result = target.recovery.finish_admission_stop(target.request);
                let receipt = StopControlReceipt::project_admission(self.binding, &result);
                // Preserve native acknowledgement before encoding or receipt I/O.
                self.application = Some(StopRecoveryApplication { result, receipt });
                self.output = receipt.encode()?;
                self.output_len = RECEIPT_BYTES;
                self.written = 0;
                self.phase = StopControlPhase::SendingReceipt;
                Ok(StopControlProgress::Applied)
            }
            StopControlPhase::Complete => Ok(StopControlProgress::Complete),
            StopControlPhase::Applying | StopControlPhase::Failed => Err(Error::WrongState.into()),
        }
    }
    fn send(&mut self) -> Result<StopControlProgress, ReviewerError> {
        let stream = self.stream.as_mut().ok_or(Error::WrongState)?;
        if self.written < self.output_len {
            let offered = self.output_len - self.written;
            match stream.write(&self.output[self.written..self.output_len]) {
                Ok(0) => return Err(ReviewerError::Io(io::ErrorKind::WriteZero)),
                Ok(count) if count <= offered => self.written += count,
                Ok(_) => return Err(Error::InvalidInput.into()),
                Err(error) if transient(error.kind()) => return Ok(StopControlProgress::Blocked),
                Err(error) => return Err(ReviewerError::Io(error.kind())),
            }
            if self.written < self.output_len { return Ok(StopControlProgress::Progress); }
        }
        match stream.flush() {
            Ok(()) => {
                if self.phase == StopControlPhase::Offering {
                    self.phase = StopControlPhase::AwaitingRequest;
                    Ok(StopControlProgress::Progress)
                } else {
                    self.phase = StopControlPhase::Complete;
                    self.stream = None;
                    Ok(StopControlProgress::Complete)
                }
            }
            Err(error) if transient(error.kind()) => Ok(StopControlProgress::Blocked),
            Err(error) => Err(ReviewerError::Io(error.kind())),
        }
    }
}
impl<S> fmt::Debug for StopRecoveryConnection<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StopRecoveryConnection").field("binding", &self.binding)
            .field("phase", &self.phase).field("applied", &self.application.is_some())
            .field("failure", &self.failure).finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
