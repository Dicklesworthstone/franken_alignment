//! Nonblocking Unix transport for the existing actor codec and framed channel.
//!
//! The operator supplies an already connected peer and its ActorChannel. No
//! listener, peer authentication, supervisor, executor or credential is created.
//! All socket I/O is bounded per drive; dropping a connection never cancels work.

use super::actor::ActorPort;
use super::actor_wire::{ActorRequestPort, ActorChannel, ActorWire, ChannelState, WireError};
use std::fmt;
use std::io::{self, Read, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixStream;

pub const SOCKET_BUFFER_BYTES: usize = 8_192;
pub const MAX_DRIVE_BYTES: usize = 1_048_576;
pub const MAX_DRIVE_FRAMES: usize = 64;
pub const MAX_DRIVE_IO_CALLS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DriveBudget {
    pub read_bytes: usize,
    pub write_bytes: usize,
    pub frames: usize,
    /// Counts read/write/flush attempts, including Interrupted and WouldBlock.
    /// A standard unbuffered socket flush need not issue a kernel syscall.
    pub io_calls: usize,
}

impl Default for DriveBudget {
    fn default() -> Self { Self { read_bytes: 65_536, write_bytes: 65_536, frames: 16, io_calls: 64 } }
}

impl DriveBudget {
    fn validate(self) -> Result<(), WireError> {
        if self.read_bytes > MAX_DRIVE_BYTES || self.write_bytes > MAX_DRIVE_BYTES
            || self.frames > MAX_DRIVE_FRAMES || self.io_calls > MAX_DRIVE_IO_CALLS
        { return Err(WireError::Capacity); }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IoDirection { Read, Write, Flush }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnectionFailure { pub direction: IoDirection, pub kind: io::ErrorKind }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DriveProgress {
    pub read_bytes: usize,
    pub written_bytes: usize,
    /// Bytes passed to the framed channel; may include data read in a prior call.
    pub consumed_bytes: usize,
    /// Complete JSON lines exchanged, including lines yielding a codec refusal.
    pub frames: usize,
    pub io_calls: usize,
    pub would_block: Option<IoDirection>,
}

/// Host-only transport state. It is not an actor outcome or nonexecution proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnectionStatus {
    pub channel: ChannelState,
    /// Bytes already read but not yet admitted to the framed channel.
    pub buffered_input_bytes: usize,
    pub pending_output_bytes: usize,
    pub failure: Option<ConnectionFailure>,
}

impl ConnectionStatus {
    pub fn closed(self) -> bool { self.failure.is_some() || matches!(self.channel, ChannelState::Closed(_)) }

    pub fn interest(self) -> DriveInterest {
        if self.closed() { return DriveInterest::Closed; }
        match self.channel {
            ChannelState::Reading if self.buffered_input_bytes > 0 => DriveInterest::LocalWork,
            ChannelState::Reading => DriveInterest::Readable,
            ChannelState::ReplyReady if self.pending_output_bytes == 0 => DriveInterest::LocalWork,
            ChannelState::ReplyReady => DriveInterest::Writable,
            ChannelState::Closed(_) => DriveInterest::Closed,
        }
    }
}

/// Scheduling action, not an effect-admission result. Buffered requests and an
/// outstanding local flush must not wait for a new peer-readability event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriveInterest { LocalWork, Readable, Writable, Closed }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DriveReport { pub progress: DriveProgress, pub status: ConnectionStatus }

/// Give the peer only the opposite socket, never this trusted-side connection.
/// A successful write acknowledges local socket acceptance, not peer delivery.
/// Retry uncertainty belongs to the original request key and effect ledger.
/// The default port remains in-memory. A durable port can perform synchronous
/// journal I/O during exchange; DriveBudget bounds socket work, not disk latency.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::actor_transport::UnixActorConnection;
/// fn escalate(connection: UnixActorConnection) { let _broker = connection.broker_mut(); }
/// ```
pub struct UnixActorConnection<P: ActorRequestPort = ActorPort> {
    socket: UnixStream,
    channel: ActorChannel<P>,
    input: [u8; SOCKET_BUFFER_BYTES],
    start: usize,
    end: usize,
    failure: Option<ConnectionFailure>,
}

impl<P: ActorRequestPort> fmt::Debug for UnixActorConnection<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnixActorConnection").field("status", &self.status()).finish_non_exhaustive()
    }
}

