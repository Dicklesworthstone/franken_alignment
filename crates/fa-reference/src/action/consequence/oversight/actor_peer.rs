//! Linux kernel-credential binding for the existing actor transport.
//!
//! This authenticates a connected socket's recorded UID/GID/PID, not executable
//! code, a human, a remote identity, or the current holder of a transferred FD.
//! The supervisor and its effect authority never enter this component.

use super::actor_transport::{ConnectionStatus, DriveBudget, DriveReport, UnixActorConnection};
use super::actor_wire::{ActorChannel, ActorWire, ChannelLimits, MAX_CHANNEL_EXCHANGES, MAX_FRAME_BYTES, WireError};
use crate::Error;
use std::fmt;
use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixStream;

pub const MAX_PEER_CONNECTIONS: u64 = 1_024;

/// Kernel-observed connection credentials. Fields cannot be supplied to attach.
/// These identify the peer at connection creation, not each subsequent sender.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeerCredentials {
    uid: u32,
    gid: u32,
    pid: u32,
}

impl PeerCredentials {
    pub fn uid(self) -> u32 { self.uid }
    pub fn gid(self) -> u32 { self.gid }
    pub fn pid(self) -> u32 { self.pid }

    /// Calls the safe standard-library SO_PEERCRED interface on THIS socket.
    /// Linux must supply a positive PID; absent/unmapped credentials refuse.
    pub fn observe(socket: &UnixStream) -> io::Result<Self> {
        let credentials = socket.peer_cred()?;
        let pid = credentials.pid.and_then(|pid| u32::try_from(pid).ok())
            .filter(|pid| *pid != 0)
            .ok_or_else(|| io::Error::from(io::ErrorKind::PermissionDenied))?;
        if credentials.uid == u32::MAX || credentials.gid == u32::MAX {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        Ok(Self { uid: credentials.uid, gid: credentials.gid, pid })
    }
}

/// Trusted bootstrap rule. UID AND effective GID must match; an optional PID
/// further narrows that rule. No wildcard/root exception or in-band identity.
/// UID/GID-only mode admits all connecting processes with those credentials.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeerPolicy {
    uid: u32,
    gid: u32,
    pid: Option<u32>,
}

impl PeerPolicy {
    pub fn new(uid: u32, gid: u32, pid: Option<u32>) -> Result<Self, Error> {
        if uid == u32::MAX || gid == u32::MAX
            || pid.is_some_and(|pid| pid == 0 || pid > i32::MAX as u32)
        { return Err(Error::InvalidInput); }
        Ok(Self { uid, gid, pid })
    }

    pub fn uid(self) -> u32 { self.uid }
    pub fn gid(self) -> u32 { self.gid }
    pub fn pid(self) -> Option<u32> { self.pid }

