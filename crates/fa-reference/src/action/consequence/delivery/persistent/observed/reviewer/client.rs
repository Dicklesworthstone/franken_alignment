//! Reviewer-side client: decode an exact offer, wait for an explicit UI choice,
//! send it once, and distinguish receipt loss from an acknowledged operation.
//! No reviewer role, FileOversight handle or approval key exists in this client.
use super::{ReviewerError, REVIEW_IO_CHUNK, transient};
use super::wire::{ReviewDecision, ReviewPacket, ReviewReceipt, DECISION_BYTES,
    OFFER_HEADER_BYTES, RECEIPT_BYTES, offer_frame_len};
use crate::action::Scope;
use crate::action::consequence::oversight::human::HumanDisposition;
use crate::Error;
use std::fmt;
use std::io::{self, Read, Write};

/// Independently configured reviewer audience. These expected values must not
/// come from the received offer. They supplement, not replace, authentication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReviewerExpectation {
    pub reviewer: u64,
    pub scope: Scope,
    pub clock_domain: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewClientPhase { ReadingOffer, NeedsDecision, SendingDecision, AwaitingReceipt, Complete, Failed }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewClientProgress { Progress, Blocked, NeedsDecision, Complete }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewClientInterest { Readable, Writable, HumanDecision, Finished }

/// Bounded one-shot transport for an operator UI. Merely calling step never
/// approves anything. A sent decision is not a durable approval until its exact
/// receipt arrives, and even that receipt is not an effect permission.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::ReviewerClient;
/// fn recover(client: ReviewerClient<std::io::Cursor<Vec<u8>>>) { client.approval_key(); }
/// ```
pub struct ReviewerClient<S> {
    stream: Option<S>,
    expected: ReviewerExpectation,
    phase: ReviewClientPhase,
    raw: Vec<u8>,
    expected_bytes: usize,
    packet: Option<ReviewPacket>,
    decision: Option<ReviewDecision>,
    output: [u8; DECISION_BYTES],
    written: usize,
    receipt_raw: [u8; RECEIPT_BYTES],
    received: usize,
    receipt: Option<ReviewReceipt>,
    attempted: bool,
    failure: Option<ReviewerError>,
}
impl<S> fmt::Debug for ReviewerClient<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReviewerClient").field("phase", &self.phase)
            .field("attempted", &self.attempted).field("receipt", &self.receipt)
            .field("failure", &self.failure).finish_non_exhaustive()
    }
}
impl<S: Read + Write> ReviewerClient<S> {
    pub fn new(stream: S, expected: ReviewerExpectation) -> Result<Self, Error> {
        let scope = expected.scope;
        if [expected.reviewer, expected.clock_domain, scope.tenant, scope.principal,
            scope.run, scope.branch, scope.authority].contains(&0) { return Err(Error::InvalidInput); }
        Ok(Self { stream: Some(stream), expected, phase: ReviewClientPhase::ReadingOffer,
            raw: Vec::new(), expected_bytes: OFFER_HEADER_BYTES, packet: None, decision: None,
            output: [0; DECISION_BYTES], written: 0, receipt_raw: [0; RECEIPT_BYTES], received: 0,
            receipt: None, attempted: false, failure: None })
    }
    pub fn phase(&self) -> ReviewClientPhase { self.phase }
    pub fn packet(&self) -> Option<&ReviewPacket> { self.packet.as_ref() }
    pub fn receipt(&self) -> Option<ReviewReceipt> { self.receipt }
    pub fn decision(&self) -> Option<ReviewDecision> { self.decision }
    pub fn failure(&self) -> Option<&ReviewerError> { self.failure.as_ref() }
    /// Conservative after ANY attempted write, including a partial write or
    /// transport error. Never interpret this as refusal or safely retry a grant.
    pub fn outcome_unknown(&self) -> bool { self.attempted && self.receipt.is_none() }
    pub fn interest(&self) -> ReviewClientInterest {
        match self.phase {
            ReviewClientPhase::ReadingOffer | ReviewClientPhase::AwaitingReceipt => ReviewClientInterest::Readable,
            ReviewClientPhase::NeedsDecision => ReviewClientInterest::HumanDecision,
            ReviewClientPhase::SendingDecision => ReviewClientInterest::Writable,
            ReviewClientPhase::Complete | ReviewClientPhase::Failed => ReviewClientInterest::Finished,
        }
    }

    /// Call only for an explicit independent human UI action. The fully decoded,
    /// pinned packet is retained unchanged. A chosen decision cannot be edited
    /// or resubmitted, even before its first socket write. No default exists.
    pub fn respond(&mut self, decision: ReviewDecision) -> Result<(), Error> {
        if self.phase != ReviewClientPhase::NeedsDecision { return Err(Error::WrongState); }
        let packet = self.packet.as_ref().ok_or(Error::Incomplete)?;
        if packet.disposition() == HumanDisposition::Approved && decision != ReviewDecision::Revoke {
            return Err(Error::WrongState);
        }
        self.output = packet.decision_frame(decision);
        self.decision = Some(decision);
        self.phase = ReviewClientPhase::SendingDecision;
        Ok(())
    }

