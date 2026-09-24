//! Source intake on the original kernel-credential-bound actor connection.
use super::{ElapsedTick, EvidenceFile, FileActorSupervisor, FileGeneratedTextActorPort,
    FileOversight, FileSupervisedDriver};
use crate::action::consequence::delivery::persistent::requests::actor::source_wire::{
    FileActorPeerDrive, FileActorPeerDriveError,
};
use crate::action::consequence::oversight::actor_peer::{
    ListenerPollBudget, ListenerPollReport, PeerSession, UnixPeerListener,
};
use crate::action::consequence::oversight::actor_transport::DriveBudget;
use crate::action::consequence::oversight::actor_wire::WireError;

impl FileActorSupervisor<FileOversight> {
    /// Service a named listener without extracting or replacing its peer session.
    /// Gateway identity and socket budgets are checked BEFORE accepting a peer.
    /// The original kernel credential gate then runs before source acquisition.
    /// At most one accept attempt and one bounded drive occur in a turn.
    ///
    /// Only complete new generated submissions read evidence. Idle, malformed,
    /// retry, poll and cancel paths keep their original read-free behavior. A
    /// Busy rejection still services the original peer, and both results remain
    /// available in the report. No human/congress key or generation step is added.
    pub fn poll_generated_listener_from_file<S, F>(&mut self,
        listener: &mut UnixPeerListener<FileGeneratedTextActorPort>, source: &mut S,
        clock: F, budget: ListenerPollBudget)
        -> Result<ListenerPollReport<FileActorPeerDrive, FileActorPeerDriveError>, FileActorPeerDriveError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.check_source_wire(&listener.request_port().port)?;
        Ok(listener.poll_with(budget, |session, drive| {
            self.drive_generated_peer_from_file(session, source, clock, drive)
        })?)
    }

    /// Drive this owner's ORIGINAL authenticated PeerSession with registered
    /// evidence acquisition at each well-formed new source-reference/finish.
    /// A foreign gateway refuses before socket reads. Kernel credential checks,
    /// revoked/disconnected state and reconnect limits belong to PeerSession.
    /// Malformed input, recorded retries, poll and cancel acquire no evidence.
    ///
    /// Preserve bounded socket work, write/flush backpressure and connection-local
    /// tickets. Intake diagnostics are in completed-frame order and private to
    /// the supervisor. Their capacity is admitted before any source transaction.
    /// Socket budgets do NOT bound synchronous source/journal latency or replay.
    /// No listener, peer-policy override, raw-port accessor or authority is added.
    pub fn drive_generated_peer_from_file<S, F>(&mut self,
        session: &mut PeerSession<FileGeneratedTextActorPort>, source: &mut S,
        mut clock: F, budget: DriveBudget) -> Result<FileActorPeerDrive, FileActorPeerDriveError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.check_source_wire(&session.request_port().port)?;
        budget.validate()?;
        let mut intakes = Vec::new();
        intakes.try_reserve_exact(budget.frames).map_err(|_| WireError::Capacity)?;
        let drive = session.drive_with_admission(budget, |_, request, proposal| {
            let mut intake = None;
            let result = self.prepare_generated_submission(request, proposal, source, &mut clock, &mut intake);
            if let Some(report) = intake { intakes.push(report); }
            result
        })?;
        Ok(FileActorPeerDrive { drive, intakes })
    }
}

impl FileSupervisedDriver {
    /// Use the same source-aware listener and retire helpers on every result,
    /// including idle turns, credential refusals and nested drive failures.
    pub fn poll_generated_listener_from_file<S, F>(&mut self,
        listener: &mut UnixPeerListener<FileGeneratedTextActorPort>, source: &mut S,
        clock: F, budget: ListenerPollBudget)
        -> Result<ListenerPollReport<FileActorPeerDrive, FileActorPeerDriveError>, FileActorPeerDriveError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let result = self.supervisor_mut().poll_generated_listener_from_file(listener, source, clock, budget);
        self.reap_helpers();
        result
    }

    /// Share the original supervisor and helper retirement path on every result.
    pub fn drive_generated_peer_from_file<S, F>(&mut self,
        session: &mut PeerSession<FileGeneratedTextActorPort>, source: &mut S,
        clock: F, budget: DriveBudget) -> Result<FileActorPeerDrive, FileActorPeerDriveError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let result = self.supervisor_mut().drive_generated_peer_from_file(session, source, clock, budget);
        self.reap_helpers();
        result
    }
}

#[cfg(test)]
mod listener_tests;
