//! Original kernel-credential admission, bounded socket drive and live intake.
use super::{EvidenceFile, ElapsedTick, FileActorSupervisor, FileLearnedTextActorPort,
    FileLearnedTextStreamActorPort, FileOversight, PolicyPort, prepare};
use crate::action::consequence::delivery::persistent::requests::actor::source_wire::{
    FileActorPeerDrive, FileActorPeerDriveError};
use crate::action::consequence::oversight::actor_peer::{PeerSession, UnixPeerListener,
    ListenerPollBudget, ListenerPollReport};
use crate::action::consequence::oversight::actor_transport::DriveBudget;
use crate::action::consequence::oversight::actor_wire::WireError;

fn drive<P, S, F>(supervisor: &mut FileActorSupervisor<FileOversight>, session: &mut PeerSession<P>,
    source: &mut S, mut clock: F, budget: DriveBudget) -> Result<FileActorPeerDrive, FileActorPeerDriveError>
where P: PolicyPort, S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
    supervisor.check_source_wire(session.request_port().original_port())?;
    budget.validate()?;
    let mut intakes = Vec::new();
    intakes.try_reserve_exact(budget.frames).map_err(|_| WireError::Capacity)?;
    let drive = session.drive_with_admission(budget, |port, request, proposal| {
        let mut intake = None;
        let result = prepare(supervisor, port, request, proposal, source, &mut clock, &mut intake);
        if let Some(report) = intake { intakes.push(report); }
        result
    })?;
    Ok(FileActorPeerDrive { drive, intakes })
}

fn poll<P, S, F>(supervisor: &mut FileActorSupervisor<FileOversight>, listener: &mut UnixPeerListener<P>,
    source: &mut S, clock: F, budget: ListenerPollBudget)
    -> Result<ListenerPollReport<FileActorPeerDrive, FileActorPeerDriveError>, FileActorPeerDriveError>
where P: PolicyPort, S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
    supervisor.check_source_wire(listener.request_port().original_port())?;
    Ok(listener.poll_with(budget, |session, budget| drive(supervisor, session, source, clock, budget))?)
}

impl FileActorSupervisor<FileOversight> {
    /// Original authenticated peer; only complete new source-only requests read
    /// policy evidence. Credentials, revocation and ticket custody remain in
    /// PeerSession. Capacity is reserved before intake. Socket budgets do not
    /// bound synchronous policy-file, journal or numerical replay latency.
    pub fn drive_learned_text_peer_from_policy_file<S, F>(&mut self,
        session: &mut PeerSession<FileLearnedTextActorPort>, source: &mut S, clock: F, budget: DriveBudget)
        -> Result<FileActorPeerDrive, FileActorPeerDriveError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        drive(self, session, source, clock, budget)
    }

    /// The original stream release law also holds over authenticated sockets.
    pub fn drive_learned_text_stream_peer_from_policy_file<S, F>(&mut self,
        session: &mut PeerSession<FileLearnedTextStreamActorPort>, source: &mut S, clock: F, budget: DriveBudget)
        -> Result<FileActorPeerDrive, FileActorPeerDriveError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        drive(self, session, source, clock, budget)
    }

    /// Check ownership and original budgets before accepting a named-socket
    /// peer. Kernel credentials are checked before any source read. Busy/idle,
    /// accept and drive outcomes remain distinct in the original poll report.
    pub fn poll_learned_text_listener_from_policy_file<S, F>(&mut self,
        listener: &mut UnixPeerListener<FileLearnedTextActorPort>, source: &mut S, clock: F, budget: ListenerPollBudget)
        -> Result<ListenerPollReport<FileActorPeerDrive, FileActorPeerDriveError>, FileActorPeerDriveError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        poll(self, listener, source, clock, budget)
    }

    /// Same listener/credential contract, with source-derived Message/Finish.
    pub fn poll_learned_text_stream_listener_from_policy_file<S, F>(&mut self,
        listener: &mut UnixPeerListener<FileLearnedTextStreamActorPort>, source: &mut S, clock: F, budget: ListenerPollBudget)
        -> Result<ListenerPollReport<FileActorPeerDrive, FileActorPeerDriveError>, FileActorPeerDriveError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        poll(self, listener, source, clock, budget)
    }
}
