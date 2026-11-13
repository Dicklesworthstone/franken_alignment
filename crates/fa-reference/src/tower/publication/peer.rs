//! Bounded Linux peer ingress for the original publication Tower session.
//!
//! The existing kernel-credential policy authenticates each socket before any
//! byte is read. The host fixes Tower grants and can install an independently
//! approved original promotion on this same session. Neither requests nor
//! reconnections can choose grants, replace the native owner, replenish
//! disclosure work or reset the lifetime exchange allowance.

mod channel;
mod connection;

use super::PublicationTower;
use crate::Error;
use crate::action::consequence::delivery::persistent::observed::FileOversight;
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileActorPort, FileSupervisorPolicyPromotionPort,
};
use crate::action::consequence::oversight::actor::{ActorError, ActorProposal};
use crate::action::consequence::oversight::actor_peer::{
    MAX_PEER_CONNECTIONS, PeerAdmission, PeerPolicy, PeerRefusal, PeerSessionStatus, PeerSetupStage,
};
use crate::action::consequence::oversight::actor_transport::{DriveBudget, DriveReport};
use crate::action::consequence::oversight::actor_wire::{
    ChannelLimits, MAX_CHANNEL_EXCHANGES, WireError,
};
use crate::tower::{MAX_REQUEST_BYTES, TowerAccess};
use channel::Channel;
use connection::Connection;
use std::fmt;
use std::io;
use std::os::fd::BorrowedFd;
use std::os::unix::net::UnixStream;

type Port = FileActorPort<FileOversight>;

/// One actor session and at most one authenticated socket. The original host
/// owns time, source intake, helper review, stop authority and publication.
/// Reconnecting preserves native tickets and all spent session allowances.
/// Socket scheduling uses the existing DriveBudget/DriveReport contract and
/// introduces no executor, listener, background task or hidden I/O loop.
///
/// ```compile_fail,E0599
/// use fa_reference::tower::publication::peer::PublicationPeerSession;
/// fn no_owner(session: PublicationPeerSession) { let _ = session.host_mut(); }
/// ```
pub struct PublicationPeerSession {
    policy: PeerPolicy,
    tower: PublicationTower,
    access: TowerAccess,
    channel: Channel,
    connection_limit: u64,
    connections: u64,
    connection: Option<Connection>,
    active: Option<PeerAdmission>,
    revoked: bool,
}

impl fmt::Debug for PublicationPeerSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PublicationPeerSession")
            .field("status", &self.status())
            .field("remaining_exchanges", &self.remaining_exchanges())
            .finish_non_exhaustive()
    }
}

impl PublicationPeerSession {
    pub fn new(
        policy: PeerPolicy,
        tower: PublicationTower,
        access: TowerAccess,
        limits: ChannelLimits,
        connection_limit: u64,
    ) -> Result<Self, Error> {
        if limits.frame_bytes == 0 || limits.exchanges == 0 || connection_limit == 0 {
            return Err(Error::InvalidInput);
        }
        if limits.frame_bytes > MAX_REQUEST_BYTES
            || limits.exchanges > MAX_CHANNEL_EXCHANGES
            || connection_limit > MAX_PEER_CONNECTIONS
        {
            return Err(Error::Limit);
        }
        Ok(Self {
            policy,
            tower,
            access,
            channel: Channel::new(limits),
            connection_limit,
            connections: 0,
            connection: None,
            active: None,
            revoked: false,
        })
    }

    pub fn status(&self) -> PeerSessionStatus {
        PeerSessionStatus {
            connections_admitted: self.connections,
            connection_limit: self.connection_limit,
            active: self.active,
            transport: self
                .connection
                .as_ref()
                .map(|connection| connection.status(&self.channel)),
            revoked: self.revoked,
        }
    }

    pub fn remaining_exchanges(&self) -> u64 {
        self.channel.remaining_exchanges()
    }

