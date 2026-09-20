//! Independently authenticated stop-only transport, available before human review.
//! The SAME durable driver performs stop and drain. No grant, evidence reader,
//! alternative ledger, autonomous task or new effect capability exists here.
mod wire;
mod client;
pub use wire::{StopBinding, StopControlReceipt, StopStatus};
pub use client::{StopClient, StopClientPhase, StopClientProgress};
use wire::{OFFER_BYTES, RECEIPT_BYTES};
use super::super::{ReviewerError, transient};
use super::super::client::ReviewerExpectation;
use super::super::super::{FileHumanReviewer, FileOversight, JournalError};
use super::super::super::driver::{CapacityDrain, CapacityStop, FileSupervisedDriver};
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::StopRequest;
use crate::Error;
use std::io::{self, Read, Write};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopControlPhase { Offering, AwaitingRequest, Applying, SendingReceipt, Complete, Failed }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopControlProgress { Progress, Blocked, Applied, Complete }

/// The exact native result remains available even when transport loses its reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StopApplication {
    pub result: Result<CapacityStop, JournalError>,
    pub receipt: StopControlReceipt,
}

/// One owner-bound, nonce-bound stop exchange. Generic streams require an
/// independently authenticated, bounded/nonblocking transport supplied by the
/// embedding supervisor. The Linux verified-socket conversion enforces that
/// boundary before protocol I/O in the runnable profile.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::control::StopControlConnection;
/// fn grant(channel: StopControlConnection<std::io::Cursor<Vec<u8>>>) { channel.approve(); }
/// ```
pub struct StopControlConnection<S> {
    stream: Option<S>,
    issuer: Rc<()>,
    binding: StopBinding,
    phase: StopControlPhase,
    output: [u8; RECEIPT_BYTES],
    output_len: usize,
    written: usize,
    input: [u8; OFFER_BYTES],
    received: usize,
    application: Option<StopApplication>,
    failure: Option<ReviewerError>,
}
impl<S: Read + Write> StopControlConnection<S> {
    pub fn new(host: &FileOversight, reviewer: &FileHumanReviewer, operation: u64,
        stream: S, session: [u8; 32]) -> Result<Self, ReviewerError>
    {
        if !Rc::ptr_eq(&host.issuer, &reviewer.issuer) { return Err(Error::Binding.into()); }
        if host.storage_failure().is_some() { return Err(JournalError::Unavailable.into()); }
        if host.journal_capacity()?.reserve().is_none() { return Err(Error::Incomplete.into()); }
        let binding = StopBinding { expected: ReviewerExpectation { reviewer: reviewer.reviewer_id(),
            scope: host.profile.delivery.scope, clock_domain: host.profile.delivery.clock_domain }, operation, session };
        let offer = binding.offer()?;
        let mut output = [0; RECEIPT_BYTES]; output[..OFFER_BYTES].copy_from_slice(&offer);
        Ok(Self { stream: Some(stream), issuer: Rc::clone(&host.issuer), binding,
            phase: StopControlPhase::Offering, output, output_len: OFFER_BYTES, written: 0,
            input: [0; OFFER_BYTES], received: 0, application: None, failure: None })
    }
    pub fn phase(&self) -> StopControlPhase { self.phase }
    pub fn application(&self) -> Option<&StopApplication> { self.application.as_ref() }
    pub fn failure(&self) -> Option<&ReviewerError> { self.failure.as_ref() }

    /// One bounded read/write per call. A complete exact stop request invokes
    /// native stop BEFORE the fallible clock/drain. Applying is latched before
    /// entering that operation: a caught unwind cannot cause another application.
    pub fn step<F>(&mut self, driver: &mut FileSupervisedDriver, mut clock: F)
        -> Result<StopControlProgress, ReviewerError>
    where F: FnMut() -> ElapsedTick {
        if let Some(error) = &self.failure { return Err(error.clone()); }
        {
            let host = driver.supervisor().host()?;
            if !Rc::ptr_eq(&host.issuer, &self.issuer) { return Err(Error::Binding.into()); }
        }
        let result = self.step_inner(driver, &mut clock);
        if let Err(error) = &result {
            self.failure = Some(error.clone()); self.phase = StopControlPhase::Failed; self.stream = None;
        }
        result
    }
    fn step_inner<F>(&mut self, driver: &mut FileSupervisedDriver, clock: &mut F)
        -> Result<StopControlProgress, ReviewerError>
    where F: FnMut() -> ElapsedTick {
        match self.phase {
            StopControlPhase::Offering | StopControlPhase::SendingReceipt => self.send(),
            StopControlPhase::AwaitingRequest => {
                let offered = OFFER_BYTES - self.received;
                match self.stream.as_mut().ok_or(Error::WrongState)?.read(&mut self.input[self.received..]) {
                    Ok(0) => return Err(ReviewerError::Io(io::ErrorKind::UnexpectedEof)),
                    Ok(count) if count <= offered => self.received += count,
                    Ok(_) => return Err(Error::InvalidInput.into()),
                    Err(error) if transient(error.kind()) => return Ok(StopControlProgress::Blocked),
                    Err(error) => return Err(ReviewerError::Io(error.kind())),
                }
                if self.received != OFFER_BYTES { return Ok(StopControlProgress::Progress); }
                if self.input != self.binding.request()? { return Err(Error::Binding.into()); }
                self.phase = StopControlPhase::Applying;
                let result = self.apply(driver, clock);
                let receipt = StopControlReceipt::project(self.binding, &result);
                // Retain the acknowledged native stop before any receipt I/O.
                self.application = Some(StopApplication { result, receipt });
                self.output = receipt.encode()?; self.output_len = RECEIPT_BYTES; self.written = 0;
                self.phase = StopControlPhase::SendingReceipt;
                Ok(StopControlProgress::Applied)
            }
            StopControlPhase::Complete => Ok(StopControlProgress::Complete),
            StopControlPhase::Applying | StopControlPhase::Failed => Err(Error::WrongState.into()),
        }
    }
    fn apply<F>(&self, driver: &mut FileSupervisedDriver, clock: &mut F) -> Result<CapacityStop, JournalError>
    where F: FnMut() -> ElapsedTick {
        let request = {
            let host = driver.supervisor().host()?;
            let state = host.inspect();
            if let Some(receipt) = state.stop {
                if receipt.request().operation != self.binding.operation { return Err(Error::Duplicate.into()); }
                receipt.request()
            } else {
                StopRequest { operation: self.binding.operation, expected_control_sequence: state.control.sequence,
                    expected_authority_epoch: state.control.ledger.epoch }
            }
        };
        driver.stop_with_recovery_reserve(request, clock)
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
                    self.phase = StopControlPhase::Complete; self.stream = None;
                    Ok(StopControlProgress::Complete)
                }
            }
            Err(error) if transient(error.kind()) => Ok(StopControlProgress::Blocked),
            Err(error) => Err(ReviewerError::Io(error.kind())),
        }
    }
}

#[cfg(test)]
mod tests;
