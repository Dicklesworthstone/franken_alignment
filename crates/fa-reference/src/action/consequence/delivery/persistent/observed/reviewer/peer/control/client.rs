//! Stop-only client. No supervisor, reviewer role or live capability crosses I/O.
use super::{Error, ReviewerError, ReviewerExpectation, StopBinding, StopControlReceipt, transient};
use super::wire::{OFFER_BYTES, RECEIPT_BYTES};
use crate::action::Purpose;
use std::io::{self, Read, Write};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopClientPhase { ReadingOffer, NeedsDecision, SendingRequest, AwaitingReceipt, Complete, Failed }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopClientProgress { Progress, Blocked, NeedsDecision, Complete }

/// An explicit request_stop() is required after independently checking the
/// offered audience. A transmitted request, disconnected socket or unconfirmed
/// result never proves durable stop. Read-only receipt data is not an effect key.
pub struct StopClient<S> {
    stream: Option<S>,
    expected: ReviewerExpectation,
    operation: u64,
    phase: StopClientPhase,
    raw: [u8; RECEIPT_BYTES],
    received: usize,
    written: usize,
    binding: Option<StopBinding>,
    receipt: Option<StopControlReceipt>,
    attempted: bool,
    failure: Option<ReviewerError>,
}
impl<S: Read + Write> StopClient<S> {
    pub fn new(stream: S, expected: ReviewerExpectation, operation: u64) -> Result<Self, Error> {
        expected.scope.validate()?;
        if expected.scope.purpose != Purpose::Effect { return Err(Error::Binding); }
        if expected.reviewer == 0 || expected.clock_domain == 0 || operation == 0 { return Err(Error::InvalidInput); }
        Ok(Self { stream: Some(stream), expected, operation, phase: StopClientPhase::ReadingOffer,
            raw: [0; RECEIPT_BYTES], received: 0, written: 0, binding: None, receipt: None,
            attempted: false, failure: None })
    }
    pub fn phase(&self) -> StopClientPhase { self.phase }
    pub fn binding(&self) -> Option<StopBinding> { self.binding }
    pub fn receipt(&self) -> Option<StopControlReceipt> { self.receipt }
    pub fn failure(&self) -> Option<&ReviewerError> { self.failure.as_ref() }
    pub fn outcome_unknown(&self) -> bool {
        self.attempted && !self.receipt.is_some_and(StopControlReceipt::acknowledged)
    }
    /// Explicit, restrictive operator choice. Never triggered by step() alone.
    pub fn request_stop(&mut self) -> Result<(), Error> {
        if self.phase != StopClientPhase::NeedsDecision { return Err(Error::WrongState); }
        let request = self.binding.ok_or(Error::WrongState)?.request()?;
        self.raw[..OFFER_BYTES].copy_from_slice(&request);
        self.written = 0; self.phase = StopClientPhase::SendingRequest;
        Ok(())
    }
    pub fn step(&mut self) -> Result<StopClientProgress, ReviewerError> {
        if let Some(error) = &self.failure { return Err(error.clone()); }
        let result = self.step_inner();
        if let Err(error) = &result {
            self.failure = Some(error.clone()); self.phase = StopClientPhase::Failed; self.stream = None;
        }
        result
    }
    fn step_inner(&mut self) -> Result<StopClientProgress, ReviewerError> {
        match self.phase {
            StopClientPhase::ReadingOffer | StopClientPhase::AwaitingReceipt => {
                let limit = if self.phase == StopClientPhase::ReadingOffer { OFFER_BYTES } else { RECEIPT_BYTES };
                let offered = limit - self.received;
                match self.stream.as_mut().ok_or(Error::WrongState)?.read(&mut self.raw[self.received..limit]) {
                    Ok(0) => return Err(ReviewerError::Io(io::ErrorKind::UnexpectedEof)),
                    Ok(count) if count <= offered => self.received += count,
                    Ok(_) => return Err(Error::InvalidInput.into()),
                    Err(error) if transient(error.kind()) => return Ok(StopClientProgress::Blocked),
                    Err(error) => return Err(ReviewerError::Io(error.kind())),
                }
                if self.received != limit { return Ok(StopClientProgress::Progress); }
                if self.phase == StopClientPhase::ReadingOffer {
                    let mut offer = [0; OFFER_BYTES]; offer.copy_from_slice(&self.raw[..OFFER_BYTES]);
                    self.binding = Some(StopBinding::decode_offer(&offer, self.expected, self.operation)?);
                    self.phase = StopClientPhase::NeedsDecision;
                    Ok(StopClientProgress::NeedsDecision)
                } else {
                    self.receipt = Some(StopControlReceipt::decode(&self.raw, self.binding.ok_or(Error::WrongState)?)?);
                    self.phase = StopClientPhase::Complete; self.stream = None;
                    Ok(StopClientProgress::Complete)
                }
            }
            StopClientPhase::NeedsDecision => Ok(StopClientProgress::NeedsDecision),
            StopClientPhase::SendingRequest => {
                let stream = self.stream.as_mut().ok_or(Error::WrongState)?;
                if self.written < OFFER_BYTES {
                    let offered = OFFER_BYTES - self.written;
                    self.attempted = true;
                    match stream.write(&self.raw[self.written..OFFER_BYTES]) {
                        Ok(0) => return Err(ReviewerError::Io(io::ErrorKind::WriteZero)),
                        Ok(count) if count <= offered => self.written += count,
                        Ok(_) => return Err(Error::InvalidInput.into()),
                        Err(error) if transient(error.kind()) => return Ok(StopClientProgress::Blocked),
                        Err(error) => return Err(ReviewerError::Io(error.kind())),
                    }
                    if self.written != OFFER_BYTES { return Ok(StopClientProgress::Progress); }
                }
                match stream.flush() {
                    Ok(()) => { self.received = 0; self.phase = StopClientPhase::AwaitingReceipt; Ok(StopClientProgress::Progress) }
                    Err(error) if transient(error.kind()) => Ok(StopClientProgress::Blocked),
                    Err(error) => Err(ReviewerError::Io(error.kind())),
                }
            }
            StopClientPhase::Complete => Ok(StopClientProgress::Complete),
            StopClientPhase::Failed => Err(Error::WrongState.into()),
        }
    }
}
