//! Linux connection credentials before ANY reviewer-protocol I/O.
//! Reuse the original actor peer-policy checker; the credential namespace and
//! its limitations are identical. An authenticated process is not a human.
use super::{FileHumanRequest, FileHumanReviewer, FileOversight, ReviewerConnection, ReviewerError};
use super::client::{ReviewerClient, ReviewerExpectation};
pub use crate::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy};
use crate::Error;
use std::fmt;
use std::io;
use std::os::unix::net::UnixStream;

pub const MAX_REVIEWER_CANDIDATES: u32 = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewerPeerError {
    Exhausted,
    AlreadyAdmitted,
    Credentials(io::ErrorKind),
    Nonblocking(io::ErrorKind),
}
impl fmt::Display for ReviewerPeerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "reviewer peer: {self:?}") }
}
impl std::error::Error for ReviewerPeerError {}

/// The SAME connected socket whose kernel credentials passed a frozen rule.
/// No unchecked constructor, raw socket extraction, clone or credential setter
/// exists. Conversion uses the original reviewer protocol without reconnecting.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::VerifiedReviewerSocket;
/// fn duplicate(socket: VerifiedReviewerSocket) { let _ = socket.clone(); }
/// ```
pub struct VerifiedReviewerSocket {
    stream: UnixStream,
    peer: PeerCredentials,
    policy: PeerPolicy,
}
impl fmt::Debug for VerifiedReviewerSocket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VerifiedReviewerSocket").field("peer", &self.peer)
            .field("policy", &self.policy).finish_non_exhaustive()
    }
}
impl VerifiedReviewerSocket {
    /// Checks SO_PEERCRED on this socket before parsing input, allocating an
    /// evidence offer or writing anything. Rejection drops only this socket.
    /// Root has no exception; UID AND effective GID and any specified PID match.
    pub fn verify(stream: UnixStream, policy: PeerPolicy) -> Result<Self, ReviewerPeerError> {
        let peer = policy.verify(&stream).map_err(|error| ReviewerPeerError::Credentials(error.kind()))?;
        stream.set_nonblocking(true).map_err(|error| ReviewerPeerError::Nonblocking(error.kind()))?;
        Ok(Self { stream, peer, policy })
    }
    pub fn peer(&self) -> PeerCredentials { self.peer }
    pub fn policy(&self) -> PeerPolicy { self.policy }

    /// Host-side offer, still requiring the independently retained reviewer role
    /// and exact original request. Merely admitting a peer creates no decision,
    /// automatic reservation or human key; step() performs the original protocol.
    pub fn into_connection(self, host: &FileOversight, reviewer: &FileHumanReviewer,
        request: FileHumanRequest, session: [u8; 32])
        -> Result<ReviewerConnection<UnixStream>, ReviewerError>
    {
        ReviewerConnection::new(host, reviewer, request, self.stream, session)
    }

    /// Client-side server authentication BEFORE reading any claimed offer.
    /// The separately configured logical audience must still match the packet.
    /// A valid server identity never chooses a default reviewer decision.
    pub fn into_client(self, expected: ReviewerExpectation) -> Result<ReviewerClient<UnixStream>, Error> {
        ReviewerClient::new(self.stream, expected)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReviewerAdmissionStatus {
    pub maximum: u32,
    pub attempted: u32,
    pub rejected: u32,
    pub admitted: Option<PeerCredentials>,
}

/// A bounded, one-success admission gate for an independently provisioned offer.
/// The host performs its existing nonblocking accept/deadline scheduling. Wrong
/// peers cannot consume the human request, replace another connection or obtain
/// any protocol bytes. After success, additional candidates are closed untouched.
///
/// A hostile process can exhaust this quota: that causes refusal/stop, never a
/// weaker authentication rule. Starting a new gate is explicit host policy.
pub struct ReviewerPeerAdmission {
    policy: PeerPolicy,
    status: ReviewerAdmissionStatus,
}
impl fmt::Debug for ReviewerPeerAdmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReviewerPeerAdmission").field("policy", &self.policy)
            .field("status", &self.status).finish()
    }
}
impl ReviewerPeerAdmission {
    pub fn new(policy: PeerPolicy, maximum: u32) -> Result<Self, Error> {
        if maximum == 0 { return Err(Error::InvalidInput); }
        if maximum > MAX_REVIEWER_CANDIDATES { return Err(Error::Limit); }
        Ok(Self { policy, status: ReviewerAdmissionStatus {
            maximum, attempted: 0, rejected: 0, admitted: None,
        } })
    }
    pub fn status(&self) -> ReviewerAdmissionStatus { self.status }
    pub fn policy(&self) -> PeerPolicy { self.policy }
    pub fn exhausted(&self) -> bool { self.status.attempted == self.status.maximum }

    pub fn admit(&mut self, stream: UnixStream) -> Result<VerifiedReviewerSocket, ReviewerPeerError> {
        if self.status.admitted.is_some() { return Err(ReviewerPeerError::AlreadyAdmitted); }
        if self.exhausted() { return Err(ReviewerPeerError::Exhausted); }
        self.status.attempted += 1;
        match VerifiedReviewerSocket::verify(stream, self.policy) {
            Ok(socket) => {
                self.status.admitted = Some(socket.peer());
                Ok(socket)
            }
            Err(error) => { self.status.rejected += 1; Err(error) }
        }
    }
}