    /// Also usable by a local client to verify its server BEFORE sending data.
    /// A caller-supplied credential object is never accepted as authentication.
    pub fn verify(self, socket: &UnixStream) -> io::Result<PeerCredentials> {
        let observed = PeerCredentials::observe(socket)?;
        if observed.uid != self.uid || observed.gid != self.gid
            || self.pid.is_some_and(|pid| pid != observed.pid)
        { return Err(io::Error::from(io::ErrorKind::PermissionDenied)); }
        Ok(observed)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerSetupStage { Credentials, Nonblocking }

/// Supervisor-only diagnostics. No response is sent to a refused socket.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerRefusal {
    Revoked,
    Busy,
    Capacity,
    CredentialsRejected,
    Io { stage: PeerSetupStage, kind: io::ErrorKind },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeerAdmission {
    pub connection: u64,
    pub credentials: PeerCredentials,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeerSessionStatus {
    pub connections_admitted: u64,
    pub connection_limit: u64,
    pub active: Option<PeerAdmission>,
    pub transport: Option<ConnectionStatus>,
    pub revoked: bool,
}

/// One original actor session, at most one socket, and a frozen peer policy.
/// There is no ActorPort getter, policy-widening method, raw connection getter,
/// effect cancellation, supervisor accessor or permission inferred from a reply.
/// Reconnects retain original tickets and mailbox limits, not fresh authority.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::actor_peer::PeerSession;
/// fn escape(session: PeerSession) { let _ = session.broker_mut(); }
/// ```
pub struct PeerSession {
    policy: PeerPolicy,
    limits: ChannelLimits,
    connection_limit: u64,
    connections: u64,
    wire: Option<ActorWire>,
    connection: Option<UnixActorConnection>,
    active: Option<PeerAdmission>,
    revoked: bool,
}

impl fmt::Debug for PeerSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PeerSession").field("status", &self.status()).finish_non_exhaustive()
    }
}

impl PeerSession {
    pub fn new(
        policy: PeerPolicy, wire: ActorWire, limits: ChannelLimits, connection_limit: u64,
    ) -> Result<Self, Error> {
        if limits.frame_bytes == 0 || limits.exchanges == 0 || connection_limit == 0 {
            return Err(Error::InvalidInput);
        }
        if limits.frame_bytes > MAX_FRAME_BYTES || limits.exchanges > MAX_CHANNEL_EXCHANGES
            || connection_limit > MAX_PEER_CONNECTIONS
        { return Err(Error::Limit); }
        Ok(Self { policy, limits, connection_limit, connections: 0, wire: Some(wire),
            connection: None, active: None, revoked: false })
    }

    pub fn policy(&self) -> PeerPolicy { self.policy }

    pub fn status(&self) -> PeerSessionStatus {
        PeerSessionStatus { connections_admitted: self.connections, connection_limit: self.connection_limit,
            active: self.active, transport: self.connection.as_ref().map(UnixActorConnection::status),
            revoked: self.revoked }
    }

    /// This descriptor belongs to the trusted scheduler, never the actor. Like
    /// the underlying transport's AsFd, it grants no controller access.
    pub fn socket_fd(&self) -> Option<BorrowedFd<'_>> {
        self.connection.as_ref().map(AsFd::as_fd)
    }

    /// Inspect real peer credentials BEFORE reading a frame or moving the wire.
    /// Every refusal closes only the supplied candidate. It leaves the original
    /// ticket session, active connection and admission count unchanged.
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
        // All recoverable setup checks precede moving the original session.
        // Limits are immutable and checked at construction. No syscall occurs in
        // from_nonblocking, and no externally supplied callback can run here.
        let channel = ActorChannel::new(self.wire.take().expect("disconnected session"), self.limits)
            .expect("validated immutable channel limits");
        let connection = UnixActorConnection::from_nonblocking(socket, channel);
        self.connections += 1;
        let admission = PeerAdmission { connection: self.connections, credentials };
        self.connection = Some(connection);
        self.active = Some(admission);
        Ok(admission)
    }

    pub fn drive(&mut self, budget: DriveBudget) -> Result<DriveReport, WireError> {
        if self.revoked { return Err(WireError::Withheld); }
        self.connection.as_mut().ok_or(WireError::Unavailable)?.drive(budget)
    }

    /// Close only the transport. Already accepted requests stay in the original
    /// mailbox. A later connection must authenticate again; exact submission
    /// retries retain the original idempotency keys and lifetime resource limits.
    pub fn disconnect(&mut self) -> bool {
        let Some(connection) = self.connection.take() else { return false; };
        self.wire = Some(connection.into_session());
        self.active = None;
        true
    }

    /// Irreversibly withdraw ingress for this session. This does NOT revoke an
    /// effect permit, cancel queued work, prove nonexecution, or refund resources.
    /// The original supervisor must use its separate control transitions for that.
    pub fn revoke(&mut self) -> bool {
        if self.revoked { return false; }
        self.disconnect();
        self.revoked = true;
        true
    }
}
