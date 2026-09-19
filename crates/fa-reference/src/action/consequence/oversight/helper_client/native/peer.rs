//! Native inference on the ORIGINAL partial-I/O helper client. One connection,
//! one frozen input, one native evaluation, one immutable commit/reveal pair.

#[cfg(test)]
mod tests;

use super::{NativeEvaluationError, NativeEvaluationProgress, NativeEvaluationStatus, NativeEvaluator, TextGenerationReport};
use super::super::{ClientInterest, ClientPhase, ClientProgress, HelperClient};
use super::super::super::helper_client_drive::MAX_CLIENT_DRIVE_STEPS;
use super::super::super::helper_workers::{MAX_WORKER_SALT_BYTES, io::WorkerIoError, wire::WorkerInput};
use crate::round::Verdict;
use crate::Error;
use std::fmt;
use std::io::{Read, Write};

/// Length admission only. Entropy, secrecy and independent provisioning remain
/// host requirements; the numerical sampler is NEVER used to generate this salt.
pub const MIN_NATIVE_SALT_BYTES: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeClientError {
    Protocol(WorkerIoError),
    Inference(NativeEvaluationError),
    Cancelled,
    /// Set before entering transport/model code, including across caught unwinds.
    Interrupted,
}
impl fmt::Display for NativeClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for NativeClientError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeClientProgress {
    Protocol(ClientProgress),
    /// At most one native token computed; no partial answer or vote is exposed.
    Inference(NativeEvaluationProgress),
    /// Native result frozen into BOTH original frames; not yet written/accepted.
    Judged(Verdict),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeClientDrive {
    /// Original state-machine calls, not syscalls, FLOPs or elapsed-clock ticks.
    pub steps: usize,
    /// New evaluation attempts, not the number of token steps in this drive.
    pub evaluations: usize,
    pub phase: ClientPhase,
    pub progress: Result<NativeClientProgress, NativeClientError>,
}

/// The unwrapped client/evaluator never escape. A caller cannot replace a vote,
/// seed, input or salt after seeing model output. Socket readiness is scheduling
/// advice, not source authentication or a receipt from the congress.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::helper_client::native::peer::NativeHelperClient;
/// use fa_reference::round::Verdict;
/// fn forge(client: &mut NativeHelperClient<std::io::Cursor<Vec<u8>>>) {
///     client.respond(Verdict::Allow, b"unearned");
/// }
/// ```
pub struct NativeHelperClient<S> {
    client: HelperClient<S>,
    evaluator: NativeEvaluator,
    salt: Vec<u8>,
    failure: Option<NativeClientError>,
    evaluations: usize,
}
impl<S> fmt::Debug for NativeHelperClient<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeHelperClient").field("failure", &self.failure)
            .field("evaluations", &self.evaluations).finish_non_exhaustive()
    }
}
impl<S: Read + Write> NativeHelperClient<S> {
    pub fn new(stream: S, evaluator: NativeEvaluator, salt: Vec<u8>) -> Result<Self, Error> {
        if !(MIN_NATIVE_SALT_BYTES..=MAX_WORKER_SALT_BYTES).contains(&salt.len()) {
            return Err(Error::Limit);
        }
        if evaluator.status() != NativeEvaluationStatus::AwaitingInput { return Err(Error::WrongState); }
        let client = HelperClient::new(stream, evaluator.policy().input_profile.clone())?;
        Ok(Self { client, evaluator, salt, failure: None, evaluations: 0 })
    }
    pub fn phase(&self) -> ClientPhase {
        if self.failure.is_some() { ClientPhase::Failed } else { self.client.phase() }
    }
    pub fn interest(&self) -> ClientInterest {
        if self.failure.is_some() { ClientInterest::Finished } else { self.client.interest() }
    }
    pub fn failure(&self) -> Option<NativeClientError> { self.failure }
    pub fn input(&self) -> Option<&WorkerInput> { self.client.input() }
    pub fn report(&self) -> Option<&TextGenerationReport> { self.evaluator.report() }
    pub fn evaluations(&self) -> usize { self.evaluations }
    pub fn sampled_draws(&self) -> u64 { self.evaluator.sampled_draws() }
    pub fn evaluation_progress(&self) -> NativeEvaluationProgress { self.evaluator.progress() }

    /// One original I/O step OR at most one native monitored token. Complete
    /// input decoding yields NeedsInference without computing in that I/O call.
    /// The first inference step admits the entire request, then advances once.
    /// Every later token yields Inference until the reviewed terminal result.
    /// There is no executor or preemption inside a token/encoding operation.
    pub fn step(&mut self) -> Result<NativeClientProgress, NativeClientError> {
        self.step_with(|| {})
    }

