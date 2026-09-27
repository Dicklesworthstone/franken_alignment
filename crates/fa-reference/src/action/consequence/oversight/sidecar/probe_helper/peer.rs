//! Computed activation votes over the ORIGINAL helper client wire protocol.
//! The client owns its numerical evaluator: no public respond/verdict override,
//! hidden residual purchase, replacement input or mutable evaluator is exposed.
use super::{ProbeHelperReport, ProbeHelperStatus, ProbeHelperWork, SidecarProbeEvaluator};
use crate::action::consequence::oversight::helper_client::{
    ClientInterest, ClientPhase, ClientProgress, HelperClient,
};
use crate::action::consequence::oversight::helper_client_drive::MAX_CLIENT_DRIVE_STEPS;
use crate::action::consequence::oversight::helper_workers::{MAX_WORKER_SALT_BYTES, io::WorkerIoError};
use crate::round::Verdict;
use crate::Error;
use std::fmt;
use std::io::{Read, Write};

/// Length admission only; salt entropy/secrecy must be independently provisioned.
pub const MIN_PROBE_HELPER_SALT_BYTES: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeClientError { Protocol(WorkerIoError), Evaluation(Error), Cancelled, Interrupted }
impl fmt::Display for ProbeClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for ProbeClientError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeClientProgress { Protocol(ClientProgress), Judged(Verdict) }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeClientDrive {
    pub steps: usize,
    pub evaluations: usize,
    pub phase: ClientPhase,
    pub progress: Result<ProbeClientProgress, ProbeClientError>,
}

/// One connection, one numerical judgment and one frozen commit/reveal pair.
/// No socket reconnect, decoder replacement or fallback vote is available.
/// The original FNV reference commitment is NOT cryptographic authentication.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::sidecar::probe_helper::peer::ProbeHelperClient;
/// use fa_reference::round::Verdict;
/// fn override_vote(client: &mut ProbeHelperClient<std::io::Cursor<Vec<u8>>>) {
///     client.respond(Verdict::Allow, b"not-a-measurement");
/// }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::sidecar::probe_helper::peer::ProbeHelperClient;
/// fn bypass(client: &mut ProbeHelperClient<std::io::Cursor<Vec<u8>>>) { client.evaluator_mut(); }
/// ```
pub struct ProbeHelperClient<S> {
    client: Option<HelperClient<S>>,
    evaluator: SidecarProbeEvaluator,
    salt: Vec<u8>,
    failure: Option<ProbeClientError>,
    evaluations: usize,
}
impl<S> fmt::Debug for ProbeHelperClient<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProbeHelperClient").field("failure", &self.failure)
            .field("evaluations", &self.evaluations).finish_non_exhaustive()
    }
}
impl<S: Read + Write> ProbeHelperClient<S> {
    /// Generic I/O needs the caller's bounded/nonblocking contract. Construction
    /// sends no evidence or vote and performs no numerical evaluation.
    pub fn new(stream: S, evaluator: SidecarProbeEvaluator, salt: Vec<u8>) -> Result<Self, Error> {
        if !(MIN_PROBE_HELPER_SALT_BYTES..=MAX_WORKER_SALT_BYTES).contains(&salt.len()) {
            return Err(Error::Limit);
        }
        if evaluator.status() != ProbeHelperStatus::AwaitingInput { return Err(Error::WrongState); }
        let expected = evaluator.input_profile().clone();
        Ok(Self { client: Some(HelperClient::new(stream, expected)?), evaluator,
            salt, failure: None, evaluations: 0 })
    }
    pub fn phase(&self) -> ClientPhase {
        if self.failure.is_some() { ClientPhase::Failed }
        else { self.client.as_ref().map_or(ClientPhase::Failed, HelperClient::phase) }
    }
    pub fn interest(&self) -> ClientInterest {
        if self.failure.is_some() { ClientInterest::Finished }
        else { self.client.as_ref().map_or(ClientInterest::Finished, HelperClient::interest) }
    }
    pub fn failure(&self) -> Option<ProbeClientError> { self.failure }
    pub fn evaluations(&self) -> usize { self.evaluations }
    pub fn report(&self) -> Option<&ProbeHelperReport> { self.evaluator.report() }
    pub fn work(&self) -> ProbeHelperWork { self.evaluator.work() }

