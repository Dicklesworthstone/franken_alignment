//! Independent reviewer transport over the ORIGINAL durable two-key gate.
//! Only a separately provisioned reviewer stream may be attached. This module
//! provides framing and role binding, NOT peer authentication or a new policy.
pub mod wire;

use super::{FileHumanPermit, FileHumanRequest, FileHumanReviewer, FileOversight, JournalError};
use crate::action::{ActionState, ElapsedTick};
use crate::action::consequence::Consequence;
use crate::action::consequence::oversight::human::HumanDisposition;
use crate::Error;
use std::fmt;
use std::io::{self, Read, Write};
use std::rc::Rc;
use wire::{ReviewBinding, ReviewDecision, ReviewPacket, ReviewReceipt, DECISION_BYTES, RECEIPT_BYTES};

pub const REVIEW_IO_CHUNK: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReviewerError { Protocol(Error), Journal(JournalError), Io(io::ErrorKind) }
impl From<Error> for ReviewerError {
    fn from(error: Error) -> Self { Self::Protocol(error) }
}
impl From<JournalError> for ReviewerError {
    fn from(error: JournalError) -> Self { Self::Journal(error) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewerPhase { Offering, AwaitingDecision, SendingReceipt, Complete, Failed }

/// Delivered exactly once to the HOST, never serialized to the reviewer socket.
/// A subsequent lost receipt cannot roll back this committed native transition.
#[derive(Debug)]
pub struct ReviewApplication {
    pub receipt: ReviewReceipt,
    pub approval: Option<FileHumanPermit>,
}

#[derive(Debug)]
pub enum ReviewerProgress { Progress, Blocked, Applied(ReviewApplication), Complete }

/// One immutable native request and one framed reviewer decision. No request
/// substitution, model judgment, key recovery, automatic retry or dispatch API.
/// Generic streams require a host-supplied nonblocking/bounded I/O contract.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::reviewer::ReviewerConnection;
/// fn authority(channel: ReviewerConnection<std::io::Cursor<Vec<u8>>>) { channel.dispatch(); }
/// ```
pub struct ReviewerConnection<S> {
    stream: Option<S>,
    request: FileHumanRequest,
    binding: ReviewBinding,
    phase: ReviewerPhase,
    output: Vec<u8>,
    written: usize,
    input: [u8; DECISION_BYTES],
    received: usize,
    committed: Option<ReviewReceipt>,
    failure: Option<ReviewerError>,
}
impl<S> fmt::Debug for ReviewerConnection<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReviewerConnection").field("request", &self.binding.request)
            .field("phase", &self.phase).field("committed", &self.committed)
            .field("failure", &self.failure).finish_non_exhaustive()
    }
}

fn check_owner(host: &FileOversight, reviewer: &FileHumanReviewer, request: &FileHumanRequest) -> Result<(), Error> {
    if !Rc::ptr_eq(&request.issuer, &host.issuer) || !Rc::ptr_eq(&reviewer.issuer, &host.issuer)
        || reviewer.reviewer_id() != request.evidence().reviewer_id() { return Err(Error::Binding); }
    Ok(())
}

