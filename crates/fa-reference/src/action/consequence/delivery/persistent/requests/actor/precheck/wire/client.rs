//! One correlated query against an explicitly authenticated local server.
//!
//! This is not the actor submission client: it creates no ticket, effect key,
//! journal, retry, reconnect or runtime. A lost reply may still have consumed the
//! server's original parser/evaluation allowances. Retrying is a NEW query, even
//! if its label and bytes are identical; this client never performs that retry.

use super::{
    MAX_PRECHECK_FRAME_BYTES, MAX_PRECHECK_RESPONSE_BYTES, PrecheckRequest,
    PrecheckResponseError, PrecheckWireError, PrecheckWireResponse, decode_precheck_response,
    encode_precheck_request,
};
use super::discovery::{MAX_CAPABILITIES_REQUEST_BYTES, PrecheckCapabilitiesRequest,
    PrecheckCapabilitiesResponse, decode_capabilities_response};
use super::policy::{MAX_POLICY_REQUEST_BYTES, MAX_POLICY_RESPONSE_BYTES,
    PolicyRequest, PolicyResponse, decode_policy_response};
use crate::action::consequence::oversight::actor_peer::{
    PeerCredentials, PeerPolicy, PeerRefusal, PeerSetupStage,
};
use crate::action::consequence::oversight::actor_transport::{
    DriveBudget, DriveInterest, DriveProgress, IoDirection, SOCKET_BUFFER_BYTES,
};
use std::fmt;
use std::io::{self, Read, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixStream;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrecheckClientError {
    Peer(PeerRefusal),
    Request(PrecheckWireError),
    ExpectedPolicyEpoch,
    InvalidBudget,
    Io { direction: IoDirection, kind: io::ErrorKind },
    TruncatedResponse,
    ResponseTooLarge,
    TrailingResponseData,
    Response(PrecheckResponseError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrecheckClientPhase { Writing, Flushing, Reading, Confirming, Complete, Failed }

/// Scheduling/transport facts only. No private proposal or response bytes are
/// exposed, and completion is not an assertion of effect admission or safety.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrecheckClientStatus {
    pub phase: PrecheckClientPhase,
    pub pending_request_bytes: usize,
    pub buffered_response_bytes: usize,
    pub response_available: bool,
    pub failure: Option<PrecheckClientError>,
}

impl PrecheckClientStatus {
    pub fn closed(self) -> bool {
        matches!(self.phase, PrecheckClientPhase::Complete | PrecheckClientPhase::Failed)
    }

    pub fn interest(self) -> DriveInterest {
        match self.phase {
            PrecheckClientPhase::Writing => DriveInterest::Writable,
            // The final flush and boundary probe need not produce another edge.
            PrecheckClientPhase::Flushing | PrecheckClientPhase::Confirming => DriveInterest::LocalWork,
            PrecheckClientPhase::Reading => DriveInterest::Readable,
            PrecheckClientPhase::Complete | PrecheckClientPhase::Failed => DriveInterest::Closed,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrecheckClientDriveReport {
    pub progress: DriveProgress,
    pub status: PrecheckClientStatus,
}

/// The independently supplied PeerPolicy is checked on THIS connected socket
/// before any request bytes are written. Kernel UID/GID/optional PID identify
/// the connection peer, not executable code or the recipient of a transferred
/// descriptor. There is no unchecked constructor, socket clone or reconnect.
///
/// One newline-terminated reply is staged before decoding. A further bounded
/// read rejects queued trailing/coalesced bytes, even across tiny drive budgets.
/// EOF or WouldBlock after that line confirms only the currently available frame
/// boundary: WouldBlock does not prove that no future peer bytes could arrive.
/// The socket is closed on completion and never reused for another exchange.
/// Response types select compiled codecs through separate authenticated
/// constructors; callers cannot install their own decoder or raw request bytes.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::client::PrecheckPeerClient;
/// fn no_effect(client: PrecheckPeerClient) { client.submit(1); }
/// ```
pub struct PrecheckPeerClient<R = PrecheckWireResponse> {
    socket: Option<UnixStream>,
    credentials: PeerCredentials,
    exchange: Exchange<R>,
}

/// The same authenticated, bounded, one-shot I/O engine, with a discovery codec.
/// A discovery observation cannot be taken as an exact-policy pass.
pub type PrecheckCapabilitiesClient = PrecheckPeerClient<PrecheckCapabilitiesResponse>;
/// Full definition observations use a distinct codec and bounded receive buffer.
pub type PrecheckPolicyClient = PrecheckPeerClient<PolicyResponse>;

impl<R> fmt::Debug for PrecheckPeerClient<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PrecheckPeerClient")
            .field("credentials", &self.credentials)
            .field("status", &self.status())
            .finish_non_exhaustive()
    }
}

impl PrecheckPeerClient {
    /// Takes an already connected socket; performs no pathname lookup, listener
    /// operation or blocking connection attempt. Any refusal closes this candidate
    /// without sending bytes. The expected epoch is independent caller context and
    /// must agree with the request, including when the expected epoch is zero.
    pub fn connect(socket: UnixStream, policy: PeerPolicy, request: PrecheckRequest,
        expected_policy_epoch: u64,
    ) -> Result<Self, PrecheckClientError> {
        Self::authenticated(socket, policy, || Exchange::new(request, expected_policy_epoch))
    }
}

impl PrecheckPeerClient<PrecheckCapabilitiesResponse> {
    pub fn connect_capabilities(socket: UnixStream, policy: PeerPolicy,
        request: PrecheckCapabilitiesRequest, expected_policy_epoch: u64,
    ) -> Result<Self, PrecheckClientError> {
        Self::authenticated(socket, policy, || {
            if request.expected_policy_epoch != expected_policy_epoch {
                return Err(PrecheckClientError::ExpectedPolicyEpoch);
            }
            let document = request.encode().map_err(PrecheckClientError::Request)?;
            Exchange::from_document(document, request.query, expected_policy_epoch,
                MAX_CAPABILITIES_REQUEST_BYTES, decode_capabilities_response)
        })
    }
}

impl PrecheckPeerClient<PolicyResponse> {
    pub fn connect_policy(socket: UnixStream, policy: PeerPolicy,
        request: PolicyRequest, expected_policy_epoch: u64,
    ) -> Result<Self, PrecheckClientError> {
        Self::authenticated(socket, policy, || {
            if request.expected_policy_epoch != expected_policy_epoch {
                return Err(PrecheckClientError::ExpectedPolicyEpoch);
            }
            let document = request.encode().map_err(PrecheckClientError::Request)?;
            Exchange::from_document_bounded(document, request.query, expected_policy_epoch,
                MAX_POLICY_REQUEST_BYTES, MAX_POLICY_RESPONSE_BYTES, decode_policy_response)
        })
    }
}

impl<R> PrecheckPeerClient<R> {
    fn authenticated(socket: UnixStream, policy: PeerPolicy,
        build: impl FnOnce() -> Result<Exchange<R>, PrecheckClientError>,
    ) -> Result<Self, PrecheckClientError> {
        let credentials = policy.verify(&socket).map_err(|error| {
            PrecheckClientError::Peer(if error.kind() == io::ErrorKind::PermissionDenied {
                PeerRefusal::CredentialsRejected
            } else {
                PeerRefusal::Io { stage: PeerSetupStage::Credentials, kind: error.kind() }
            })
        })?;
        let exchange = build()?;
        socket.set_nonblocking(true).map_err(|error| {
            PrecheckClientError::Peer(PeerRefusal::Io {
                stage: PeerSetupStage::Nonblocking, kind: error.kind(),
            })
        })?;
        Ok(Self { socket: Some(socket), credentials, exchange })
    }

    pub fn credentials(&self) -> PeerCredentials { self.credentials }
    pub fn status(&self) -> PrecheckClientStatus { self.exchange.status() }

    /// Scheduler-only descriptor, absent once this one exchange terminates.
    pub fn socket_fd(&self) -> Option<BorrowedFd<'_>> {
        self.socket.as_ref().map(AsFd::as_fd)
    }

    /// Invalid budgets refuse without work. Valid calls return actual progress
    /// together with any latched failure; driving a terminal client never does
    /// I/O. Interrupted and WouldBlock each spend one I/O attempt. Zero frame or
    /// I/O allowance yields without work; at most one response frame is entered.
    /// The whole request must drain AND flush before the first response read.
    pub fn drive(&mut self, budget: DriveBudget)
        -> Result<PrecheckClientDriveReport, PrecheckClientError>
    {
        budget.validate().map_err(|_| PrecheckClientError::InvalidBudget)?;
        let report = if let Some(socket) = &mut self.socket {
            self.exchange.drive_io(socket, budget)?
        } else {
            PrecheckClientDriveReport { progress: DriveProgress::default(), status: self.status() }
        };
        if report.status.closed() { self.socket = None; }
        Ok(report)
    }

    /// Exactly one strict correlated response, including a valid server-side
    /// refusal. None means pending, failed, or already taken; inspect status to
    /// distinguish them. The returned Knowledge never carries effect authority.
    pub fn take_response(&mut self) -> Option<R> { self.exchange.response.take() }
}

// Private reusable I/O core permits deterministic partial-I/O fault tests, not
// an unauthenticated public transport constructor. Production uses UnixStream.
struct Exchange<R = PrecheckWireResponse> {
    request: Vec<u8>,
    written: usize,
    query: u64,
    expected_policy_epoch: u64,
    input: Vec<u8>,
    input_len: usize,
    phase: PrecheckClientPhase,
    failure: Option<PrecheckClientError>,
    response: Option<R>,
    decode: fn(&[u8], u64, u64) -> Result<R, PrecheckResponseError>,
}

impl Exchange {
    fn new(request: PrecheckRequest, expected_policy_epoch: u64) -> Result<Self, PrecheckClientError> {
        if request.proposal.expected_policy_epoch != expected_policy_epoch {
            return Err(PrecheckClientError::ExpectedPolicyEpoch);
        }
        let document = encode_precheck_request(&request).map_err(PrecheckClientError::Request)?;
        Self::from_document(document, request.query, expected_policy_epoch,
            MAX_PRECHECK_FRAME_BYTES, decode_precheck_response)
    }
}

impl<R> Exchange<R> {
    fn from_document(document: Vec<u8>, query: u64, expected_policy_epoch: u64,
        maximum: usize, decode: fn(&[u8], u64, u64) -> Result<R, PrecheckResponseError>,
    ) -> Result<Self, PrecheckClientError> {
        Self::from_document_bounded(document, query, expected_policy_epoch, maximum,
            MAX_PRECHECK_RESPONSE_BYTES, decode)
    }

    fn from_document_bounded(mut document: Vec<u8>, query: u64, expected_policy_epoch: u64,
        maximum: usize, response_limit: usize,
        decode: fn(&[u8], u64, u64) -> Result<R, PrecheckResponseError>,
    ) -> Result<Self, PrecheckClientError> {
        if document.len() > maximum || response_limit == 0 || response_limit > MAX_POLICY_RESPONSE_BYTES {
            return Err(PrecheckClientError::Request(PrecheckWireError::Capacity));
        }
        document.try_reserve(1).map_err(|_| PrecheckClientError::Request(PrecheckWireError::Capacity))?;
        document.push(b'\n');
        let mut input = Vec::new();
        input.try_reserve_exact(response_limit)
            .map_err(|_| PrecheckClientError::Request(PrecheckWireError::Capacity))?;
        input.resize(response_limit, 0);
        Ok(Self { request: document, written: 0, query, expected_policy_epoch,
            input, input_len: 0,
            phase: PrecheckClientPhase::Writing, failure: None, response: None, decode })
    }

    fn status(&self) -> PrecheckClientStatus {
        PrecheckClientStatus { phase: self.phase,
            pending_request_bytes: self.request.len() - self.written,
            buffered_response_bytes: self.input_len,
            response_available: self.response.is_some(), failure: self.failure }
    }

    fn drive_io<T: Read + Write>(&mut self, socket: &mut T, budget: DriveBudget)
        -> Result<PrecheckClientDriveReport, PrecheckClientError>
    {
        budget.validate().map_err(|_| PrecheckClientError::InvalidBudget)?;
        let mut progress = DriveProgress::default();
        while !self.status().closed() && progress.io_calls < budget.io_calls && budget.frames != 0 {
            match self.phase {
                PrecheckClientPhase::Writing => {
                    if progress.written_bytes == budget.write_bytes { break; }
                    let length = (self.request.len() - self.written)
                        .min(budget.write_bytes - progress.written_bytes);
                    progress.io_calls += 1;
                    match socket.write(&self.request[self.written..self.written + length]) {
                        Ok(0) => self.fail_io(IoDirection::Write, io::ErrorKind::WriteZero),
                        Ok(count) if count <= length => {
                            self.written += count;
                            progress.written_bytes += count;
                            if self.written == self.request.len() { self.phase = PrecheckClientPhase::Flushing; }
                        }
                        Ok(_) => self.fail_io(IoDirection::Write, io::ErrorKind::InvalidData),
                        Err(error) => if self.io_error(error, IoDirection::Write, &mut progress) { break; },
                    }
                }
                PrecheckClientPhase::Flushing => {
                    progress.io_calls += 1;
                    match socket.flush() {
                        Ok(()) => {
                            self.request.clear(); self.written = 0;
                            self.phase = PrecheckClientPhase::Reading;
                        }
                        Err(error) => if self.io_error(error, IoDirection::Flush, &mut progress) { break; },
                    }
                }
                PrecheckClientPhase::Reading | PrecheckClientPhase::Confirming => {
                    if progress.read_bytes == budget.read_bytes { break; }
                    let mut bytes = [0; SOCKET_BUFFER_BYTES];
                    let length = if self.phase == PrecheckClientPhase::Confirming { 1 }
                        else { SOCKET_BUFFER_BYTES.min(budget.read_bytes - progress.read_bytes) };
                    progress.io_calls += 1;
                    match socket.read(&mut bytes[..length]) {
                        Ok(0) if self.phase == PrecheckClientPhase::Confirming => self.finish(),
                        Ok(0) => self.fail(PrecheckClientError::TruncatedResponse),
                        Ok(count) if count <= length => {
                            progress.read_bytes += count;
                            if self.phase == PrecheckClientPhase::Confirming {
                                progress.consumed_bytes += count;
                                self.fail(PrecheckClientError::TrailingResponseData);
                            } else {
                                self.feed(&bytes[..count], &mut progress);
                            }
                        }
                        Ok(_) => self.fail_io(IoDirection::Read, io::ErrorKind::InvalidData),
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock
                            && self.phase == PrecheckClientPhase::Confirming => {
                                progress.would_block = Some(IoDirection::Read);
                                self.finish();
                                break;
                            }
                        Err(error) => if self.io_error(error, IoDirection::Read, &mut progress) { break; },
                    }
                }
                PrecheckClientPhase::Complete | PrecheckClientPhase::Failed => break,
            }
        }
        Ok(PrecheckClientDriveReport { progress, status: self.status() })
    }

    fn feed(&mut self, bytes: &[u8], progress: &mut DriveProgress) {
        for (index, &byte) in bytes.iter().enumerate() {
            progress.consumed_bytes += 1;
            if byte == b'\n' {
                progress.frames += 1;
                if index + 1 != bytes.len() { self.fail(PrecheckClientError::TrailingResponseData); }
                else { self.phase = PrecheckClientPhase::Confirming; }
                return;
            }
            if self.input_len == self.input.len() {
                self.fail(PrecheckClientError::ResponseTooLarge);
                return;
            }
            self.input[self.input_len] = byte;
            self.input_len += 1;
        }
    }

    fn finish(&mut self) {
        match (self.decode)(&self.input[..self.input_len], self.query, self.expected_policy_epoch) {
            Ok(response) => {
                self.response = Some(response);
                self.input_len = 0;
                self.phase = PrecheckClientPhase::Complete;
            }
            Err(error) => self.fail(PrecheckClientError::Response(error)),
        }
    }

    fn io_error(&mut self, error: io::Error, direction: IoDirection, progress: &mut DriveProgress) -> bool {
        match error.kind() {
            io::ErrorKind::Interrupted => false,
            io::ErrorKind::WouldBlock => { progress.would_block = Some(direction); true }
            kind => { self.fail_io(direction, kind); true }
        }
    }

    fn fail_io(&mut self, direction: IoDirection, kind: io::ErrorKind) {
        self.fail(PrecheckClientError::Io { direction, kind });
    }

    fn fail(&mut self, error: PrecheckClientError) {
        self.failure = Some(error);
        self.phase = PrecheckClientPhase::Failed;
        self.request.clear(); self.written = 0; self.input_len = 0;
        self.response = None;
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod policy_tests;
