//! Retain original requests, observations and budgets across explicit reconnects.
use super::{ActorExchange, ClientError, ClientFailure, ClientIoBudget, ClientIoLimits, ClientIoWork,
    ClientPhase, ClientProgress, Command, ResponseError, WireError, WireResponse};
use super::super::super::actor::{ActorBasis, ActorOutcome, ActorProposal, Knowledge, MAX_ACTOR_BYTES, MAX_ACTOR_REQUESTS};
use crate::action::MAX_PAYLOAD_BYTES;
use std::collections::BTreeMap;
use std::fmt;
use std::io::{Read, Write};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClientSessionLimits { pub requests: usize, pub retained_payload_bytes: usize, pub io: ClientIoLimits }
impl Default for ClientSessionLimits {
    fn default() -> Self { Self { requests: MAX_ACTOR_REQUESTS, retained_payload_bytes: MAX_ACTOR_BYTES,
        io: ClientIoLimits::default() } }
}
struct Request {
    proposal: ActorProposal,
    visible: bool,
    response: Option<WireResponse>,
    terminal: Option<(ActorOutcome, ActorBasis)>,
}

/// Process-local client history for ONE independently provisioned server domain.
/// Reconnecting moves this owner; it does not clone requests or replenish work.
/// No server key, live permission, connection or clock is retained in this value.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::actor_wire::client::ActorClientState;
/// fn replenish(state: ActorClientState) { let _ = state.clone(); }
/// ```
pub struct ActorClientState {
    requests: BTreeMap<u64, Request>, limits: ClientSessionLimits, payload_bytes: usize,
    budget: ClientIoBudget, interrupted: Option<Command>, failure: Option<ClientFailure>,
}
impl fmt::Debug for ActorClientState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActorClientState").field("requests", &self.requests.len())
            .field("work", &self.budget.work()).finish_non_exhaustive()
    }
}
impl ActorClientState {
    pub fn new(limits: ClientSessionLimits) -> Result<Self, ClientError> {
        if limits.requests == 0 || limits.requests > MAX_ACTOR_REQUESTS
            || limits.retained_payload_bytes == 0 || limits.retained_payload_bytes > MAX_ACTOR_BYTES
        { return Err(ClientError::Limit); }
        Ok(Self { requests: BTreeMap::new(), payload_bytes: 0, budget: ClientIoBudget::new(limits.io)?,
            limits, interrupted: None, failure: None })
    }
    pub fn work(&self) -> ClientIoWork { self.budget.work() }
    pub fn limits(&self) -> ClientSessionLimits { self.limits }
    pub fn request_count(&self) -> usize { self.requests.len() }
    pub fn retained_payload_bytes(&self) -> usize { self.payload_bytes }
    pub fn original_proposal(&self, request: u64) -> Option<&ActorProposal> {
        self.requests.get(&request).map(|entry| &entry.proposal)
    }
    /// Historical observation, not a current availability or execution promise.
    pub fn last_response(&self, request: u64) -> Option<&WireResponse> {
        self.requests.get(&request)?.response.as_ref()
    }
    pub fn interrupted_command(&self) -> Option<&Command> { self.interrupted.as_ref() }
    pub fn last_failure(&self) -> Option<ClientFailure> { self.failure }

    /// Attach a stream already bound to the SAME server domain by the caller.
    /// This sends nothing. New wire sessions require exact Submit to recover
    /// ticket visibility before Poll or Cancel; numeric IDs alone do not suffice.
    pub fn connect<S>(mut self, stream: S) -> ActorClient<S> {
        for entry in self.requests.values_mut() { entry.visible = false; }
        ActorClient { state: self, stream: Some(stream), active: None }
    }

    #[cfg(unix)]
    pub fn connect_unix(self, stream: std::os::unix::net::UnixStream)
        -> Result<ActorClient<std::os::unix::net::UnixStream>, ClientConnectFailure>
    {
        if let Err(error) = stream.set_nonblocking(true) {
            return Err(ClientConnectFailure { error: ClientError::Io(error.kind()), state: self, stream });
        }
        Ok(self.connect(stream))
    }
}
#[cfg(unix)]
pub struct ClientConnectFailure {
    pub error: ClientError,
    pub state: ActorClientState,
    pub stream: std::os::unix::net::UnixStream,
}
#[cfg(unix)]
impl fmt::Debug for ClientConnectFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientConnectFailure").field("error", &self.error).finish_non_exhaustive()
    }
}

/// One in-flight exchange, no hidden polling, reconnection, resubmission or
/// replacement-key generation. The original server remains the sole authority.
pub struct ActorClient<S> { state: ActorClientState, stream: Option<S>, active: Option<ActorExchange<S>> }
impl<S> fmt::Debug for ActorClient<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActorClient").field("state", &self.state).field("phase", &self.phase()).finish_non_exhaustive()
    }
}
impl<S> ActorClient<S> {
    pub fn state(&self) -> &ActorClientState { &self.state }
    pub fn phase(&self) -> Option<ClientPhase> { self.active.as_ref().map(ActorExchange::phase) }
    pub fn connected(&self) -> bool { self.stream.is_some() || self.active.as_ref().is_some_and(|exchange| exchange.stream.is_some()) }