impl<P: ActorRequestPort> AsFd for UnixActorConnection<P> {
    fn as_fd(&self) -> BorrowedFd<'_> { self.socket.as_fd() }
}

impl<P: ActorRequestPort> UnixActorConnection<P> {
    /// No socket clone or task is spawned. A native scheduler may register the
    /// borrowed descriptor; this adapter does not supply another event loop.
    pub fn new(socket: UnixStream, channel: ActorChannel<P>) -> io::Result<Self> {
        socket.set_nonblocking(true)?;
        Ok(Self::from_nonblocking(socket, channel))
    }

    /// Internal constructor after peer authentication and nonblocking setup.
    /// No fallible syscall runs after the original actor session is moved here.
    pub(super) fn from_nonblocking(socket: UnixStream, channel: ActorChannel<P>) -> Self {
        Self { socket, channel, input: [0; SOCKET_BUFFER_BYTES], start: 0, end: 0, failure: None }
    }

    pub fn status(&self) -> ConnectionStatus {
        ConnectionStatus {
            channel: self.channel.state(), buffered_input_bytes: self.end - self.start,
            pending_output_bytes: self.channel.pending_output().len(), failure: self.failure,
        }
    }

    /// Invalid budgets refuse before any work. Later socket failures are returned
    /// WITH completed progress, then latched: another drive never retries intake.
    /// WouldBlock yields immediately; Interrupted consumes the I/O-attempt budget.
    pub fn drive(&mut self, budget: DriveBudget) -> Result<DriveReport, WireError> {
        budget.validate()?;
        let mut progress = DriveProgress::default();
        while !self.status().closed() {
            match self.channel.state() {
                ChannelState::Reading => {
                    if progress.frames == budget.frames { break; }
                    if self.start != self.end {
                        let fed = self.channel.feed(&self.input[self.start..self.end]);
                        self.start += fed.consumed;
                        progress.consumed_bytes += fed.consumed;
                        if fed.state == ChannelState::ReplyReady { progress.frames += 1; }
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
                        // Preserve the channel's write-then-flush barrier even
                        // when the byte budget ended on the last response byte.
                        progress.io_calls += 1;
                        match self.socket.flush() {
                            Ok(()) => { self.channel.acknowledge_flushed().expect("fully written channel response"); }
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
                                self.channel.acknowledge_written(count).expect("socket accepted no more than offered");
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

    /// Explicit operator reconnect: close this socket and retain the SAME actor
    /// ticket/idempotency session. Unfinished input and unsent replies are lost,
    /// not accepted effects. Clients retry the original key and exact proposal.
    pub fn into_session(self) -> ActorWire<P> { self.channel.into_wire() }

    fn fail(&mut self, direction: IoDirection, kind: io::ErrorKind) {
        self.failure = Some(ConnectionFailure { direction, kind });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::actor_wire::CloseReason;

    #[test]
    fn buffered_work_and_flushes_do_not_wait_for_a_new_readability_edge() {
        let mut status = ConnectionStatus { channel: ChannelState::Reading,
            buffered_input_bytes: 0, pending_output_bytes: 0, failure: None };
        assert_eq!(status.interest(), DriveInterest::Readable);
        status.buffered_input_bytes = 3;
        assert_eq!(status.interest(), DriveInterest::LocalWork);
        status.channel = ChannelState::ReplyReady; status.pending_output_bytes = 1;
        assert_eq!(status.interest(), DriveInterest::Writable);
        status.pending_output_bytes = 0;
        assert_eq!(status.interest(), DriveInterest::LocalWork);
        status.failure = Some(ConnectionFailure { direction: IoDirection::Write, kind: io::ErrorKind::BrokenPipe });
        assert_eq!(status.interest(), DriveInterest::Closed);
        status.failure = None; status.channel = ChannelState::Closed(CloseReason::PeerClosed);
        assert_eq!(status.interest(), DriveInterest::Closed);
    }
}
