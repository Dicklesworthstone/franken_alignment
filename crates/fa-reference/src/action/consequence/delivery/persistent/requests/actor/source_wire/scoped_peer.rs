//! One operator-selected request through the original authenticated actor session.
//! Restrict intake, not the durable request's outcome or existing cancellation law.
use super::{ActorError, DriveBudget, DriveReport, ElapsedTick, Error, EvidenceFile,
    FileActorPeerDrive, FileActorPeerDriveError, FileActorSupervisor, FileOversight,
    FileSupervisedDriver, JournalError, PeerSession, Port};

impl FileActorSupervisor<FileOversight> {
    /// Admit only the independently selected request. Foreign Submit keys refuse
    /// BEFORE source/time acquisition or use of a pre-existing snapshot. Partial
    /// frames, poll/cancel and retries keep the original codec/backpressure rules.
    /// This does not start a review, allocate a permit or execute the request.
    pub fn drive_peer_request_from_file<S, F>(&mut self, session: &mut PeerSession<Port>,
        request: u64, source: &mut S, mut clock: F, budget: DriveBudget)
        -> Result<FileActorPeerDrive, FileActorPeerDriveError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.check_selected_peer(session, request)?;
        let mut intakes = Vec::new();
        let drive = session.drive_with_admission(budget, |_, selected, _| {
            if selected != request { return Err(ActorError::Withheld); }
            let mut intake = None;
            let result = self.prepare_wire_submission(selected, source, &mut clock, &mut intake);
            if let Some(report) = intake { intakes.push(report); }
            result
        })?;
        Ok(FileActorPeerDrive { drive, intakes })
    }

    /// During review and after completion, keep the SAME ticket session serving
    /// polls, cancellations and exact recorded retries, but never admit NEW work.
    /// No source or clock is accepted by this interface. Even a preinstalled
    /// admission snapshot cannot bypass the explicit recorded-request check.
    /// An exact retry still compares all original bytes in the durable owner.
    pub fn drive_peer_request_observe(&mut self, session: &mut PeerSession<Port>,
        request: u64, budget: DriveBudget) -> Result<DriveReport, FileActorPeerDriveError>
    {
        self.check_selected_peer(session, request)?;
        Ok(session.drive_with_admission(budget, |_, selected, _| {
            if selected != request { return Err(ActorError::Withheld); }
            match self.host().and_then(|host| host.request_status(selected)) {
                Ok(_) => Ok(()),
                Err(_) => Err(ActorError::Unavailable),
            }
        })?)
    }

    fn check_selected_peer(&self, session: &PeerSession<Port>, request: u64) -> Result<(), JournalError> {
        if request == 0 { return Err(Error::InvalidInput.into()); }
        self.check_source_wire(session.request_port())
    }
}

impl FileSupervisedDriver {
    pub fn drive_peer_request_from_file<S, F>(&mut self, session: &mut PeerSession<Port>,
        request: u64, source: &mut S, clock: F, budget: DriveBudget)
        -> Result<FileActorPeerDrive, FileActorPeerDriveError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let result = self.supervisor_mut().drive_peer_request_from_file(session, request, source, clock, budget);
        self.reap_helpers();
        result
    }

    pub fn drive_peer_request_observe(&mut self, session: &mut PeerSession<Port>,
        request: u64, budget: DriveBudget) -> Result<DriveReport, FileActorPeerDriveError>
    {
        let result = self.supervisor_mut().drive_peer_request_observe(session, request, budget);
        self.reap_helpers();
        result
    }
}

#[cfg(test)]
mod tests;