impl<S: Read + Write> ReviewerConnection<S> {
    /// The trusted host supplies a fresh nonzero session nonce, unique to this
    /// offer, and an already authenticated/isolated reviewer stream. A nonce is
    /// an exact correlation value, NOT a signature, credential or proof of entropy.
    /// This sends nothing and cannot reserve or approve an effect.
    pub fn new(host: &FileOversight, reviewer: &FileHumanReviewer, request: FileHumanRequest,
        stream: S, session: [u8; 32]) -> Result<Self, ReviewerError>
    {
        check_owner(host, reviewer, &request)?;
        if host.storage_failure().is_some() { return Err(JournalError::Unavailable.into()); }
        let disposition = host.human_status(request.evidence().id())?.disposition;
        if !matches!(disposition, HumanDisposition::Pending | HumanDisposition::Approved) {
            return Err(Error::WrongState.into());
        }
        let packet = ReviewPacket::capture(&request, host.profile.delivery.clock_domain,
            host.revision(), disposition, session)?;
        let output = packet.encode()?;
        Ok(Self { stream: Some(stream), request, binding: packet.binding(), phase: ReviewerPhase::Offering,
            output, written: 0, input: [0; DECISION_BYTES], received: 0, committed: None, failure: None })
    }
    pub fn phase(&self) -> ReviewerPhase { self.phase }
    pub fn committed(&self) -> Option<ReviewReceipt> { self.committed }
    pub fn failure(&self) -> Option<&ReviewerError> { self.failure.as_ref() }
    pub fn expires_at(&self) -> ElapsedTick { self.request.evidence().expires_at() }

    /// At most one bounded read OR write plus a final flush. No reply is read
    /// before the entire original packet has been written and flushed. This is
    /// evidence of transmission, not proof that a human read or understood it.
    ///
    /// Approval samples the trusted clock AFTER the complete decision arrives,
    /// journals that observation, and checks the original current input/control
    /// cut before calling FileHumanReviewer::approve. Reject/revoke never need a
    /// healthy evidence provider or a fresh clock. Original dispatch still checks
    /// both keys, fresh policy, evidence, expiry and endpoint preconditions.
    pub fn step<F>(&mut self, host: &mut FileOversight, reviewer: &FileHumanReviewer, mut clock: F)
        -> Result<ReviewerProgress, ReviewerError>
    where F: FnMut() -> ElapsedTick {
        if let Some(error) = &self.failure { return Err(error.clone()); }
        // A wrongly routed call does not destroy the correctly bound channel.
        check_owner(host, reviewer, &self.request)?;
        if self.phase == ReviewerPhase::Complete { return Ok(ReviewerProgress::Complete); }
        let result = self.step_inner(host, reviewer, &mut clock);
        if let Err(error) = &result {
            self.failure = Some(error.clone()); self.phase = ReviewerPhase::Failed;
            self.output = Vec::new(); self.stream = None;
        }
        result
    }

    fn step_inner<F>(&mut self, host: &mut FileOversight, reviewer: &FileHumanReviewer, clock: &mut F)
        -> Result<ReviewerProgress, ReviewerError>
    where F: FnMut() -> ElapsedTick {
        if self.committed.is_none() && host.storage_failure().is_some() {
            return Err(JournalError::Unavailable.into());
        }
        match self.phase {
            ReviewerPhase::Offering | ReviewerPhase::SendingReceipt => self.send(),
            ReviewerPhase::AwaitingDecision => {
                let remaining = &mut self.input[self.received..];
                let offered = remaining.len().min(REVIEW_IO_CHUNK);
                let read = self.stream.as_mut().ok_or(Error::WrongState)?.read(&mut remaining[..offered]);
                match read {
                    Ok(0) => return Err(ReviewerError::Io(io::ErrorKind::UnexpectedEof)),
                    Ok(count) if count <= offered => self.received += count,
                    Ok(_) => return Err(Error::InvalidInput.into()),
                    Err(error) if transient(error.kind()) => return Ok(ReviewerProgress::Blocked),
                    Err(error) => return Err(ReviewerError::Io(error.kind())),
                }
                if self.received != DECISION_BYTES { return Ok(ReviewerProgress::Progress); }
                let decision = wire::decode_decision(&self.input, self.binding)?;
                self.apply(host, reviewer, clock, decision)
            }
            ReviewerPhase::Complete => Ok(ReviewerProgress::Complete),
            ReviewerPhase::Failed => Err(Error::WrongState.into()),
        }
    }

