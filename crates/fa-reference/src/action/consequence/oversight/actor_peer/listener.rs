//! One nonblocking accept boundary for a kernel-bound actor session.
//! The host owns socket naming/permissions and schedules readiness. No accept
//! loop, thread, subprocess, alternate codec, or authority ledger is introduced.

use super::{PeerAdmission, PeerRefusal, PeerSession, PeerSessionStatus};
use crate::action::consequence::oversight::actor_transport::{DriveBudget, DriveReport};
use crate::action::consequence::oversight::actor_wire::WireError;
use std::fmt;
use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixListener;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcceptEvent {
    /// No connection currently queued; no actor bytes were read.
    Idle,
    Admitted(PeerAdmission),
    /// Exactly one candidate was closed without a response or frame read.
    Rejected(PeerRefusal),
    Stopped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeerListenerStatus {
    pub listening: bool,
    pub session: PeerSessionStatus,
}

/// Setup failure returns BOTH original owners; neither actor tickets nor the
/// already bound socket are discarded merely because nonblocking setup failed.
pub struct ListenerSetupFailure {
    pub error: io::Error,
    pub listener: UnixListener,
    pub session: PeerSession,
}

impl fmt::Debug for ListenerSetupFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ListenerSetupFailure").field("kind", &self.error.kind()).finish_non_exhaustive()
    }
}

/// Supply a listener bound in an operator-controlled namespace and one fixed
/// actor session. The peer cannot select another ActorPort or authority scope.
/// The socket path is not created, unlinked or reused by this component.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::actor_peer::UnixPeerListener;
/// fn escape(listener: UnixPeerListener) { let _ = listener.broker_mut(); }
/// ```
pub struct UnixPeerListener {
    listener: Option<UnixListener>,
    session: PeerSession,
}

impl fmt::Debug for UnixPeerListener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnixPeerListener").field("status", &self.status()).finish_non_exhaustive()
    }
}

impl UnixPeerListener {
    pub fn new(listener: UnixListener, session: PeerSession) -> Result<Self, ListenerSetupFailure> {
        if let Err(error) = listener.set_nonblocking(true) {
            return Err(ListenerSetupFailure { error, listener, session });
        }
        Ok(Self { listener: Some(listener), session })
    }

    pub fn status(&self) -> PeerListenerStatus {
        let session = self.session.status();
        PeerListenerStatus { listening: self.listener.is_some() && !session.revoked, session }
    }

    pub fn listener_fd(&self) -> Option<BorrowedFd<'_>> {
        if self.session.status().revoked { return None; }
        self.listener.as_ref().map(AsFd::as_fd)
    }

    pub fn socket_fd(&self) -> Option<BorrowedFd<'_>> { self.session.socket_fd() }

    /// At most one accept syscall, followed by the original peer check. An
    /// Interrupted error returns to the scheduler; there is no hidden retry loop.
    /// Busy/revoked/quota failures never replace the original session or socket.
    pub fn accept_once(&mut self) -> io::Result<AcceptEvent> {
        if self.session.status().revoked { return Ok(AcceptEvent::Stopped); }
        let Some(listener) = &self.listener else { return Ok(AcceptEvent::Stopped); };
        let (socket, _) = match listener.accept() {
            Ok(accepted) => accepted,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(AcceptEvent::Idle),
            Err(error) => return Err(error),
        };
        Ok(match self.session.attach(socket) {
            Ok(admitted) => AcceptEvent::Admitted(admitted),
            Err(refused) => AcceptEvent::Rejected(refused),
        })
    }

    pub fn drive(&mut self, budget: DriveBudget) -> Result<DriveReport, WireError> {
        self.session.drive(budget)
    }

    /// A later candidate must authenticate again. No effect is cancelled.
    pub fn disconnect(&mut self) -> bool { self.session.disconnect() }

    /// Close listener and active ingress; retain accepted obligations only in
    /// their original supervisor. This is NOT a control-ledger or endpoint fence.
    pub fn revoke(&mut self) -> bool {
        let closed = self.listener.take().is_some();
        self.session.revoke() || closed
    }

    /// Move the SAME credential gate and ticket session to another explicitly
    /// provisioned listener. Revocation and lifetime connection counts survive.
    pub fn into_session(self) -> PeerSession { self.session }
}
