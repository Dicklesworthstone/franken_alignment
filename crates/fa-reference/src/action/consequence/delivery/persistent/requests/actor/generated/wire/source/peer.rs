//! Source intake on the original kernel-credential-bound actor connection.
use super::{ElapsedTick, EvidenceFile, FileActorSupervisor, FileGeneratedTextActorPort,
    FileOversight, FileSupervisedDriver};
use crate::action::consequence::delivery::persistent::requests::actor::source_wire::{
    FileActorPeerDrive, FileActorPeerDriveError,
};
use crate::action::consequence::oversight::actor_peer::PeerSession;
use crate::action::consequence::oversight::actor_transport::DriveBudget;
use crate::action::consequence::oversight::actor_wire::WireError;

impl FileActorSupervisor<FileOversight> {
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
