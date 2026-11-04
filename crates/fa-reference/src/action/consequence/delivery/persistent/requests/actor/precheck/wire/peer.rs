//! Linux kernel-credential admission for one separately granted precheck wire.
//! Recorded connection UID/GID/PID is not executable or remote identity, nor
//! proof of the current sender after a descriptor is transferred elsewhere.

use super::{FilePrecheckChannel, FilePrecheckWire, MAX_PRECHECK_FRAME_BYTES,
    PrecheckWireError, UnixPrecheckConnection};
use crate::action::consequence::oversight::actor_peer::{MAX_PEER_CONNECTIONS,
    PeerAdmission, PeerPolicy, PeerRefusal, PeerSessionStatus, PeerSetupStage};
use crate::action::consequence::oversight::actor_transport::{DriveBudget, DriveReport};
use crate::Error;
use std::fmt;
use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixStream;

/// One original finite disclosure wire and one active authenticated socket.
/// Reconnection keeps that wire; it neither renews a grant nor resets parsing
/// or evaluation work. Keep the separate native disclosure with the supervisor.
///
/// No raw connection, port, owner or custodian can be extracted from this
/// session. Its borrowed descriptor is for the trusted host's scheduler only.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::FilePrecheckPeerSession;
/// fn no_authority(session: FilePrecheckPeerSession) { let _ = session.host_mut(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::FilePrecheckPeerSession;
/// fn no_escape(session: FilePrecheckPeerSession) { let _ = session.into_session(); }
/// ```
pub struct FilePrecheckPeerSession {
    policy: PeerPolicy,
    connection_limit: u64,
    connections: u64,
    channel: Option<FilePrecheckChannel>,
    connection: Option<UnixPrecheckConnection>,
    active: Option<PeerAdmission>,
    revoked: bool,
}

impl fmt::Debug for FilePrecheckPeerSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilePrecheckPeerSession").field("status", &self.status()).finish_non_exhaustive()
    }
}

impl FilePrecheckPeerSession {
    pub fn new(policy: PeerPolicy, wire: FilePrecheckWire, frame_bytes: usize, connection_limit: u64)
        -> Result<Self, Error>
    {
        if frame_bytes == 0 || connection_limit == 0 { return Err(Error::InvalidInput); }
        if frame_bytes > MAX_PRECHECK_FRAME_BYTES || connection_limit > MAX_PEER_CONNECTIONS {
            return Err(Error::Limit);
        }
        let channel = FilePrecheckChannel::new(wire, frame_bytes).map_err(|_| Error::Limit)?;
        Ok(Self { policy, connection_limit, connections: 0, channel: Some(channel),
            connection: None, active: None, revoked: false })
    }

    pub fn policy(&self) -> PeerPolicy { self.policy }

    pub fn status(&self) -> PeerSessionStatus {
        PeerSessionStatus { connections_admitted: self.connections, connection_limit: self.connection_limit,
            active: self.active, transport: self.connection.as_ref().map(UnixPrecheckConnection::status),
            revoked: self.revoked }
    }

    pub fn socket_fd(&self) -> Option<BorrowedFd<'_>> {
        self.connection.as_ref().map(AsFd::as_fd)
    }

    /// Verify real kernel credentials before reading a frame. Refused candidates
    /// are closed without moving the original wire or changing active custody.
    /// An accepted reconnect receives only the original remaining allowance.
    pub fn attach(&mut self, socket: UnixStream) -> Result<PeerAdmission, PeerRefusal> {
        if self.revoked { return Err(PeerRefusal::Revoked); }
        if self.connection.is_some() { return Err(PeerRefusal::Busy); }
        if self.connections == self.connection_limit { return Err(PeerRefusal::Capacity); }
        let credentials = self.policy.verify(&socket).map_err(|error| {
            if error.kind() == io::ErrorKind::PermissionDenied { PeerRefusal::CredentialsRejected }
            else { PeerRefusal::Io { stage: PeerSetupStage::Credentials, kind: error.kind() } }
        })?;
        socket.set_nonblocking(true).map_err(|error| PeerRefusal::Io {
            stage: PeerSetupStage::Nonblocking, kind: error.kind(),
        })?;
        // All bounded channel storage was allocated at construction. No fallible
        // work remains after moving the original channel out of this session.
        let channel = self.channel.take().expect("disconnected precheck session");
        self.connection = Some(UnixPrecheckConnection::from_nonblocking(socket, channel));
        self.connections += 1;
        let admission = PeerAdmission { connection: self.connections, credentials };
        self.active = Some(admission);
        Ok(admission)
    }

    pub fn drive(&mut self, budget: DriveBudget) -> Result<DriveReport, PrecheckWireError> {
        if self.revoked { return Err(PrecheckWireError::Withheld); }
        self.connection.as_mut().ok_or(PrecheckWireError::Unavailable)?.drive(budget)
    }

    /// Drop transport fragments/replies only. An evaluated query stays spent;
    /// even a lost response cannot make a repeat query free.
    pub fn disconnect(&mut self) -> bool {
        let Some(connection) = self.connection.take() else { return false; };
        self.channel = Some(connection.into_channel());
        self.active = None;
        true
    }

    /// Irreversibly close this read ingress. This does not revoke the native
    /// grant's other ports, cancel an effect, or refund any allowance.
    pub fn revoke(&mut self) -> bool {
        if self.revoked { return false; }
        self.disconnect();
        self.revoked = true;
        true
    }
}
