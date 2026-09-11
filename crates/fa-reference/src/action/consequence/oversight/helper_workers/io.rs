//! Incremental I/O for one preprovisioned helper, without a thread or executor.
//! A caller using blocking Read/Write must supply its own bounded I/O contract;
//! the Unix pool selects nonblocking streams before any request bytes are sent.

#[cfg(unix)]
mod pool;
#[cfg(unix)]
pub use pool::{HelperPool, HelperPump};

use super::{HelperFailure, HelperPhase, HelperPort};
use super::wire::{
    REVEAL_REQUEST, decode_commitment, decode_reveal, encode_request, reveal_frame_len,
};
use crate::Error;
use std::io::{self, Read, Write};

pub const IO_CHUNK_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerIoError { Protocol(Error), Io(io::ErrorKind) }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IoProgress { Progress, Blocked, AwaitCoordinator, Complete, Closed }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage { WriteRequest, ReadCommit, WaitCommit, WriteReveal, ReadReveal, WaitReveal, Complete, Closed }

/// Owns a member's port and its transport. No actor or helper request can choose
/// another member, replace a socket, restart a round, or import a caller vote.
pub struct HelperConnection<S> {
    port: HelperPort,
    stream: S,
    stage: Stage,
    output: Vec<u8>,
    written: usize,
    input: Vec<u8>,
    error: Option<WorkerIoError>,
}

impl<S: Read + Write> HelperConnection<S> {
    pub fn new(port: HelperPort, stream: S) -> Result<Self, Error> {
        if port.phase() != HelperPhase::AwaitCommit { return Err(Error::WrongState); }
        let output = encode_request(&port)?;
        Ok(Self { port, stream, stage: Stage::WriteRequest, output, written: 0, input: Vec::new(), error: None })
    }

    pub fn member(&self) -> &str { self.port.request().member() }
    pub fn failure(&self) -> Option<WorkerIoError> { self.error }

    /// At most one read OR one write plus an optional flush. Interrupted and
    /// WouldBlock preserve all offsets. Failed I/O never resends the request,
    /// substitutes a vote or removes the helper from the original denominator.
    pub fn step(&mut self) -> Result<IoProgress, WorkerIoError> {
        match self.port.phase() {
            HelperPhase::Complete => { self.stage = Stage::Complete; return Ok(IoProgress::Complete); }
            HelperPhase::Failed | HelperPhase::Closed => {
                self.stage = Stage::Closed; self.input.clear(); self.output.clear();
                return Ok(IoProgress::Closed);
            }
            _ => {}
        }
        let result = match self.stage {
            Stage::WriteRequest | Stage::WriteReveal => self.send(),
            Stage::ReadCommit | Stage::ReadReveal => self.receive(),
            Stage::WaitCommit => {
                if self.port.phase() == HelperPhase::ReadyReveal {
                    self.output = vec![REVEAL_REQUEST]; self.written = 0; self.stage = Stage::WriteReveal;
                    self.send()
                } else { Ok(IoProgress::AwaitCoordinator) }
            }
            Stage::WaitReveal => Ok(IoProgress::AwaitCoordinator),
            Stage::Complete => Ok(IoProgress::Complete),
            Stage::Closed => Ok(IoProgress::Closed),
        };
        if let Err(error) = result {
            self.error = Some(error);
            let reason = match error {
                WorkerIoError::Protocol(reason) => HelperFailure::Rejected(reason),
                WorkerIoError::Io(_) => HelperFailure::Disconnected,
            };
            self.port.slot.borrow_mut().fail(reason);
            self.stage = Stage::Closed;
            self.input.clear(); self.output.clear();
        }
        result
    }

    fn send(&mut self) -> Result<IoProgress, WorkerIoError> {
        if self.written < self.output.len() {
            let end = self.output.len().min(self.written + IO_CHUNK_BYTES);
            let offered = end - self.written;
            match self.stream.write(&self.output[self.written..end]) {
                Ok(0) => return Err(WorkerIoError::Io(io::ErrorKind::WriteZero)),
                Ok(count) if count > offered => return Err(WorkerIoError::Protocol(Error::InvalidInput)),
                Ok(count) => self.written += count,
                Err(error) if transient(error.kind()) => return Ok(IoProgress::Blocked),
                Err(error) => return Err(WorkerIoError::Io(error.kind())),
            }
            if self.written < self.output.len() { return Ok(IoProgress::Progress); }
        }
        match self.stream.flush() {
            Ok(()) => {
                self.stage = if self.stage == Stage::WriteRequest { Stage::ReadCommit } else { Stage::ReadReveal };
                self.output.clear(); self.written = 0;
                Ok(IoProgress::Progress)
            }
            Err(error) if transient(error.kind()) => Ok(IoProgress::Blocked),
            Err(error) => Err(WorkerIoError::Io(error.kind())),
        }
    }

    fn receive(&mut self) -> Result<IoProgress, WorkerIoError> {
        let expected = if self.stage == Stage::ReadCommit { 9 }
            else if self.input.len() < 4 { 4 }
            else { reveal_frame_len(&self.input[..4], self.port.salt_limit).map_err(WorkerIoError::Protocol)? };
        if self.input.len() < expected {
            let mut scratch = [0_u8; 512];
            let offered = (expected - self.input.len()).min(scratch.len());
            match self.stream.read(&mut scratch[..offered]) {
                Ok(0) => return Err(WorkerIoError::Io(io::ErrorKind::UnexpectedEof)),
                Ok(count) if count > offered => return Err(WorkerIoError::Protocol(Error::InvalidInput)),
                Ok(count) => self.input.extend_from_slice(&scratch[..count]),
                Err(error) if transient(error.kind()) => return Ok(IoProgress::Blocked),
                Err(error) => return Err(WorkerIoError::Io(error.kind())),
            }
        }
        if self.input.len() < expected { return Ok(IoProgress::Progress); }
        if self.stage == Stage::ReadCommit {
            let digest = decode_commitment(&self.input).map_err(WorkerIoError::Protocol)?;
            self.port.submit_commitment(digest).map_err(WorkerIoError::Protocol)?;
            self.input.clear(); self.stage = Stage::WaitCommit;
        } else {
            let complete = reveal_frame_len(&self.input[..4], self.port.salt_limit).map_err(WorkerIoError::Protocol)?;
            if self.input.len() < complete { return Ok(IoProgress::Progress); }
            let (verdict, salt) = decode_reveal(&self.input, self.port.salt_limit).map_err(WorkerIoError::Protocol)?;
            self.port.reveal(verdict, salt).map_err(WorkerIoError::Protocol)?;
            self.input.clear(); self.stage = Stage::WaitReveal;
        }
        Ok(IoProgress::AwaitCoordinator)
    }
}

fn transient(kind: io::ErrorKind) -> bool {
    matches!(kind, io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted)
}
