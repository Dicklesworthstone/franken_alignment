//! Actor-side request I/O over an explicitly provisioned original server domain.
//! Receipt parsing is not authentication and no response authorizes an effect.
use super::{Command, MAX_RESPONSE_BYTES, WireError, WireResponse, encode_command};
use super::response::{ResponseError, decode_response};
use std::fmt;
use std::io::{self, Read, Write};

pub const CLIENT_IO_CHUNK: usize = 4096;
pub const MAX_CLIENT_IO_CALLS: u64 = 65_536;
pub const MAX_CLIENT_IO_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientError {
    Command(WireError), Response(ResponseError), Io(io::ErrorKind),
    Limit, Busy, Disconnected, TicketUnavailable,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClientIoLimits { pub exchanges: u64, pub calls: u64, pub written_bytes: u64, pub read_bytes: u64 }
impl Default for ClientIoLimits {
    fn default() -> Self { Self { exchanges: super::MAX_CHANNEL_EXCHANGES, calls: MAX_CLIENT_IO_CALLS,
        written_bytes: MAX_CLIENT_IO_BYTES, read_bytes: MAX_CLIENT_IO_BYTES } }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClientIoWork { pub exchanges: u64, pub calls: u64, pub written_bytes: u64, pub read_bytes: u64 }
/// Owned lifetime allowance. Retry, Interrupted, WouldBlock and EOF spend calls.
/// Nothing refunds work or resets this allowance after a transport failure.
#[derive(Debug)]
pub struct ClientIoBudget { limits: ClientIoLimits, work: ClientIoWork }
impl ClientIoBudget {
    pub fn new(limits: ClientIoLimits) -> Result<Self, ClientError> {
        if limits.exchanges == 0 || limits.exchanges > super::MAX_CHANNEL_EXCHANGES
            || limits.calls == 0 || limits.calls > MAX_CLIENT_IO_CALLS
            || limits.written_bytes == 0 || limits.written_bytes > MAX_CLIENT_IO_BYTES
            || limits.read_bytes == 0 || limits.read_bytes > MAX_CLIENT_IO_BYTES
        { return Err(ClientError::Limit); }
        Ok(Self { limits, work: ClientIoWork::default() })
    }
    pub fn limits(&self) -> ClientIoLimits { self.limits }
    pub fn work(&self) -> ClientIoWork { self.work }
    fn admit(&mut self, bytes: usize) -> Result<(), ClientError> {
        if self.work.exchanges == self.limits.exchanges || self.work.calls == self.limits.calls
            || bytes as u64 > self.limits.written_bytes - self.work.written_bytes
            || self.work.read_bytes == self.limits.read_bytes
        { return Err(ClientError::Limit); }
        self.work.exchanges += 1; Ok(())
    }
    fn call(&mut self) -> Result<(), ClientError> {
        if self.work.calls == self.limits.calls { return Err(ClientError::Limit); }
        self.work.calls += 1; Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientPhase { Sending, Receiving, Complete, Failed }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClientFailure {
    pub error: ClientError,
    pub request: u64,
    pub written_bytes: usize,
    /// Conservative: true after ANY write call, even one returning an error.
    /// This concerns request delivery, not execution, cancellation or a refund.
    pub request_may_have_reached_peer: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientProgress { Progress, Blocked, Response(WireResponse), Complete }

pub struct ClientStartFailure<S> { pub error: ClientError, pub stream: S }
impl<S> fmt::Debug for ClientStartFailure<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientStartFailure").field("error", &self.error).finish_non_exhaustive()
    }
}

/// Exactly one original command and one bounded newline response. Generic I/O
/// requires the caller's nonblocking/bounded stream contract; no thread or runtime
/// is created. No reply is read until the full command AND flush complete.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::actor_wire::client::ActorExchange;
/// fn permission(exchange: ActorExchange<std::io::Cursor<Vec<u8>>>) { exchange.permit(); }
/// ```
pub struct ActorExchange<S> {
    stream: Option<S>, command: Command, output: Vec<u8>, written: usize,
    input: Vec<u8>, phase: ClientPhase, attempted_write: bool,
    response: Option<WireResponse>, failure: Option<ClientFailure>,
}
impl<S> fmt::Debug for ActorExchange<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActorExchange").field("request", &self.command.request())
            .field("phase", &self.phase).field("failure", &self.failure).finish_non_exhaustive()
    }
}
impl<S> ActorExchange<S> {
    /// On preflight failure return the untouched stream. No work is admitted
    /// until command encoding and bounded buffer preparation have succeeded.
    pub fn new(stream: S, command: Command, budget: &mut ClientIoBudget) -> Result<Self, ClientStartFailure<S>> {
        let prepared = (|| {
            let mut output = encode_command(&command).map_err(ClientError::Command)?;
            output.try_reserve(1).map_err(|_| ClientError::Limit)?; output.push(b'\n');
            let mut input = Vec::new();
            input.try_reserve_exact(MAX_RESPONSE_BYTES).map_err(|_| ClientError::Limit)?;
            budget.admit(output.len())?;
            Ok::<_, ClientError>((output, input))
        })();
        match prepared {
            Ok((output, input)) => Ok(Self { stream: Some(stream), command, output, written: 0,
                input, phase: ClientPhase::Sending, attempted_write: false, response: None, failure: None }),
            Err(error) => Err(ClientStartFailure { error, stream }),
        }
    }
    pub fn phase(&self) -> ClientPhase { self.phase }
    pub fn command(&self) -> &Command { &self.command }
    pub fn response(&self) -> Option<&WireResponse> { self.response.as_ref() }
    pub fn failure(&self) -> Option<ClientFailure> { self.failure }
    /// Only a completed exchange returns a reusable stream. Abandonment or a
    /// failure closes the transport without cancelling anything at the server.
    pub fn into_stream(mut self) -> Option<S> {
        if self.phase == ClientPhase::Complete { self.stream.take() } else { None }
    }
    fn fail(&mut self, error: ClientError) -> ClientError {
        self.failure = Some(ClientFailure { error, request: self.command.request(), written_bytes: self.written,
            request_may_have_reached_peer: self.attempted_write });
        self.phase = ClientPhase::Failed; self.stream = None;
        self.input.clear(); self.output.clear(); error
    }
}
impl<S: Read + Write> ActorExchange<S> {
    /// At most one read OR one write, plus a final flush. All I/O attempts are
    /// charged before calling the stream, including transient failures. A result
    /// is delivered once; subsequent calls return Complete without I/O or replay.
    pub fn step(&mut self, budget: &mut ClientIoBudget) -> Result<ClientProgress, ClientError> {
        if let Some(failure) = self.failure { return Err(failure.error); }
        if self.phase == ClientPhase::Complete { return Ok(ClientProgress::Complete); }
        match self.step_inner(budget) { Ok(progress) => Ok(progress), Err(error) => Err(self.fail(error)) }
    }
    fn step_inner(&mut self, budget: &mut ClientIoBudget) -> Result<ClientProgress, ClientError> {
        if self.phase == ClientPhase::Sending {
            if self.written < self.output.len() {
                let allowed = usize::try_from(budget.limits.written_bytes - budget.work.written_bytes)
                    .map_err(|_| ClientError::Limit)?;
                let offered = (self.output.len() - self.written).min(CLIENT_IO_CHUNK).min(allowed);
                if offered == 0 { return Err(ClientError::Limit); }
                budget.call()?; self.attempted_write = true;
                let stream = self.stream.as_mut().ok_or(ClientError::Disconnected)?;
                match stream.write(&self.output[self.written..self.written + offered]) {
                    Ok(0) => return Err(ClientError::Io(io::ErrorKind::WriteZero)),
                    Ok(count) if count <= offered => { self.written += count; budget.work.written_bytes += count as u64; }
                    Ok(_) => return Err(ClientError::Io(io::ErrorKind::InvalidData)),
                    Err(error) if transient(&error) => return Ok(ClientProgress::Blocked),
                    Err(error) => return Err(ClientError::Io(error.kind())),
                }
                if self.written != self.output.len() { return Ok(ClientProgress::Progress); }
            }
            budget.call()?;
            match self.stream.as_mut().ok_or(ClientError::Disconnected)?.flush() {
                Ok(()) => { self.output.clear(); self.phase = ClientPhase::Receiving; Ok(ClientProgress::Progress) }
                Err(error) if transient(&error) => Ok(ClientProgress::Blocked),
                Err(error) => Err(ClientError::Io(error.kind())),
            }
        } else {
            let mut buffer = [0_u8; MAX_RESPONSE_BYTES + 1];
            let allowed = usize::try_from(budget.limits.read_bytes - budget.work.read_bytes).map_err(|_| ClientError::Limit)?;
            let offered = (MAX_RESPONSE_BYTES + 1 - self.input.len()).min(allowed);
            if offered == 0 { return Err(ClientError::Limit); }
            budget.call()?;
            let count = match self.stream.as_mut().ok_or(ClientError::Disconnected)?.read(&mut buffer[..offered]) {
                Ok(0) => return Err(ClientError::Io(io::ErrorKind::UnexpectedEof)),
                Ok(count) if count <= offered => count,
                Ok(_) => return Err(ClientError::Io(io::ErrorKind::InvalidData)),
                Err(error) if transient(&error) => return Ok(ClientProgress::Blocked),
                Err(error) => return Err(ClientError::Io(error.kind())),
            };
            budget.work.read_bytes += count as u64;
            if let Some(end) = buffer[..count].iter().position(|byte| *byte == b'\n') {
                if end + 1 != count { return Err(ClientError::Response(ResponseError::Malformed)); }
                self.input.extend_from_slice(&buffer[..end]);
                let response = decode_response(&self.input).map_err(ClientError::Response)?;
                // Null IDs are useful parser diagnostics but cannot acknowledge
                // this correctly encoded command or be assigned to another one.
                if response.request != Some(self.command.request()) { return Err(ClientError::Response(ResponseError::Binding)); }
                self.input.clear(); self.phase = ClientPhase::Complete;
                self.response = Some(response.clone());
                Ok(ClientProgress::Response(response))
            } else {
                if self.input.len() + count > MAX_RESPONSE_BYTES { return Err(ClientError::Response(ResponseError::Capacity)); }
                self.input.extend_from_slice(&buffer[..count]); Ok(ClientProgress::Progress)
            }
        }
    }
}
fn transient(error: &io::Error) -> bool { matches!(error.kind(), io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock) }
