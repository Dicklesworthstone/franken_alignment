//! Bounded precheck framing with one reply and a mandatory write/flush barrier.
//! No source acquisition, clock observation, grant renewal or effect operation.

use super::{FilePrecheckWire, MAX_PRECHECK_FRAME_BYTES, MAX_PRECHECK_RESPONSE_BYTES,
    PrecheckWireError};
use crate::action::consequence::oversight::actor_wire::{ChannelState, CloseReason, FeedResult};
use std::fmt;

/// One original precheck wire across arbitrary transport fragments. The finite
/// exchange allowance belongs to that wire, not to this framing lifetime, so
/// recovering it after a disconnect cannot replenish already entered work.
///
/// Give an actor only the opposite transport. This channel does not authenticate
/// a peer and deliberately has no owner, source, custodian or effect-port getter.
pub struct FilePrecheckChannel {
    wire: FilePrecheckWire,
    input: Vec<u8>,
    output: Vec<u8>,
    written: usize,
    frame_bytes: usize,
    closed: Option<CloseReason>,
}

impl fmt::Debug for FilePrecheckChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilePrecheckChannel")
            .field("state", &self.state())
            .field("buffered_input_bytes", &self.input.len())
            .field("remaining_exchanges", &self.wire.remaining_exchanges())
            .finish_non_exhaustive()
    }
}

impl FilePrecheckChannel {
    /// `frame_bytes` excludes the newline and may only narrow the codec limit.
    /// An exhausted wire is valid but already closed; construction never renews it.
    pub fn new(wire: FilePrecheckWire, frame_bytes: usize) -> Result<Self, PrecheckWireError> {
        if frame_bytes == 0 { return Err(PrecheckWireError::MalformedRequest); }
        if frame_bytes > MAX_PRECHECK_FRAME_BYTES { return Err(PrecheckWireError::Capacity); }
        let mut input = Vec::new();
        input.try_reserve_exact(frame_bytes).map_err(|_| PrecheckWireError::Capacity)?;
        let mut output = Vec::new();
        output.try_reserve_exact(MAX_PRECHECK_RESPONSE_BYTES + 1)
            .map_err(|_| PrecheckWireError::Capacity)?;
        Ok(Self { wire, input, output, written: 0, frame_bytes, closed: None })
    }

    pub fn state(&self) -> ChannelState {
        if let Some(reason) = self.closed {
            if reason != CloseReason::PeerClosed || self.output.is_empty() {
                return ChannelState::Closed(reason);
            }
        }
        if !self.output.is_empty() { return ChannelState::ReplyReady; }
        if self.wire.remaining_exchanges() == 0 {
            return ChannelState::Closed(CloseReason::ExchangeLimit);
        }
        ChannelState::Reading
    }

    /// Consume exactly one prefix through at most one newline. Pipelined suffix
    /// bytes stay caller-owned until this response has been written AND flushed.
    /// An oversized or unterminated frame never enters the native evaluator.
    pub fn feed(&mut self, bytes: &[u8]) -> FeedResult {
        if self.state() != ChannelState::Reading {
            return FeedResult { consumed: 0, state: self.state() };
        }
        for (index, &byte) in bytes.iter().enumerate() {
            if byte == b'\n' {
                // exchange charges BEFORE parsing. A later encoding or output
                // failure never refunds that allowance or replays the request.
                let response = self.wire.exchange_frame(&self.input);
                self.input.clear();
                self.written = 0;
                match response {
                    Ok(bytes) if bytes.len() <= MAX_PRECHECK_RESPONSE_BYTES => {
                        self.output.extend_from_slice(&bytes);
                        self.output.push(b'\n');
                    }
                    _ => self.fail(CloseReason::IoFailure),
                }
                return FeedResult { consumed: index + 1, state: self.state() };
            }
            if self.input.len() == self.frame_bytes {
                self.fail(CloseReason::FrameTooLarge);
                return FeedResult { consumed: index + 1, state: self.state() };
            }
            self.input.push(byte);
        }
        FeedResult { consumed: bytes.len(), state: self.state() }
    }

    /// EOF is not a delimiter. A completed response may still drain after EOF;
    /// an incomplete JSON line is dropped without parsing or evaluation.
    pub fn finish_input(&mut self) -> ChannelState {
        if self.closed.is_none() {
            if self.input.is_empty() { self.closed = Some(CloseReason::PeerClosed); }
            else { self.fail(CloseReason::TruncatedFrame); }
        }
        self.state()
    }

    pub fn pending_output(&self) -> &[u8] { &self.output[self.written..] }

    /// Advance only bytes accepted by the trusted writer. Even an empty suffix
    /// remains ReplyReady until a separate successful flush is acknowledged.
    pub fn acknowledge_written(&mut self, bytes: usize) -> Result<(), PrecheckWireError> {
        if self.state() != ChannelState::ReplyReady || bytes > self.pending_output().len() {
            return Err(PrecheckWireError::MalformedRequest);
        }
        self.written += bytes;
        Ok(())
    }

    pub fn acknowledge_flushed(&mut self) -> Result<ChannelState, PrecheckWireError> {
        if self.state() != ChannelState::ReplyReady || self.written != self.output.len() {
            return Err(PrecheckWireError::MalformedRequest);
        }
        self.output.clear();
        self.written = 0;
        Ok(self.state())
    }

    /// Drop fragments and undelivered replies, retaining the SAME finite wire.
    /// A retry is another query, not an idempotent effect or a free evaluation.
    pub fn into_wire(self) -> FilePrecheckWire { self.wire }

    #[cfg(unix)]
    pub(super) fn remaining_exchanges(&self) -> usize { self.wire.remaining_exchanges() }

    #[cfg(target_os = "linux")]
    pub(super) fn reset_transport(&mut self) {
        self.input.clear();
        self.output.clear();
        self.written = 0;
        self.closed = None;
    }

    fn fail(&mut self, reason: CloseReason) {
        self.input.clear();
        self.output.clear();
        self.written = 0;
        self.closed = Some(reason);
    }
}