    fn send(&mut self) -> Result<ReviewerProgress, ReviewerError> {
        let stream = self.stream.as_mut().ok_or(Error::WrongState)?;
        if self.written < self.output.len() {
            let end = self.output.len().min(self.written + REVIEW_IO_CHUNK);
            let offered = end - self.written;
            match stream.write(&self.output[self.written..end]) {
                Ok(0) => return Err(ReviewerError::Io(io::ErrorKind::WriteZero)),
                Ok(count) if count <= offered => self.written += count,
                Ok(_) => return Err(Error::InvalidInput.into()),
                Err(error) if transient(error.kind()) => return Ok(ReviewerProgress::Blocked),
                Err(error) => return Err(ReviewerError::Io(error.kind())),
            }
            if self.written < self.output.len() { return Ok(ReviewerProgress::Progress); }
        }
        match stream.flush() {
            Ok(()) => {
                self.output = Vec::new(); self.written = 0;
                if self.phase == ReviewerPhase::Offering {
                    self.phase = ReviewerPhase::AwaitingDecision;
                    Ok(ReviewerProgress::Progress)
                } else {
                    self.phase = ReviewerPhase::Complete; self.stream = None;
                    Ok(ReviewerProgress::Complete)
                }
            }
            Err(error) if transient(error.kind()) => Ok(ReviewerProgress::Blocked),
            Err(error) => Err(ReviewerError::Io(error.kind())),
        }
    }

    fn apply<F>(&mut self, host: &mut FileOversight, reviewer: &FileHumanReviewer,
        clock: &mut F, decision: ReviewDecision) -> Result<ReviewerProgress, ReviewerError>
    where F: FnMut() -> ElapsedTick {
        // Allocate the small receipt buffer BEFORE any authority transition.
        self.output.try_reserve_exact(RECEIPT_BYTES).map_err(|_| Error::Limit)?;
        let approval = match decision {
            ReviewDecision::Approve => {
                let now = clock();
                if !host.clock_ready() || host.inspect().control.ledger.elapsed != Some(now) {
                    host.observe_time(host.revision(), now)?;
                }
                let evidence = self.request.evidence();
                let control = host.inspect().control;
                if evidence.control_sequence() != control.sequence
                    || evidence.action().spec().policy_epoch != control.ledger.epoch
                    || evidence.input_revision() != host.current_reference(evidence.attempt(), evidence.inputs())?
                    || control.decisions.get(&evidence.attempt()) != Some(&Consequence::Continue)
                    || !matches!(control.ledger.stages.get(&evidence.attempt()), Some(ActionState::Reviewing | ActionState::Authorized))
                { return Err(Error::Stale.into()); }
                let revision = host.revision();
                Some(reviewer.approve(host, revision, &self.request)?)
            }
            ReviewDecision::Reject => {
                let revision = host.revision();
                reviewer.reject(host, revision, &self.request)?; None
            }
            ReviewDecision::Revoke => {
                let revision = host.revision();
                reviewer.revoke(host, revision, &self.request)?; None
            }
        };
        let receipt = ReviewReceipt { binding: self.binding, decision, revision: host.revision() };
        self.output.extend_from_slice(&receipt.encode());
        self.committed = Some(receipt);
        self.phase = ReviewerPhase::SendingReceipt;
        // No later transport failure reruns this operation or regenerates a key.
        Ok(ReviewerProgress::Applied(ReviewApplication { receipt, approval }))
    }
}

pub(super) fn transient(kind: io::ErrorKind) -> bool {
    matches!(kind, io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted)
}

#[cfg(unix)]
impl ReviewerConnection<std::os::unix::net::UnixStream> {
    pub fn from_unix(host: &FileOversight, reviewer: &FileHumanReviewer, request: FileHumanRequest,
        stream: std::os::unix::net::UnixStream, session: [u8; 32]) -> Result<Self, ReviewerError>
    {
        stream.set_nonblocking(true).map_err(|error| ReviewerError::Io(error.kind()))?;
        Self::new(host, reviewer, request, stream, session)
    }
}
