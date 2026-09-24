//! One nonblocking accept boundary for a kernel-bound actor session.
//! The host owns socket naming/permissions and schedules readiness. No accept
//! loop, thread, subprocess, alternate codec, or authority ledger is introduced.

use super::{PeerAdmission, PeerRefusal, PeerSession, PeerSessionStatus};
use crate::action::consequence::oversight::actor::ActorPort;
use crate::action::consequence::oversight::actor_transport::{DriveBudget, DriveReport};
use crate::action::consequence::oversight::actor_wire::{ActorRequestPort, WireError};
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

/// One host-scheduled turn. Accept has a separate, explicit allowance: false
/// means no accept syscall, even when the drive budget permits socket work.
/// True permits at most one candidate, in addition to the drive's I/O budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListenerPollBudget {
    pub accept: bool,
    pub drive: DriveBudget,
}

impl Default for ListenerPollBudget {
    fn default() -> Self {
        Self { accept: true, drive: DriveBudget::default() }
    }
}

/// Supervisor-only results from one accept/drive turn, not effect authority.
/// Preserve BOTH outcomes: a refused candidate or accept I/O error must neither
/// starve an already admitted connection nor hide that connection's result.
/// Conversely, a drive error must not erase a successful connection admission.
#[derive(Debug)]
pub struct ListenerPollReport<D = DriveReport, E = WireError> {
    /// None means acceptance was not scheduled, not an empty listen backlog.
    pub accept: Option<Result<AcceptEvent, io::ErrorKind>>,
    /// None means there was no active connection after the accept attempt.
    pub drive: Option<Result<D, E>>,
}

/// Setup failure returns BOTH original owners; neither actor tickets nor the
/// already bound socket are discarded merely because nonblocking setup failed.
pub struct ListenerSetupFailure<P: ActorRequestPort = ActorPort> {
    pub error: io::Error,
    pub listener: UnixListener,
    pub session: PeerSession<P>,
}

impl<P: ActorRequestPort> fmt::Debug for ListenerSetupFailure<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ListenerSetupFailure").field("kind", &self.error.kind()).finish_non_exhaustive()
    }
}

/// Supply a listener bound in an operator-controlled namespace and one fixed
/// actor session. The peer cannot select another request port or authority scope.
/// The socket path is not created, unlinked or reused by this component.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::actor_peer::UnixPeerListener;
/// fn escape(listener: UnixPeerListener) { let _ = listener.broker_mut(); }
/// ```
pub struct UnixPeerListener<P: ActorRequestPort = ActorPort> {
    listener: Option<UnixListener>,
    session: PeerSession<P>,
}

impl<P: ActorRequestPort> fmt::Debug for UnixPeerListener<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnixPeerListener").field("status", &self.status()).finish_non_exhaustive()
    }
}

impl<P: ActorRequestPort> UnixPeerListener<P> {
    pub fn new(listener: UnixListener, session: PeerSession<P>) -> Result<Self, ListenerSetupFailure<P>> {
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

    /// Accept at most one candidate, then service the original active session.
    /// Invalid budgets refuse before acceptance. Busy or rejected candidates do
    /// not replace the active connection, its tickets or its pending response.
    /// An accept error is retained while existing work is still serviced.
    ///
    /// Source-backed owners must use their supervisor's source-aware poll, not
    /// this ordinary port drive. There is no public admission callback.
    ///
    /// ```compile_fail,E0624
    /// use fa_reference::action::consequence::oversight::actor_peer::{
    ///     ListenerPollBudget, UnixPeerListener,
    /// };
    /// fn bypass(listener: &mut UnixPeerListener) {
    ///     listener.poll_with(ListenerPollBudget::default(), |_, _| Ok::<_, ()>(()));
    /// }
    /// ```
    pub fn poll(&mut self, budget: ListenerPollBudget) -> Result<ListenerPollReport, WireError> {
        self.poll_with(budget, |session, drive| session.drive(drive))
    }

    /// Identity inspection is available only to existing trusted integrations.
    pub(crate) fn request_port(&self) -> &P { self.session.request_port() }

    /// Keep acceptance, peer authentication and ticket ownership in this owner;
    /// the original source-aware supervisor supplies only its existing drive.
    pub(crate) fn poll_with<D, E, F>(&mut self, budget: ListenerPollBudget, drive: F)
        -> Result<ListenerPollReport<D, E>, WireError>
    where F: FnOnce(&mut PeerSession<P>, DriveBudget) -> Result<D, E> {
        budget.drive.validate()?;
        let accept = if budget.accept {
            Some(self.accept_once().map_err(|error| error.kind()))
        } else {
            None
        };
        let drive = if self.session.status().active.is_some() {
            Some(drive(&mut self.session, budget.drive))
        } else {
            None
        };
        Ok(ListenerPollReport { accept, drive })
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
    pub fn into_session(self) -> PeerSession<P> { self.session }
}
