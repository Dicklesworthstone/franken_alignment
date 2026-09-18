//! An actual actor executable on the ORIGINAL restricted request transport.
//! Launch and child polling are synchronous, not an executor or an OS sandbox.
//! The host must isolate ambient filesystem/network rights and private stderr.
use super::actor::{ActorError, ActorPort, ActorProposal};
use super::actor_transport::{ConnectionStatus, DriveBudget, DriveReport, UnixActorConnection};
use super::actor_wire::{ActorChannel, ActorRequestPort, ActorWire, ChannelLimits,
    MAX_CHANNEL_EXCHANGES, MAX_FRAME_BYTES, WireError};
use super::actor_wire::client::{ActorClient, ActorClientState, ClientError};
use super::helper_processes::{HelperProgram, OwnedChild};
pub use super::helper_processes::{ProcessExit, ProcessFailure, ProcessStage, ProcessStatus,
    MAX_PROGRAM_ARGUMENTS, MAX_PROGRAM_ENVIRONMENT, MAX_PROGRAM_FIELD_BYTES, MAX_PROGRAM_TEXT_BYTES};
use crate::Error;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

/// Operator-only program data, using the original bounded launcher validation.
/// The child receives no supervisor, helper socket, human role or effect key.
/// An explicitly selected interpreter is allowed; no shell or PATH lookup is added.
#[derive(Debug)]
pub struct ActorProgram(HelperProgram);
impl ActorProgram {
    pub fn new(executable: PathBuf, directory: PathBuf, arguments: Vec<OsString>,
        environment: BTreeMap<OsString, OsString>) -> Result<Self, Error>
    {
        HelperProgram::new(executable, directory, arguments, environment).map(Self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActorLaunchFailure { Protocol(WireError), Process(ProcessFailure) }

/// No child exists on returned failure. The original ticket session is returned
/// untouched, instead of losing it when program inspection or spawn refuses.
pub struct ActorLaunchError<P: ActorRequestPort = ActorPort> {
    pub failure: ActorLaunchFailure,
    pub session: ActorWire<P>,
}
impl<P: ActorRequestPort> fmt::Debug for ActorLaunchError<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActorLaunchError").field("failure", &self.failure).finish_non_exhaustive()
    }
}

/// Process observations do not settle actions. Even a successful exit or a
/// confirmed kill leaves the original request/effect ledger authoritative.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActorProcessStatus {
    pub child: ProcessStatus,
    pub transport: Option<ConnectionStatus>,
    pub ingress_closed: bool,
    pub interrupted_drive: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActorProcessDrive {
    pub drive: DriveReport,
    pub process: ActorProcessStatus,
}

/// Own exactly one direct child and its request connection. No auto-restart,
/// hidden thread, new wire verb, effect dispatch, process-tree claim or wait loop.
/// The private inherited socket is a trusted launch handoff, NOT SO_PEERCRED
/// authentication of an executable. Descendants and other inherited descriptors
/// remain operator responsibilities; do not give an unisolated actor real secrets.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::actor_process::ActorProcess;
/// fn authority(actor: ActorProcess) { actor.broker_mut(); }
/// ```
#[must_use = "stop the actor and poll until its direct child is reaped"]
pub struct ActorProcess<P: ActorRequestPort = ActorPort> {
    child: OwnedChild,
    connection: Option<UnixActorConnection<P>>,
    session: Option<ActorWire<P>>,
    interrupted: bool,
}
impl<P: ActorRequestPort> fmt::Debug for ActorProcess<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActorProcess").field("status", &self.status()).finish()
    }
}
impl<P: ActorRequestPort> ActorProcess<P> {
    /// Create all bounded protocol state BEFORE spawn. All fallible OS setup also
    /// precedes spawn. On success there is exactly one retained direct child.
    /// stdin is a private full-duplex socket; stdout/stderr go to PRIVATE host
    /// diagnostics, never the actor protocol. Only explicit environment survives.
    pub fn launch(program: &ActorProgram, session: ActorWire<P>, limits: ChannelLimits)
        -> Result<Self, ActorLaunchError<P>>
    {
        // Same immutable bounds as ActorChannel::new, checked without consuming
        // the caller's ticket session. Construction below is the actual validator.
        let failure = if limits.frame_bytes == 0 || limits.exchanges == 0 {
            Some(ActorLaunchFailure::Protocol(WireError::MalformedRequest))
        } else if limits.frame_bytes > MAX_FRAME_BYTES || limits.exchanges > MAX_CHANNEL_EXCHANGES {
            Some(ActorLaunchFailure::Protocol(WireError::Capacity))
        } else { program.0.preflight().err().map(ActorLaunchFailure::Process) };
        if let Some(failure) = failure { return Err(ActorLaunchError { failure, session }); }
        let channel = ActorChannel::new(session, limits).expect("validated immutable channel limits");
        match program.0.spawn_socket() {
            Ok((socket, child)) => Ok(Self { child: OwnedChild::new(child),
                connection: Some(UnixActorConnection::from_nonblocking(socket, channel)),
                session: None, interrupted: false }),
            Err(failure) => Err(ActorLaunchError { failure: ActorLaunchFailure::Process(failure),
                session: channel.into_wire() }),
        }
    }
    pub fn status(&self) -> ActorProcessStatus {
        ActorProcessStatus { child: self.child.status(),
            transport: self.connection.as_ref().map(UnixActorConnection::status),
            ingress_closed: self.connection.is_none(), interrupted_drive: self.interrupted }
    }
    pub fn socket_fd(&self) -> Option<BorrowedFd<'_>> {
        self.connection.as_ref().map(AsFd::as_fd)
    }
    /// One try_wait and, if requested, at most one kill. Exit closes ingress but
    /// is NEVER transformed into a cancellation, nonexecution receipt or refund.
    pub fn poll(&mut self) -> ActorProcessStatus {
        self.child.poll();
        if self.child.reaped() { self.close_ingress(); }
        self.status()
    }
    /// Close request ingress BEFORE requesting direct-child termination. Killing
    /// is not reaping; retain this owner and poll. Existing actions are untouched.
    pub fn request_stop(&mut self) -> ActorProcessStatus {
        self.close_ingress();
        self.child.request_stop();
        self.status()
    }
    fn close_ingress(&mut self) {
        if let Some(connection) = self.connection.take() { self.session = Some(connection.into_session()); }
    }
    /// Recover the SAME ticket session only after observing/reaping this child.
    /// No automatic restart, reconnect or replay follows. Running ownership is
    /// returned intact on refusal, including its stop/reap obligations.
    pub fn into_session(mut self) -> Result<ActorWire<P>, Box<Self>> {
        if !self.child.reaped() { return Err(Box::new(self)); }
        self.close_ingress();
        Ok(self.session.take().expect("closed actor retains original session"))
    }
    pub fn drive(&mut self, budget: DriveBudget) -> Result<ActorProcessDrive, WireError> {
        self.drive_with_admission(budget, |_, _, _| Ok(()))
    }
    pub(crate) fn request_port(&self) -> &P {
        if let Some(connection) = &self.connection { connection.request_port() }
        else { self.session.as_ref().expect("closed actor retains session").request_port() }
    }
    /// Only existing trusted source integrations may install admission work.
    /// An unwind leaves a latched interruption; another call cannot repeat intake.
    pub(crate) fn drive_with_admission<A>(&mut self, budget: DriveBudget, admission: A)
        -> Result<ActorProcessDrive, WireError>
    where A: FnMut(&P, u64, &ActorProposal) -> Result<(), ActorError> {
        // Budget refusal precedes process polling and all socket/source work.
        budget.validate()?;
        if self.interrupted {
            self.request_stop();
            return Err(WireError::Unavailable);
        }
        if self.connection.is_none() { return Err(WireError::Withheld); }
        let observed = self.poll().child;
        if observed.exit.is_some() || observed.failure.is_some() {
            if observed.failure.is_some() { self.request_stop(); }
            return Err(WireError::Unavailable);
        }
        self.interrupted = true;
        let result = self.connection.as_mut().expect("live actor connection")
            .drive_with_admission(budget, admission);
        self.interrupted = false;
        let drive = result?;
        if drive.status.closed() { self.close_ingress(); }
        Ok(ActorProcessDrive { drive, process: self.status() })
    }
}
impl<P: ActorRequestPort> Drop for ActorProcess<P> {
    fn drop(&mut self) {
        self.request_stop();
        if !self.child.reaped() { self.child.poll(); }
        if !self.child.reaped() {
            eprintln!("franken_alignment: direct actor child not reaped on drop; retain and poll ActorProcess during shutdown");
        }
    }
}

/// An unsuccessful inherited-socket attachment keeps the original client state.
/// No command, time observation or request retry is emitted during attachment.
#[derive(Debug)]
pub struct ActorStdinFailure { pub error: ClientError, pub state: ActorClientState }
impl ActorClientState {
    /// Worker-side counterpart of ActorProcess::launch, using safe owned-fd APIs.
    /// Ordinary file/pipe stdin is refused before setting nonblocking mode.
    pub fn connect_process_stdin(self) -> Result<ActorClient<UnixStream>, ActorStdinFailure> {
        let socket = (|| -> io::Result<UnixStream> {
            let input = io::stdin();
            let descriptor = input.as_fd().try_clone_to_owned()?;
            let socket = UnixStream::from(descriptor);
            socket.peer_addr()?;
            socket.set_nonblocking(true)?;
            Ok(socket)
        })();
        match socket {
            Ok(socket) => Ok(self.connect(socket)),
            Err(error) => Err(ActorStdinFailure { error: ClientError::Io(error.kind()), state: self }),
        }
    }
}
