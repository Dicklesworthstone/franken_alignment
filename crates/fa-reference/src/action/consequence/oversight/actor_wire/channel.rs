//! Bounded newline framing and resumable I/O without owning a task runtime.
//! A reply must drain and flush before another command can touch the actor port.

use super::{ActorWire, MAX_FRAME_BYTES, MAX_RESPONSE_BYTES, WireError};
use std::fmt;
use std::io::{self, BufRead, Write};

pub const MAX_CHANNEL_EXCHANGES: u64 = 4_096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelLimits { pub frame_bytes: usize, pub exchanges: u64 }
impl Default for ChannelLimits {
    fn default() -> Self { Self { frame_bytes: MAX_FRAME_BYTES, exchanges: MAX_CHANNEL_EXCHANGES } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReason { PeerClosed, TruncatedFrame, FrameTooLarge, ExchangeLimit, IoFailure }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelState { Reading, ReplyReady, Closed(CloseReason) }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeedResult { pub consumed: usize, pub state: ChannelState }

/// One input frame and one bounded reply, no unbounded output queue. The host
/// schedules calls and authenticates the peer. Debug output contains no frames.
pub struct ActorChannel {
    wire: ActorWire,
    input: Vec<u8>,
    output: Vec<u8>,
    written: usize,
    limits: ChannelLimits,
    exchanges: u64,
    closed: Option<CloseReason>,
}

impl fmt::Debug for ActorChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActorChannel").field("state", &self.state())
            .field("buffered_input_bytes", &self.input.len()).field("exchanges", &self.exchanges).finish_non_exhaustive()
    }
}

impl ActorChannel {
    pub fn new(wire: ActorWire, limits: ChannelLimits) -> Result<Self, WireError> {
        if limits.frame_bytes == 0 || limits.exchanges == 0 { return Err(WireError::MalformedRequest); }
        if limits.frame_bytes > MAX_FRAME_BYTES || limits.exchanges > MAX_CHANNEL_EXCHANGES { return Err(WireError::Capacity); }
        Ok(Self { wire, input: Vec::with_capacity(limits.frame_bytes), output: Vec::new(),
            written: 0, limits, exchanges: 0, closed: None })
    }

    pub fn state(&self) -> ChannelState {
        if let Some(reason) = self.closed {
            if reason != CloseReason::PeerClosed || self.output.is_empty() { return ChannelState::Closed(reason); }
        }
        if !self.output.is_empty() { return ChannelState::ReplyReady; }
        if self.exchanges == self.limits.exchanges { return ChannelState::Closed(CloseReason::ExchangeLimit); }
        ChannelState::Reading
    }

    /// Returns the EXACT consumed prefix, including at most one newline. Bytes
    /// beyond it remain caller-owned and unprocessed, even in a pipelined read.
    /// No input is accepted while a previous reply is waiting to drain/flush.
    pub fn feed(&mut self, bytes: &[u8]) -> FeedResult {
        if self.state() != ChannelState::Reading { return FeedResult { consumed: 0, state: self.state() }; }
        for (index, &byte) in bytes.iter().enumerate() {
            if byte == b'\n' {
                self.output = self.wire.exchange(&self.input).encode();
                self.output.push(b'\n');
                debug_assert!(self.output.len() <= MAX_RESPONSE_BYTES + 1);
                self.input.clear(); self.written = 0; self.exchanges += 1;
                return FeedResult { consumed: index + 1, state: self.state() };
            }
            if self.input.len() == self.limits.frame_bytes {
                self.fail(CloseReason::FrameTooLarge);
                return FeedResult { consumed: index + 1, state: self.state() };
            }
            self.input.push(byte);
        }
        FeedResult { consumed: bytes.len(), state: self.state() }
    }

    /// EOF never promotes an unterminated JSON document to a command. A prior
    /// complete command remains accepted; closing a transport is not cancelling it.
    pub fn finish_input(&mut self) -> ChannelState {
        if self.closed.is_none() {
            if self.input.is_empty() { self.closed = Some(CloseReason::PeerClosed); }
            else { self.fail(CloseReason::TruncatedFrame); }
        }
        self.state()
    }

    pub fn pending_output(&self) -> &[u8] { &self.output[self.written..] }

    /// Integration with a native asynchronous writer: advance only bytes actually
    /// accepted by it. This does not assert that its buffered output was flushed.
    pub fn acknowledge_written(&mut self, bytes: usize) -> Result<(), WireError> {
        if self.state() != ChannelState::ReplyReady || bytes > self.pending_output().len() { return Err(WireError::MalformedRequest); }
        self.written += bytes;
        Ok(())
    }

    pub fn acknowledge_flushed(&mut self) -> Result<ChannelState, WireError> {
        if self.state() != ChannelState::ReplyReady || self.written != self.output.len() { return Err(WireError::MalformedRequest); }
        self.output.clear(); self.written = 0;
        Ok(self.state())
    }

    /// Recover the role-bound handler after disconnect, discarding only transport
    /// fragments and reply bytes. A new connection must use complete frames and
    /// clients retry the SAME submission key/bytes, not a replacement effect.
    pub fn into_wire(self) -> ActorWire { self.wire }

    /// One buffered read operation, with no hidden blocking loop or overread.
    /// Interrupted/WouldBlock preserve fragments. A terminal I/O error closes
    /// only this transport; already queued/dispatched requests retain their state.
    pub fn read_once<R: BufRead>(&mut self, reader: &mut R) -> io::Result<ChannelState> {
        if self.state() != ChannelState::Reading { return Ok(self.state()); }
        let bytes = match reader.fill_buf() {
            Ok(bytes) => bytes,
            Err(error) => { self.io_failure(&error); return Err(error); }
        };
        if bytes.is_empty() { return Ok(self.finish_input()); }
        let progress = self.feed(bytes);
        reader.consume(progress.consumed);
        Ok(progress.state)
    }

    /// One write and, when all bytes were accepted, one flush. On partial writes
    /// or WouldBlock the next call resumes only the remaining bytes/flush. A
    /// response write failure never replays the corresponding input command.
    pub fn write_once<W: Write>(&mut self, writer: &mut W) -> io::Result<ChannelState> {
        if self.state() != ChannelState::ReplyReady { return Ok(self.state()); }
        if !self.pending_output().is_empty() {
            let offered = self.pending_output().len();
            let written = match writer.write(self.pending_output()) {
                Ok(0) => {
                    self.fail(CloseReason::IoFailure);
                    return Err(io::Error::from(io::ErrorKind::WriteZero));
                }
                Ok(count) if count <= offered => count,
                Ok(_) => {
                    self.fail(CloseReason::IoFailure);
                    return Err(io::Error::from(io::ErrorKind::InvalidData));
                }
                Err(error) => { self.io_failure(&error); return Err(error); }
            };
            self.written += written;
        }
        if self.pending_output().is_empty() {
            if let Err(error) = writer.flush() { self.io_failure(&error); return Err(error); }
            self.output.clear(); self.written = 0;
        }
        Ok(self.state())
    }

    fn io_failure(&mut self, error: &io::Error) {
        if !matches!(error.kind(), io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock) { self.fail(CloseReason::IoFailure); }
    }

    fn fail(&mut self, reason: CloseReason) {
        self.input.clear(); self.output.clear(); self.written = 0; self.closed = Some(reason);
    }
}
