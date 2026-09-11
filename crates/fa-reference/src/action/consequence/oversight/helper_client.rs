//! Worker-side client for the existing helper wire and supervisor I/O engine.
//!
//! Decoding never chooses a verdict. The worker supplies one result after using
//! the exact, profile-matched input. Commitment and reveal then share one frozen
//! result; transport errors cannot ask inference to silently choose another.

use super::helper_workers::io::WorkerIoError;
use super::helper_workers::wire::{WorkerInput, REQUEST_HEADER_BYTES, REVEAL_REQUEST,
    decode_request, request_frame_len};
use crate::full_input::{InputProfileBinding, MAX_PROFILE_BYTES};
use crate::round::Verdict;
use crate::Error;
use std::fmt;
use std::io::{self, Read, Write};

pub const CLIENT_READ_CHUNK: usize = 4_096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientPhase { ReadingRequest, NeedsInference, SendingCommitment, AwaitingReveal, SendingReveal, ReplySent, Failed }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientProgress { Progress, Blocked, NeedsInference, ReplySent }

/// Scheduling advice, never evidence that a model ran or a vote was accepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientInterest { Readable, Writable, Inference, Finished }

/// One connected worker, one exact input and one immutable response. This owns
/// no helper port, coordinator, broker, provider snapshot or production rights.
/// Generic streams require a caller-supplied bounded/nonblocking I/O contract.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::helper_client::HelperClient;
/// fn authority(worker: HelperClient<std::io::Cursor<Vec<u8>>>) { let _ = worker.broker_mut(); }
/// ```
pub struct HelperClient<S> {
    stream: S,
    expected: InputProfileBinding,
    phase: ClientPhase,
    raw: Vec<u8>,
    expected_bytes: usize,
    input: Option<WorkerInput>,
    output: Vec<u8>,
    reveal: Vec<u8>,
    written: usize,
    failure: Option<WorkerIoError>,
}

impl<S> fmt::Debug for HelperClient<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HelperClient").field("phase", &self.phase)
            .field("buffered_bytes", &self.raw.len()).field("failure", &self.failure)
            .finish_non_exhaustive()
    }
}

impl<S: Read + Write> HelperClient<S> {
    pub fn new(stream: S, expected: InputProfileBinding) -> Result<Self, Error> {
        if expected.profile_bytes.len() > MAX_PROFILE_BYTES { return Err(Error::Limit); }
        Ok(Self { stream, expected, phase: ClientPhase::ReadingRequest,
            raw: Vec::with_capacity(REQUEST_HEADER_BYTES), expected_bytes: REQUEST_HEADER_BYTES,
            input: None, output: Vec::new(), reveal: Vec::new(), written: 0, failure: None })
    }

    pub fn phase(&self) -> ClientPhase { self.phase }
    pub fn failure(&self) -> Option<WorkerIoError> { self.failure }
    /// Available only after the complete request passes the original parser and
    /// exact expected-profile comparison. Remains historical data after failure.
    pub fn input(&self) -> Option<&WorkerInput> { self.input.as_ref() }

    pub fn interest(&self) -> ClientInterest {
        match self.phase {
            ClientPhase::ReadingRequest | ClientPhase::AwaitingReveal => ClientInterest::Readable,
            ClientPhase::SendingCommitment | ClientPhase::SendingReveal => ClientInterest::Writable,
            ClientPhase::NeedsInference => ClientInterest::Inference,
            ClientPhase::ReplySent | ClientPhase::Failed => ClientInterest::Finished,
        }
    }

    /// Freeze BOTH original wire frames before any state changes or I/O. Invalid
    /// salt lengths retain the uncommitted input. Once chosen, even before the
    /// first write, no API replaces the verdict or salt. No default vote exists.
    pub fn respond(&mut self, verdict: Verdict, salt: &[u8]) -> Result<(), Error> {
        if self.phase != ClientPhase::NeedsInference { return Err(Error::WrongState); }
        let input = self.input.as_ref().ok_or(Error::Incomplete)?;
        let commitment = input.commitment_frame(verdict, salt)?;
        let reveal = input.reveal_frame(verdict, salt)?;
        self.output = commitment.to_vec();
        self.reveal = reveal;
        self.written = 0;
        self.phase = ClientPhase::SendingCommitment;
        Ok(())
    }

    /// At most one read, or one write followed by at most one flush. WouldBlock
    /// and Interrupted preserve offsets and never rerun inference or a command.
    /// A terminal error is latched; future steps perform no I/O.
    pub fn step(&mut self) -> Result<ClientProgress, WorkerIoError> {
        if let Some(error) = self.failure { return Err(error); }
        let result = match self.phase {
            ClientPhase::ReadingRequest => self.read_request(),
            ClientPhase::NeedsInference => Ok(ClientProgress::NeedsInference),
            ClientPhase::SendingCommitment | ClientPhase::SendingReveal => self.send(),
            ClientPhase::AwaitingReveal => self.read_reveal_request(),
            ClientPhase::ReplySent => Ok(ClientProgress::ReplySent),
            ClientPhase::Failed => Err(WorkerIoError::Protocol(Error::WrongState)),
        };
        if let Err(error) = result {
            self.failure = Some(error);
            self.phase = ClientPhase::Failed;
            self.raw.clear(); self.output.clear(); self.reveal.clear(); self.written = 0;
        }
        result
    }