    /// Host-only installation of an independently approved original campaign,
    /// including while a socket or reply is retained. The same Tower checks the
    /// original owner and withdraws its previous installation on a foreign
    /// replacement. This does not recreate the channel, reset counters, discard
    /// a pending reply, approve a policy or revive a revoked ingress.
    pub fn set_policy_promotion(
        &mut self,
        port: Option<FileSupervisorPolicyPromotionPort>,
    ) -> Result<(), Error> {
        if self.revoked {
            return Err(Error::WrongState);
        }
        self.tower
            .set_policy_promotion(port)
            .map_err(|_| Error::Binding)
    }

    pub fn socket_fd(&self) -> Option<BorrowedFd<'_>> {
        self.connection.as_ref().map(Connection::socket_fd)
    }

    /// Invalid candidates close without reading, changing grants or replacing
    /// an existing connection. UID/GID and optional PID use the original gate.
    pub fn attach(&mut self, socket: UnixStream) -> Result<PeerAdmission, PeerRefusal> {
        if self.revoked {
            return Err(PeerRefusal::Revoked);
        }
        if self.connection.is_some() {
            return Err(PeerRefusal::Busy);
        }
        if self.connections == self.connection_limit || self.remaining_exchanges() == 0 {
            return Err(PeerRefusal::Capacity);
        }
        let credentials = self.policy.verify(&socket).map_err(|error| {
            if error.kind() == io::ErrorKind::PermissionDenied {
                PeerRefusal::CredentialsRejected
            } else {
                PeerRefusal::Io {
                    stage: PeerSetupStage::Credentials,
                    kind: error.kind(),
                }
            }
        })?;
        socket
            .set_nonblocking(true)
            .map_err(|error| PeerRefusal::Io {
                stage: PeerSetupStage::Nonblocking,
                kind: error.kind(),
            })?;
        self.connections += 1;
        let admission = PeerAdmission {
            connection: self.connections,
            credentials,
        };
        self.connection = Some(Connection::new(socket));
        self.active = Some(admission);
        Ok(admission)
    }

    /// Use only when the host separately supplies one-use admission snapshots.
    /// Normal live hosts use FileSupervisedDriver's source-aware Tower methods.
    /// This never refreshes source evidence or invents a current clock.
    pub fn drive(&mut self, budget: DriveBudget) -> Result<DriveReport, WireError> {
        self.drive_with_admission(budget, |_, _, _| Ok(()))
    }

    pub(crate) fn request_port(&self) -> &Port {
        self.tower.request_port()
    }

    /// Private host callback after complete Tower/native validation and ticket
    /// capacity checks, immediately before original durable submission.
    pub(crate) fn drive_with_admission<A>(
        &mut self,
        budget: DriveBudget,
        mut admission: A,
    ) -> Result<DriveReport, WireError>
    where
        A: FnMut(&Port, u64, &ActorProposal) -> Result<(), ActorError>,
    {
        if self.revoked {
            return Err(WireError::Withheld);
        }
        let connection = self.connection.as_mut().ok_or(WireError::Unavailable)?;
        let tower = &mut self.tower;
        let access = &self.access;
        connection.drive(&mut self.channel, budget, |document| {
            tower
                .exchange_with_admission(access, document, &mut admission)
                .to_json()
        })
    }

    /// Disconnect only transport. A lost reply is retried with the same native
    /// request key and exact proposal; no acknowledged action is rolled back.
    pub fn disconnect(&mut self) -> bool {
        if self.connection.take().is_none() {
            return false;
        }
        self.channel.reset_transport();
        self.active = None;
        true
    }

    /// Permanently retire this ingress, without claiming to stop, cancel or
    /// refund effects. The supervisor's independent Stop path remains necessary.
    pub fn revoke(&mut self) -> bool {
        if self.revoked {
            return false;
        }
        self.disconnect();
        self.revoked = true;
        true
    }
}