    // Private causal interruption seam; no public transport/inference callback.
    fn step_with<F: FnOnce()>(&mut self, after_step: F) -> Result<NativeClientProgress, NativeClientError> {
        if let Some(error) = self.failure {
            self.release_evaluation();
            return Err(error);
        }
        if self.client.phase() == ClientPhase::ReplySent {
            return Ok(NativeClientProgress::Protocol(ClientProgress::ReplySent));
        }
        // A write may have reached the peer before a caught unwind. Neither its
        // frame nor inference is allowed to restart from an older offset/result.
        self.failure = Some(NativeClientError::Interrupted);
        let result = self.step_once();
        after_step();
        self.failure = result.as_ref().err().copied();
        if self.failure.is_some() { self.release_evaluation(); }
        result
    }
    fn step_once(&mut self) -> Result<NativeClientProgress, NativeClientError> {
        if self.client.phase() != ClientPhase::NeedsInference {
            return self.client.step().map(NativeClientProgress::Protocol).map_err(NativeClientError::Protocol);
        }
        let input = self.client.input().ok_or(NativeClientError::Protocol(WorkerIoError::Protocol(Error::Incomplete)))?;
        if self.salt.len() > input.salt_limit() {
            return Err(NativeClientError::Protocol(WorkerIoError::Protocol(Error::Limit)));
        }
        // Admit once, never once per poll. The original client retains the exact
        // request while its one-shot response slot remains NeedsInference.
        if self.evaluator.status() == NativeEvaluationStatus::AwaitingInput {
            self.evaluations += 1;
            self.evaluator.begin(input).map_err(NativeClientError::Inference)?;
        }
        let progress = self.evaluator.advance(self.evaluator.position()).map_err(NativeClientError::Inference)?;
        let verdict = match progress.status {
            NativeEvaluationStatus::Running => return Ok(NativeClientProgress::Inference(progress)),
            NativeEvaluationStatus::Judged(verdict) => verdict,
            _ => return Err(NativeClientError::Inference(Error::WrongState.into())),
        };
        self.client.respond(verdict, &self.salt)
            .map_err(|error| NativeClientError::Protocol(WorkerIoError::Protocol(error)))?;
        // The original client now retains the one reveal frame. Erasing this
        // redundant logical copy is not a zeroization or secure-memory claim.
        self.release_evaluation();
        Ok(NativeClientProgress::Judged(verdict))
    }

    /// Bounded caller-driven pumping. Yield on backpressure, input readiness,
    /// EVERY native token, native judgment, reply completion or failure. Even
    /// drive(MAX_CLIENT_DRIVE_STEPS) cannot compute a whole prompt without yielding.
    /// Attempts count once; evaluation_progress retains actual native work.
    pub fn drive(&mut self, max_steps: usize) -> Result<NativeClientDrive, Error> {
        if max_steps == 0 { return Err(Error::InvalidInput); }
        if max_steps > MAX_CLIENT_DRIVE_STEPS { return Err(Error::Limit); }
        let before = self.evaluations;
        let initial = if let Some(error) = self.failure { Some(Err(error)) }
            else if self.phase() == ClientPhase::ReplySent {
                Some(Ok(NativeClientProgress::Protocol(ClientProgress::ReplySent)))
            } else { None };
        if let Some(progress) = initial {
            if self.failure.is_some() { self.release_evaluation(); }
            return Ok(NativeClientDrive { steps: 0, evaluations: 0, phase: self.phase(), progress });
        }
        let mut report = NativeClientDrive { steps: 0, evaluations: 0, phase: self.phase(),
            progress: Ok(NativeClientProgress::Protocol(ClientProgress::Progress)) };
        for _ in 0..max_steps {
            report.steps += 1;
            report.progress = self.step();
            report.phase = self.phase();
            report.evaluations = self.evaluations - before;
            if report.progress != Ok(NativeClientProgress::Protocol(ClientProgress::Progress)) { break; }
        }
        Ok(report)
    }

    /// Terminal local withdrawal, not a vote, remote cancellation or refund.
    /// Already sent bytes cannot be unsent. No new bytes or inference follow.
    /// Drop this owner to close its transport; supervisor absence/deadline rules
    /// still decide the round, with the original member in the denominator.
    pub fn cancel(&mut self) -> bool {
        self.release_evaluation();
        if self.failure.is_some() || self.client.phase() == ClientPhase::ReplySent { return false; }
        self.failure = Some(NativeClientError::Cancelled);
        true
    }

    // Destroy unfinished native ownership; retain its original counters/report.
    // A caught unwind is still Interrupted, never relabeled as clean cancellation.
    fn release_evaluation(&mut self) {
        self.evaluator.cancel();
        self.salt.clear();
    }
}

#[cfg(unix)]
impl NativeHelperClient<std::os::unix::net::UnixStream> {
    pub fn from_unix(stream: std::os::unix::net::UnixStream, evaluator: NativeEvaluator,
        salt: Vec<u8>) -> Result<Self, WorkerIoError>
    {
        stream.set_nonblocking(true).map_err(|error| WorkerIoError::Io(error.kind()))?;
        Self::new(stream, evaluator, salt).map_err(WorkerIoError::Protocol)
    }
}
#[cfg(unix)]
impl std::os::fd::AsFd for NativeHelperClient<std::os::unix::net::UnixStream> {
    fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> { std::os::fd::AsFd::as_fd(&self.client) }
}
