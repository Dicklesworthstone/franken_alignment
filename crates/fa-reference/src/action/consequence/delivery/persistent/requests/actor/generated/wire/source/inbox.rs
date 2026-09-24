//! Native generated requests on the original bounded, journal-checked inbox.
mod pool;

use super::{ElapsedTick, EvidenceFile, FileGeneratedTextActorPort, FileSupervisedDriver, JournalError};
use crate::action::consequence::delivery::persistent::requests::FileRequestStatus;
use crate::action::consequence::delivery::persistent::requests::actor::FileActorInbox;
use crate::action::consequence::delivery::persistent::requests::actor::source_wire::{
    FileActorPeerDrive, FileActorPeerDriveError,
};
use crate::action::consequence::oversight::actor_transport::DriveBudget;

impl FileActorInbox<FileGeneratedTextActorPort> {
    /// Drive source-only requests through the SAME kernel-bound session and
    /// ready queue as ordinary actors. The fixed generated-intent validator runs
    /// before file/time acquisition; only the original durable request status
    /// can create a work hint. Neither source preparation nor a delivered reply
    /// is substituted for acknowledged admission.
    ///
    /// Repeated IDs are deduplicated while queued. An exact retry may wake an
    /// already dequeued request, but dequeue rechecks its current journal stage.
    /// Partial frames, malformed references, polls and cancellations never read
    /// evidence; complete new valid submissions consume their original one-use
    /// source observations. Preparation success is not admission, and a hint
    /// for an existing request never grants new authority after a refusal.
    pub fn drive<S, F>(&mut self, driver: &mut FileSupervisedDriver, source: &mut S,
        mut clock: F, budget: DriveBudget) -> Result<FileActorPeerDrive, FileActorPeerDriveError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.drive_prepared(driver, budget,
            |supervisor, port| supervisor.check_source_wire(&port.port),
            |supervisor, request, proposal, intake| {
                supervisor.prepare_generated_submission(request, proposal, source, &mut clock, intake)
            })
    }

    /// Return the next ORIGINAL request still needing supervision. Reviewing
    /// remains subject to full-input congress and the separately held human key;
    /// Dispatching/Unknown is reconciliation-only, never permission to replay.
    /// Cancelled and terminal hints are skipped. A foreign or faulted owner
    /// cannot consume a hint, and ingress revocation does not erase obligations.
    pub fn next_request(&mut self, driver: &FileSupervisedDriver)
        -> Result<Option<FileRequestStatus>, JournalError>
    {
        self.next_checked(driver, |supervisor, port| supervisor.check_source_wire(&port.port))
    }
}

#[cfg(test)]
mod tests;