    /// One bounded read OR write and one final flush. Inference, human decisions,
    /// clock selection, reconnect and retries are never hidden inside this call.
    pub fn step(&mut self) -> Result<ReviewClientProgress, ReviewerError> {
        if let Some(error) = &self.failure { return Err(error.clone()); }
        let result = match self.phase {
            ReviewClientPhase::ReadingOffer => self.read_offer(),
            ReviewClientPhase::NeedsDecision => Ok(ReviewClientProgress::NeedsDecision),
            ReviewClientPhase::SendingDecision => self.send(),
            ReviewClientPhase::AwaitingReceipt => self.read_receipt(),
            ReviewClientPhase::Complete => Ok(ReviewClientProgress::Complete),
            ReviewClientPhase::Failed => Err(Error::WrongState.into()),
        };
        if let Err(error) = &result {
            self.failure = Some(error.clone()); self.phase = ReviewClientPhase::Failed;
            self.raw = Vec::new(); self.stream = None;
        }
        result
    }

    fn read_offer(&mut self) -> Result<ReviewClientProgress, ReviewerError> {
        let mut scratch = [0; REVIEW_IO_CHUNK];
        let offered = (self.expected_bytes - self.raw.len()).min(scratch.len());
        match self.stream.as_mut().ok_or(Error::WrongState)?.read(&mut scratch[..offered]) {
            Ok(0) => return Err(ReviewerError::Io(io::ErrorKind::UnexpectedEof)),
            Ok(count) if count <= offered => {
                self.raw.try_reserve(count).map_err(|_| Error::Limit)?;
                self.raw.extend_from_slice(&scratch[..count]);
            }
            Ok(_) => return Err(Error::InvalidInput.into()),
            Err(error) if transient(error.kind()) => return Ok(ReviewClientProgress::Blocked),
            Err(error) => return Err(ReviewerError::Io(error.kind())),
        }
        if self.raw.len() < self.expected_bytes { return Ok(ReviewClientProgress::Progress); }
        if self.expected_bytes == OFFER_HEADER_BYTES {
            self.expected_bytes = offer_frame_len(&self.raw)?;
            self.raw.try_reserve_exact(self.expected_bytes - self.raw.len()).map_err(|_| Error::Limit)?;
            return Ok(ReviewClientProgress::Progress);
        }
        let packet = ReviewPacket::decode(&self.raw)?;
        if packet.binding().reviewer != self.expected.reviewer || packet.clock_domain() != self.expected.clock_domain
            || packet.action().spec().scope != self.expected.scope { return Err(Error::Binding.into()); }
        self.packet = Some(packet); self.raw = Vec::new();
        self.phase = ReviewClientPhase::NeedsDecision;
        Ok(ReviewClientProgress::NeedsDecision)
    }

    fn send(&mut self) -> Result<ReviewClientProgress, ReviewerError> {
        let stream = self.stream.as_mut().ok_or(Error::WrongState)?;
        if self.written < self.output.len() {
            let offered = self.output.len() - self.written;
            self.attempted = true;
            match stream.write(&self.output[self.written..]) {
                Ok(0) => return Err(ReviewerError::Io(io::ErrorKind::WriteZero)),
                Ok(count) if count <= offered => self.written += count,
                Ok(_) => return Err(Error::InvalidInput.into()),
                Err(error) if transient(error.kind()) => return Ok(ReviewClientProgress::Blocked),
                Err(error) => return Err(ReviewerError::Io(error.kind())),
            }
            if self.written < self.output.len() { return Ok(ReviewClientProgress::Progress); }
        }
        match stream.flush() {
            Ok(()) => { self.phase = ReviewClientPhase::AwaitingReceipt; Ok(ReviewClientProgress::Progress) }
            Err(error) if transient(error.kind()) => Ok(ReviewClientProgress::Blocked),
            Err(error) => Err(ReviewerError::Io(error.kind())),
        }
    }

    fn read_receipt(&mut self) -> Result<ReviewClientProgress, ReviewerError> {
        let remaining = &mut self.receipt_raw[self.received..];
        let offered = remaining.len();
        match self.stream.as_mut().ok_or(Error::WrongState)?.read(remaining) {
            Ok(0) => return Err(ReviewerError::Io(io::ErrorKind::UnexpectedEof)),
            Ok(count) if count <= offered => self.received += count,
            Ok(_) => return Err(Error::InvalidInput.into()),
            Err(error) if transient(error.kind()) => return Ok(ReviewClientProgress::Blocked),
            Err(error) => return Err(ReviewerError::Io(error.kind())),
        }
        if self.received < RECEIPT_BYTES { return Ok(ReviewClientProgress::Progress); }
        let receipt = self.packet.as_ref().ok_or(Error::Incomplete)?.receipt(&self.receipt_raw)?;
        if Some(receipt.decision) != self.decision { return Err(Error::Binding.into()); }
        self.receipt = Some(receipt); self.phase = ReviewClientPhase::Complete; self.stream = None;
        Ok(ReviewClientProgress::Complete)
    }
}

#[cfg(unix)]
impl ReviewerClient<std::os::unix::net::UnixStream> {
    /// Supply an already connected/authenticated private endpoint. This neither
    /// searches for a server nor trusts audience IDs provided by that server.
    pub fn from_unix(stream: std::os::unix::net::UnixStream, expected: ReviewerExpectation) -> Result<Self, ReviewerError> {
        stream.set_nonblocking(true).map_err(|error| ReviewerError::Io(error.kind()))?;
        Ok(Self::new(stream, expected)?)
    }
}
