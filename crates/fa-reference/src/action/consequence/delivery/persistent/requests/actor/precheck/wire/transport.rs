//! Nonblocking Unix transport for the separately granted precheck capability.
//! The caller authenticates a raw connection; FilePrecheckPeerSession does so
//! before reading bytes. No listener, task runtime or authority is created.

use super::{FilePrecheckChannel, FilePrecheckWire, PrecheckWireError};
use crate::action::consequence::oversight::actor_transport::{ConnectionFailure,
    ConnectionStatus, DriveBudget, DriveProgress, DriveReport, IoDirection, SOCKET_BUFFER_BYTES};
use crate::action::consequence::oversight::actor_wire::ChannelState;
use std::fmt;
use std::io::{self, Read, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixStream;

/// Only this trusted-side connection holds the granted read wire. A successful
/// write reports local socket acceptance, not peer receipt or execution rights.
/// Socket work is bounded; the precheck's own finite native evaluator remains
/// synchronous. No descriptor is cloned and no second executor is introduced.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::UnixPrecheckConnection;
/// fn no_owner(connection: UnixPrecheckConnection) { let _ = connection.host_mut(); }
/// ```
pub struct UnixPrecheckConnection {
    socket: UnixStream,
    channel: FilePrecheckChannel,
    input: [u8; SOCKET_BUFFER_BYTES],
    start: usize,
    end: usize,
    failure: Option<ConnectionFailure>,
}

impl fmt::Debug for UnixPrecheckConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnixPrecheckConnection").field("status", &self.status()).finish_non_exhaustive()
    }
}

impl AsFd for UnixPrecheckConnection {
    fn as_fd(&self) -> BorrowedFd<'_> { self.socket.as_fd() }
}

impl UnixPrecheckConnection {
    /// The integrating host must authenticate this socket before handing it in.
    /// On Linux prefer FilePrecheckPeerSession, which performs that admission.
    pub fn new(socket: UnixStream, channel: FilePrecheckChannel) -> io::Result<Self> {
        socket.set_nonblocking(true)?;
        Ok(Self::from_nonblocking(socket, channel))
    }

    pub(super) fn from_nonblocking(socket: UnixStream, channel: FilePrecheckChannel) -> Self {
        Self { socket, channel, input: [0; SOCKET_BUFFER_BYTES], start: 0, end: 0, failure: None }
    }

    pub fn status(&self) -> ConnectionStatus {
        ConnectionStatus { channel: self.channel.state(), buffered_input_bytes: self.end - self.start,
            pending_output_bytes: self.channel.pending_output().len(), failure: self.failure }
    }

    /// Zero or over-limit budgets refuse before any I/O or buffered evaluation.
    /// WouldBlock yields; every Interrupted read/write/flush spends an I/O slot.
    /// Completed progress accompanies a latched terminal failure. Later drives
    /// do not retry intake, an evaluation, or any partially delivered response.
    pub fn drive(&mut self, budget: DriveBudget) -> Result<DriveReport, PrecheckWireError> {
        if budget.read_bytes == 0 || budget.write_bytes == 0 || budget.frames == 0 || budget.io_calls == 0 {
            return Err(PrecheckWireError::MalformedRequest);
        }
        budget.validate().map_err(|_| PrecheckWireError::Capacity)?;
        let mut progress = DriveProgress::default();
        while !self.status().closed() {
            match self.channel.state() {
                ChannelState::Reading => {
                    if progress.frames == budget.frames { break; }
                    if self.start != self.end {
                        let remaining = self.channel.remaining_exchanges();
                        let fed = self.channel.feed(&self.input[self.start..self.end]);
                        self.start += fed.consumed;
                        progress.consumed_bytes += fed.consumed;
                        // Count entered complete frames even if response encoding
                        // refuses after the original wire has charged the frame.
                        progress.frames += usize::from(self.channel.remaining_exchanges() != remaining);
                        if fed.consumed == 0 { break; }
                        continue;
                    }
                    if progress.read_bytes == budget.read_bytes || progress.io_calls == budget.io_calls { break; }
                    let length = SOCKET_BUFFER_BYTES.min(budget.read_bytes - progress.read_bytes);
                    progress.io_calls += 1;
                    match self.socket.read(&mut self.input[..length]) {
                        Ok(0) => { self.channel.finish_input(); break; }
                        Ok(count) => { self.start = 0; self.end = count; progress.read_bytes += count; }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            progress.would_block = Some(IoDirection::Read); break;
                        }
                        Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                        Err(error) => { self.fail(IoDirection::Read, error.kind()); break; }
                    }
                }
                ChannelState::ReplyReady => {
                    if progress.io_calls == budget.io_calls { break; }
                    if self.channel.pending_output().is_empty() {
                        progress.io_calls += 1;
                        match self.socket.flush() {
                            Ok(()) => {
                                self.channel.acknowledge_flushed().expect("fully written precheck response");
                            }
                            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                                progress.would_block = Some(IoDirection::Flush); break;
                            }
                            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                            Err(error) => { self.fail(IoDirection::Flush, error.kind()); break; }
                        }
                    } else {
                        if progress.written_bytes == budget.write_bytes { break; }
                        let length = self.channel.pending_output().len().min(budget.write_bytes - progress.written_bytes);
                        progress.io_calls += 1;
                        match self.socket.write(&self.channel.pending_output()[..length]) {
                            Ok(0) => { self.fail(IoDirection::Write, io::ErrorKind::WriteZero); break; }
                            Ok(count) => {
                                self.channel.acknowledge_written(count).expect("socket accepted offered bytes");
                                progress.written_bytes += count;
                            }
                            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                                progress.would_block = Some(IoDirection::Write); break;
                            }
                            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                            Err(error) => { self.fail(IoDirection::Write, error.kind()); break; }
                        }
                    }
                }
                ChannelState::Closed(_) => break,
            }
        }
        Ok(DriveReport { progress, status: self.status() })
    }

    /// Close this socket and discard its incomplete input/output only. Both
    /// original parser and shared native query allowances remain spent.
    pub fn into_session(self) -> FilePrecheckWire { self.channel.into_wire() }

    #[cfg(target_os = "linux")]
    pub(super) fn into_channel(mut self) -> FilePrecheckChannel {
        self.channel.reset_transport();
        self.channel
    }

    fn fail(&mut self, direction: IoDirection, kind: io::ErrorKind) {
        self.failure = Some(ConnectionFailure { direction, kind });
    }
}