    /// One original I/O call OR one completely budgeted numerical evaluation.
    /// Decoding the request yields NeedsInference WITHOUT evaluating in that call.
    /// Scoring is not preemptive: its existing coordinate/product caps bound work,
    /// not wall time. Every judgment is yielded before a commitment byte is sent.
    pub fn step(&mut self) -> Result<ProbeClientProgress, ProbeClientError> {
        if let Some(error) = self.failure { self.salt.clear(); return Err(error); }
        if self.phase() == ClientPhase::ReplySent {
            return Ok(ProbeClientProgress::Protocol(ClientProgress::ReplySent));
        }
        self.failure = Some(ProbeClientError::Interrupted);
        // Move the connection into this stack frame. It closes on errors AND
        // unwinds, including a panic after a partial read/write. No older socket
        // offset survives for retry. Salt clearing is not secure zeroization.
        let mut client = self.client.take().ok_or(ProbeClientError::Interrupted)?;
        let result = self.step_once(&mut client);
        match result {
            Ok(progress) => { self.client = Some(client); self.failure = None; Ok(progress) }
            Err(error) => { self.failure = Some(error); self.salt.clear(); Err(error) }
        }
    }
    fn step_once(&mut self, client: &mut HelperClient<S>) -> Result<ProbeClientProgress, ProbeClientError> {
        if client.phase() != ClientPhase::NeedsInference {
            return client.step().map(ProbeClientProgress::Protocol).map_err(ProbeClientError::Protocol);
        }
        let input = client.input().ok_or(ProbeClientError::Protocol(WorkerIoError::Protocol(Error::Incomplete)))?;
        if self.salt.len() > input.salt_limit() {
            return Err(ProbeClientError::Protocol(WorkerIoError::Protocol(Error::Limit)));
        }
        self.evaluations += 1;
        let verdict = self.evaluator.evaluate(input).map_err(ProbeClientError::Evaluation)?;
        // Only the computed result reaches the original one-shot response slot.
        // Original respond freezes BOTH frames and cannot send the reveal early.
        client.respond(verdict, &self.salt)
            .map_err(|error| ProbeClientError::Protocol(WorkerIoError::Protocol(error)))?;
        self.salt.clear();
        Ok(ProbeClientProgress::Judged(verdict))
    }

    /// Stops on backpressure, input readiness, judgment, completion or error.
    /// State-machine calls are counted, not syscalls or floating-point operations.
    pub fn drive(&mut self, max_steps: usize) -> Result<ProbeClientDrive, Error> {
        if max_steps == 0 { return Err(Error::InvalidInput); }
        if max_steps > MAX_CLIENT_DRIVE_STEPS { return Err(Error::Limit); }
        let before = self.evaluations;
        if let Some(error) = self.failure {
            self.salt.clear();
            return Ok(ProbeClientDrive { steps: 0, evaluations: 0, phase: self.phase(), progress: Err(error) });
        }
        if self.phase() == ClientPhase::ReplySent {
            return Ok(ProbeClientDrive { steps: 0, evaluations: 0, phase: self.phase(),
                progress: Ok(ProbeClientProgress::Protocol(ClientProgress::ReplySent)) });
        }
        let mut report = ProbeClientDrive { steps: 0, evaluations: 0, phase: self.phase(),
            progress: Ok(ProbeClientProgress::Protocol(ClientProgress::Progress)) };
        for _ in 0..max_steps {
            report.steps += 1;
            report.progress = self.step(); report.phase = self.phase();
            report.evaluations = self.evaluations - before;
            if report.progress != Ok(ProbeClientProgress::Protocol(ClientProgress::Progress)) { break; }
        }
        Ok(report)
    }

    /// Close the owned connection without inventing a vote, refund or remote
    /// cancellation receipt. Already sent commitment bytes cannot be unsent;
    /// the original coordinator retains missing reveal/deadline obligations.
    pub fn cancel(&mut self) -> bool {
        if self.failure.is_some() { self.client.take(); self.salt.clear(); return false; }
        if self.phase() == ClientPhase::ReplySent { return false; }
        self.failure = Some(ProbeClientError::Cancelled);
        self.client.take(); self.salt.clear();
        true
    }
}

#[cfg(unix)]
impl ProbeHelperClient<std::os::unix::net::UnixStream> {
    /// A provisioned connection, not a listener, process launcher or authenticated
    /// source loader. Nonblocking mode is set before any protocol I/O.
    pub fn from_unix(stream: std::os::unix::net::UnixStream, evaluator: SidecarProbeEvaluator,
        salt: Vec<u8>) -> Result<Self, WorkerIoError>
    {
        stream.set_nonblocking(true).map_err(|error| WorkerIoError::Io(error.kind()))?;
        Self::new(stream, evaluator, salt).map_err(WorkerIoError::Protocol)
    }
}
