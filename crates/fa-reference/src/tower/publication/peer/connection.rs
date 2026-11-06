//! Cooperative socket work for one already authenticated Tower peer.

use super::channel::Channel;
use crate::action::consequence::oversight::actor_transport::{
    ConnectionFailure, ConnectionStatus, DriveBudget, DriveProgress, DriveReport, IoDirection,
    SOCKET_BUFFER_BYTES,
};
use crate::action::consequence::oversight::actor_wire::{ChannelState, WireError};
use crate::tower::TowerEncodingError;
use std::io::{self, Read, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixStream;

pub(super) struct Connection {
    socket: UnixStream,
    input: [u8; SOCKET_BUFFER_BYTES],
    start: usize,
    end: usize,
    failure: Option<ConnectionFailure>,
}

impl Connection {
    /// Authentication and nonblocking setup have already succeeded. No fallible
    /// syscall follows moving this connection into the original ticket session.
    pub(super) fn new(socket: UnixStream) -> Self {
        Self {
            socket,
            input: [0; SOCKET_BUFFER_BYTES],
            start: 0,
            end: 0,
            failure: None,
        }
    }

    pub(super) fn socket_fd(&self) -> BorrowedFd<'_> {
        self.socket.as_fd()
    }

    pub(super) fn status(&self, channel: &Channel) -> ConnectionStatus {
        ConnectionStatus {
            channel: channel.state(),
            buffered_input_bytes: self.end - self.start,
            pending_output_bytes: channel.pending_output().len(),
            failure: self.failure,
        }
    }

    pub(super) fn drive(
        &mut self,
        channel: &mut Channel,
        budget: DriveBudget,
        mut exchange: impl FnMut(&[u8]) -> Result<String, TowerEncodingError>,
    ) -> Result<DriveReport, WireError> {
        budget.validate()?;
        let mut progress = DriveProgress::default();
        while !self.status(channel).closed() {
            match channel.state() {
                ChannelState::Reading => {
                    if progress.frames == budget.frames {
                        break;
                    }
                    if self.start != self.end {
                        let remaining = channel.remaining_exchanges();
                        let fed = channel.feed(&self.input[self.start..self.end], &mut exchange);
                        self.start += fed.consumed;
                        progress.consumed_bytes += fed.consumed;
                        // Encoding can fail after native intake. Such an entered
                        // frame still spends the scheduler's frame allowance.
                        progress.frames += usize::from(channel.remaining_exchanges() != remaining);
                        if fed.consumed == 0 {
                            break;
                        }
                        continue;
                    }
                    if progress.read_bytes == budget.read_bytes
                        || progress.io_calls == budget.io_calls
                    {
                        break;
                    }
                    let length = SOCKET_BUFFER_BYTES.min(budget.read_bytes - progress.read_bytes);
                    progress.io_calls += 1;
                    match self.socket.read(&mut self.input[..length]) {
                        Ok(0) => {
                            channel.finish_input();
                            break;
                        }
                        Ok(count) => {
                            self.start = 0;
                            self.end = count;
                            progress.read_bytes += count;
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            progress.would_block = Some(IoDirection::Read);
                            break;
                        }
                        Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                        Err(error) => {
                            self.fail(IoDirection::Read, error.kind());
                            break;
                        }
                    }
                }
                ChannelState::ReplyReady => {
                    if progress.io_calls == budget.io_calls {
                        break;
                    }
                    if channel.pending_output().is_empty() {
                        progress.io_calls += 1;
                        match self.socket.flush() {
                            Ok(()) => channel
                                .acknowledge_flushed()
                                .expect("fully written Tower response"),
                            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                                progress.would_block = Some(IoDirection::Flush);
                                break;
                            }
                            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                            Err(error) => {
                                self.fail(IoDirection::Flush, error.kind());
                                break;
                            }
                        }
                    } else {
                        if progress.written_bytes == budget.write_bytes {
                            break;
                        }
                        let length = channel
                            .pending_output()
                            .len()
                            .min(budget.write_bytes - progress.written_bytes);
                        progress.io_calls += 1;
                        match self.socket.write(&channel.pending_output()[..length]) {
                            Ok(0) => {
                                self.fail(IoDirection::Write, io::ErrorKind::WriteZero);
                                break;
                            }
                            Ok(count) => {
                                channel
                                    .acknowledge_written(count)
                                    .expect("socket accepted offered bytes");
                                progress.written_bytes += count;
                            }
                            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                                progress.would_block = Some(IoDirection::Write);
                                break;
                            }
                            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                            Err(error) => {
                                self.fail(IoDirection::Write, error.kind());
                                break;
                            }
                        }
                    }
                }
                ChannelState::Closed(_) => break,
            }
        }
        Ok(DriveReport {
            progress,
            status: self.status(channel),
        })
    }

    fn fail(&mut self, direction: IoDirection, kind: io::ErrorKind) {
        self.failure = Some(ConnectionFailure { direction, kind });
    }
}
