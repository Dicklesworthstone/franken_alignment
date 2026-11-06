//! One bounded Tower frame and one reply, with a lifetime exchange allowance.

use crate::action::consequence::oversight::actor_wire::{
    ChannelLimits, ChannelState, CloseReason, FeedResult, WireError,
};
use crate::tower::TowerEncodingError;

pub(super) struct Channel {
    input: Vec<u8>,
    output: Vec<u8>,
    written: usize,
    limits: ChannelLimits,
    exchanges: u64,
    closed: Option<CloseReason>,
}

impl Channel {
    pub(super) fn new(limits: ChannelLimits) -> Self {
        Self {
            input: Vec::with_capacity(limits.frame_bytes),
            output: Vec::new(),
            written: 0,
            limits,
            exchanges: 0,
            closed: None,
        }
    }

    pub(super) fn state(&self) -> ChannelState {
        if let Some(reason) = self.closed
            && (reason != CloseReason::PeerClosed || self.output.is_empty())
        {
            return ChannelState::Closed(reason);
        }
        if !self.output.is_empty() {
            return ChannelState::ReplyReady;
        }
        if self.remaining_exchanges() == 0 {
            return ChannelState::Closed(CloseReason::ExchangeLimit);
        }
        ChannelState::Reading
    }

    pub(super) fn remaining_exchanges(&self) -> u64 {
        self.limits.exchanges - self.exchanges
    }

    /// Consume at most one LF-terminated document. Pipelined suffixes stay in
    /// the caller's bounded socket buffer until this reply is written AND flushed.
    pub(super) fn feed(
        &mut self,
        bytes: &[u8],
        exchange: impl FnOnce(&[u8]) -> Result<String, TowerEncodingError>,
    ) -> FeedResult {
        if self.state() != ChannelState::Reading {
            return FeedResult {
                consumed: 0,
                state: self.state(),
            };
        }
        for (index, &byte) in bytes.iter().enumerate() {
            if byte == b'\n' {
                // The completed exchange is spent even if a native operation
                // commits but its response cannot be encoded. Never retry here.
                self.exchanges += 1;
                match exchange(&self.input) {
                    Ok(response) => {
                        self.output = response.into_bytes();
                        self.output.push(b'\n');
                        self.input.clear();
                        self.written = 0;
                    }
                    Err(_) => self.fail(CloseReason::IoFailure),
                }
                return FeedResult {
                    consumed: index + 1,
                    state: self.state(),
                };
            }
            if self.input.len() == self.limits.frame_bytes {
                self.fail(CloseReason::FrameTooLarge);
                return FeedResult {
                    consumed: index + 1,
                    state: self.state(),
                };
            }
            self.input.push(byte);
        }
        FeedResult {
            consumed: bytes.len(),
            state: self.state(),
        }
    }

    pub(super) fn finish_input(&mut self) {
        if self.closed.is_none() {
            if self.input.is_empty() {
                self.closed = Some(CloseReason::PeerClosed);
            } else {
                self.fail(CloseReason::TruncatedFrame);
            }
        }
    }

    pub(super) fn pending_output(&self) -> &[u8] {
        &self.output[self.written..]
    }

    pub(super) fn acknowledge_written(&mut self, bytes: usize) -> Result<(), WireError> {
        if self.state() != ChannelState::ReplyReady || bytes > self.pending_output().len() {
            return Err(WireError::MalformedRequest);
        }
        self.written += bytes;
        Ok(())
    }

    pub(super) fn acknowledge_flushed(&mut self) -> Result<(), WireError> {
        if self.state() != ChannelState::ReplyReady || self.written != self.output.len() {
            return Err(WireError::MalformedRequest);
        }
        self.output.clear();
        self.written = 0;
        Ok(())
    }

    /// A new authenticated connection discards transport fragments only. Its
    /// original Tower tickets, disclosure grants and exchange allowance survive.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragments_and_flush_barrier_cannot_enter_a_second_exchange() {
        let mut channel = Channel::new(ChannelLimits {
            frame_bytes: 8,
            exchanges: 2,
        });
        let mut seen = Vec::new();
        let first = channel.feed(b"a", |_| panic!("fragment executed"));
        assert_eq!(first.consumed, 1);
        let second = channel.feed(b"b\nnext\n", |bytes| {
            seen.push(bytes.to_vec());
            Ok("reply".into())
        });
        assert_eq!(second.consumed, 2);
        assert_eq!(seen, vec![b"ab".to_vec()]);
        assert_eq!(
            channel
                .feed(b"next\n", |_| panic!("pending reply executed"))
                .consumed,
            0
        );
        channel.acknowledge_written(6).unwrap();
        assert!(channel.pending_output().is_empty());
        assert_eq!(
            channel
                .feed(b"next\n", |_| panic!("unflushed reply executed"))
                .consumed,
            0
        );
        channel.acknowledge_flushed().unwrap();
        channel.feed(b"next\n", |bytes| {
            assert_eq!(bytes, b"next");
            Ok("last".into())
        });
        channel.acknowledge_written(5).unwrap();
        channel.acknowledge_flushed().unwrap();
        assert_eq!(
            channel.state(),
            ChannelState::Closed(CloseReason::ExchangeLimit)
        );
        channel.reset_transport();
        assert_eq!(
            channel.state(),
            ChannelState::Closed(CloseReason::ExchangeLimit)
        );
    }

    #[test]
    fn truncated_or_oversized_frames_never_execute_and_encoding_failure_spends_once() {
        let limits = ChannelLimits {
            frame_bytes: 3,
            exchanges: 2,
        };
        let mut channel = Channel::new(limits);
        channel.feed(b"abc", |_| panic!("unterminated input executed"));
        channel.finish_input();
        assert_eq!(
            channel.state(),
            ChannelState::Closed(CloseReason::TruncatedFrame)
        );
        assert_eq!(channel.remaining_exchanges(), 2);
        channel.reset_transport();
        assert_eq!(
            channel
                .feed(b"abcd\n", |_| panic!("oversize input executed"))
                .consumed,
            4
        );
        assert_eq!(
            channel.state(),
            ChannelState::Closed(CloseReason::FrameTooLarge)
        );
        channel.reset_transport();
        channel.feed(b"abc\n", |_| Err(TowerEncodingError::OutputLimit));
        assert_eq!(
            channel.state(),
            ChannelState::Closed(CloseReason::IoFailure)
        );
        assert_eq!(channel.remaining_exchanges(), 1);
        channel.reset_transport();
        assert_eq!(channel.remaining_exchanges(), 1);
    }
}