    pub fn submit(&mut self, request: u64, proposal: &ActorProposal) -> Result<(), ClientError> {
        if self.active.is_some() { return Err(ClientError::Busy); }
        if self.stream.is_none() { return Err(ClientError::Disconnected); }
        if let Some(previous) = self.state.requests.get(&request) {
            if &previous.proposal != proposal { return Err(ClientError::Command(WireError::IdempotencyConflict)); }
        } else {
            if proposal.payload.len() > MAX_PAYLOAD_BYTES { return Err(ClientError::Limit); }
            let bytes = self.state.payload_bytes.checked_add(proposal.payload.len()).ok_or(ClientError::Limit)?;
            if self.state.requests.len() == self.state.limits.requests || bytes > self.state.limits.retained_payload_bytes {
                return Err(ClientError::Limit);
            }
        }
        self.begin(Command::Submit { request, proposal: proposal.clone() })
    }
    /// Explicit exact retry only; it cannot replace payload, epoch or deadline.
    /// The server still decides whether this key is old, refused or newly admitted.
    pub fn retry_submission(&mut self, request: u64) -> Result<(), ClientError> {
        let proposal = self.state.original_proposal(request).ok_or(ClientError::TicketUnavailable)?.clone();
        self.submit(request, &proposal)
    }
    pub fn poll(&mut self, request: u64) -> Result<(), ClientError> { self.begin(Command::Poll { request }) }
    pub fn cancel(&mut self, request: u64) -> Result<(), ClientError> { self.begin(Command::Cancel { request }) }

    fn begin(&mut self, command: Command) -> Result<(), ClientError> {
        if self.active.is_some() { return Err(ClientError::Busy); }
        if self.stream.is_none() { return Err(ClientError::Disconnected); }
        let request = command.request();
        if !matches!(&command, Command::Submit { .. })
            && !self.state.requests.get(&request).is_some_and(|entry| entry.visible)
        { return Err(ClientError::TicketUnavailable); }
        let stream = self.stream.take().ok_or(ClientError::Disconnected)?;
        match ActorExchange::new(stream, command, &mut self.state.budget) {
            Ok(exchange) => {
                if let Command::Submit { proposal, .. } = exchange.command() {
                    if !self.state.requests.contains_key(&request) {
                        self.state.payload_bytes += proposal.payload.len();
                        self.state.requests.insert(request, Request { proposal: proposal.clone(), visible: false,
                            response: None, terminal: None });
                    }
                }
                self.active = Some(exchange); Ok(())
            }
            Err(failure) => { self.stream = Some(failure.stream); Err(failure.error) }
        }
    }

    /// Close only transport. Preserve every original proposal and tombstone,
    /// including an unacknowledged command; do not infer cancellation or refund.
    pub fn into_state(mut self) -> ActorClientState {
        if let Some(exchange) = self.active.take() {
            self.state.interrupted = Some(exchange.command().clone());
            if let Some(failure) = exchange.failure() { self.state.failure = Some(failure); }
        }
        for entry in self.state.requests.values_mut() { entry.visible = false; }
        self.state
    }
}
impl<S: Read + Write> ActorClient<S> {
    pub fn step(&mut self) -> Result<ClientProgress, ClientError> {
        let Some(exchange) = &mut self.active else {
            return if self.stream.is_some() { Ok(ClientProgress::Complete) } else { Err(ClientError::Disconnected) };
        };
        let progress = exchange.step(&mut self.state.budget);
        match progress {
            Ok(ClientProgress::Response(response)) => {
                let request = exchange.command().request();
                let entry = self.state.requests.get_mut(&request).expect("admitted client request");
                // A terminal outcome cannot become a different outcome or a
                // pending request. Unknown/Withheld after reconnect remain valid
                // availability observations without erasing terminal history.
                if let Some((old_value, old_basis)) = entry.terminal {
                    let invalid = match &response.result {
                        Ok(Knowledge::Known { value, basis }) => *value != old_value
                            || basis.source != old_basis.source || basis.generation < old_basis.generation,
                        Ok(Knowledge::Pending { .. }) => true,
                        _ => false,
                    };
                    if invalid {
                        let error = ClientError::Response(ResponseError::Binding);
                        exchange.fail(error);
                        self.state.failure = exchange.failure(); self.state.interrupted = Some(exchange.command().clone());
                        self.active = None; return Err(error);
                    }
                }
                if let Ok(Knowledge::Known { value, basis }) = &response.result { entry.terminal = Some((*value, *basis)); }
                if matches!(exchange.command(), Command::Submit { .. }) && response.result.is_ok() { entry.visible = true; }
                entry.response = Some(response.clone());
                if self.state.interrupted.as_ref().is_some_and(|command| command.request() == request) {
                    self.state.interrupted = None;
                }
                self.stream = self.active.take().expect("completed client exchange").into_stream();
                Ok(ClientProgress::Response(response))
            }
            Ok(progress) => Ok(progress),
            Err(error) => {
                self.state.failure = exchange.failure(); self.state.interrupted = Some(exchange.command().clone());
                self.active = None; Err(error)
            }
        }
    }
}