    fn read_request(&mut self) -> Result<ClientProgress, WorkerIoError> {
        let mut scratch = [0_u8; CLIENT_READ_CHUNK];
        let offered = (self.expected_bytes - self.raw.len()).min(scratch.len());
        if offered != 0 {
            let count = match self.stream.read(&mut scratch[..offered]) {
                Ok(0) => return Err(WorkerIoError::Io(io::ErrorKind::UnexpectedEof)),
                Ok(n) if n <= offered => n,
                Ok(_) => return Err(WorkerIoError::Protocol(Error::InvalidInput)),
                Err(error) if transient(error.kind()) => return Ok(ClientProgress::Blocked),
                Err(error) => return Err(WorkerIoError::Io(error.kind())),
            };
            self.raw.extend_from_slice(&scratch[..count]);
        }
        if self.raw.len() < self.expected_bytes { return Ok(ClientProgress::Progress); }
        if self.expected_bytes == REQUEST_HEADER_BYTES {
            self.expected_bytes = request_frame_len(&self.raw).map_err(WorkerIoError::Protocol)?;
            self.raw.try_reserve_exact(self.expected_bytes - self.raw.len())
                .map_err(|_| WorkerIoError::Protocol(Error::Limit))?;
            if self.raw.len() < self.expected_bytes { return Ok(ClientProgress::Progress); }
        }
        let input = decode_request(&self.raw).map_err(WorkerIoError::Protocol)?;
        if input.actual_input().input_profile() != &self.expected {
            return Err(WorkerIoError::Protocol(Error::Binding));
        }
        self.input = Some(input);
        self.raw = Vec::new();
        self.phase = ClientPhase::NeedsInference;
        Ok(ClientProgress::NeedsInference)
    }

    fn read_reveal_request(&mut self) -> Result<ClientProgress, WorkerIoError> {
        let mut signal = [0_u8; 1];
        match self.stream.read(&mut signal) {
            Ok(0) => Err(WorkerIoError::Io(io::ErrorKind::UnexpectedEof)),
            Ok(1) if signal[0] == REVEAL_REQUEST => {
                self.output = std::mem::take(&mut self.reveal);
                self.written = 0;
                self.phase = ClientPhase::SendingReveal;
                Ok(ClientProgress::Progress)
            }
            Ok(_) => Err(WorkerIoError::Protocol(Error::InvalidInput)),
            Err(error) if transient(error.kind()) => Ok(ClientProgress::Blocked),
            Err(error) => Err(WorkerIoError::Io(error.kind())),
        }
    }

    fn send(&mut self) -> Result<ClientProgress, WorkerIoError> {
        if self.written < self.output.len() {
            let remaining = &self.output[self.written..];
            match self.stream.write(remaining) {
                Ok(0) => return Err(WorkerIoError::Io(io::ErrorKind::WriteZero)),
                Ok(n) if n <= remaining.len() => self.written += n,
                Ok(_) => return Err(WorkerIoError::Protocol(Error::InvalidInput)),
                Err(error) if transient(error.kind()) => return Ok(ClientProgress::Blocked),
                Err(error) => return Err(WorkerIoError::Io(error.kind())),
            }
            if self.written < self.output.len() { return Ok(ClientProgress::Progress); }
        }
        match self.stream.flush() {
            Ok(()) => {
                self.output.clear(); self.written = 0;
                self.phase = if self.phase == ClientPhase::SendingCommitment {
                    ClientPhase::AwaitingReveal
                } else { ClientPhase::ReplySent };
                Ok(if self.phase == ClientPhase::ReplySent { ClientProgress::ReplySent } else { ClientProgress::Progress })
            }
            Err(error) if transient(error.kind()) => Ok(ClientProgress::Blocked),
            Err(error) => Err(WorkerIoError::Io(error.kind())) ,
        }
    }
}

fn transient(kind: io::ErrorKind) -> bool {
    matches!(kind, io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted)
}

#[cfg(unix)]
impl HelperClient<std::os::unix::net::UnixStream> {
    /// The caller provisions/authenticates the connected peer. No listener,
    /// process launcher, model loader or second executor is hidden here.
    pub fn from_unix(stream: std::os::unix::net::UnixStream, expected: InputProfileBinding) -> Result<Self, WorkerIoError> {
        stream.set_nonblocking(true).map_err(|error| WorkerIoError::Io(error.kind()))?;
        Self::new(stream, expected).map_err(WorkerIoError::Protocol)
    }
}

#[cfg(unix)]
impl std::os::fd::AsFd for HelperClient<std::os::unix::net::UnixStream> {
    fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> { std::os::fd::AsFd::as_fd(&self.stream) }
}
