//! Native generation references on the same fair multi-peer intake scheduler.
use super::{ElapsedTick, EvidenceFile, FileGeneratedTextActorPort, FileSupervisedDriver,
    FileActorPeerDriveError, JournalError};
use super::super::decode;
use crate::action::consequence::delivery::persistent::requests::actor::source_wire::inbox::pool::{
    FileActorPool, PoolBudget, PoolDriveReport, PoolReady, require_recorded,
};

impl FileActorPool<FileGeneratedTextActorPort> {
    /// Each completed new source-reference/finish intent uses the original fixed
    /// decoder and registered-file intake. Other peers cannot bypass either by
    /// exhausting the shared budget or retaining a partial frame. Pool scheduling
    /// does not generate a token, substitute output bytes or grant publication.
    pub fn drive<S, F>(&mut self, driver: &mut FileSupervisedDriver, source: &mut S,
        mut clock: F, budget: PoolBudget) -> Result<PoolDriveReport, FileActorPeerDriveError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.drive_prepared(driver, budget,
            |supervisor, port| supervisor.check_source_wire(&port.port),
            |supervisor, request, proposal, intake| {
                supervisor.prepare_generated_submission(request, proposal, source, &mut clock, intake)
            })
    }

    /// During review/approval or terminal grace, retain polling, exact retries
    /// and cancellation WITHOUT source reads or clock observations. New requests
    /// are withheld before any waiting admission snapshot could be consumed.
    /// Malformed native references still fail the original fixed decoder first.
    pub fn observe(&mut self, driver: &mut FileSupervisedDriver, budget: PoolBudget)
        -> Result<PoolDriveReport, FileActorPeerDriveError>
    {
        self.drive_prepared(driver, budget,
            |supervisor, port| supervisor.check_source_wire(&port.port),
            |supervisor, request, proposal, _| {
                decode(request, proposal)?;
                require_recorded(supervisor, request)
            })
    }

    pub fn next_request(&mut self, driver: &FileSupervisedDriver) -> Result<Option<PoolReady>, JournalError> {
        self.next_checked(driver, |supervisor, port| supervisor.check_source_wire(&port.port))
    }
}

#[cfg(test)]
mod tests;
