//! Original source and selected-request laws on the authenticated Tower peer.

use super::super::{DriveBudget, DriveReport, FileActorPeerDrive, FileActorPeerDriveError};
use super::{
    ActorError, ElapsedTick, Error, EvidenceFile, FileActorSupervisor, FileOversight,
    FileSupervisedDriver, JournalError,
};
use crate::tower::publication::peer::PublicationPeerSession;

impl FileActorSupervisor<FileOversight> {
    /// The authenticated session must contain this exact original owner. Only
    /// complete new proposals for the independently selected request acquire
    /// source/time; framing, replies and ticket capacity precede that handoff.
    pub fn drive_tower_request_from_file<S, F>(
        &mut self,
        session: &mut PublicationPeerSession,
        request: u64,
        source: &mut S,
        mut clock: F,
        budget: DriveBudget,
    ) -> Result<FileActorPeerDrive, FileActorPeerDriveError>
    where
        S: EvidenceFile + ?Sized,
        F: FnMut() -> ElapsedTick,
    {
        self.check_selected_tower_peer(session, request)?;
        let mut intakes = Vec::new();
        let drive = session.drive_with_admission(budget, |_, selected, _| {
            if selected != request {
                return Err(ActorError::Withheld);
            }
            let mut intake = None;
            let result = self.prepare_wire_submission(selected, source, &mut clock, &mut intake);
            if let Some(report) = intake {
                intakes.push(report);
            }
            result
        })?;
        Ok(FileActorPeerDrive { drive, intakes })
    }

    /// Retain the same Tower tickets during review, recovery and reconnection,
    /// but admit no new work even if a snapshot was separately installed. Reads
    /// retain their original Tower/disclosure authorization and query charges.
    pub fn drive_tower_request_observe(
        &mut self,
        session: &mut PublicationPeerSession,
        request: u64,
        budget: DriveBudget,
    ) -> Result<DriveReport, FileActorPeerDriveError> {
        self.check_selected_tower_peer(session, request)?;
        Ok(session.drive_with_admission(budget, |_, selected, _| {
            if selected != request {
                return Err(ActorError::Withheld);
            }
            match self.host().and_then(|host| host.request_status(selected)) {
                Ok(_) => Ok(()),
                Err(_) => Err(ActorError::Unavailable),
            }
        })?)
    }

    fn check_selected_tower_peer(
        &self,
        session: &PublicationPeerSession,
        request: u64,
    ) -> Result<(), JournalError> {
        if request == 0 {
            return Err(Error::InvalidInput.into());
        }
        self.check_source_wire(session.request_port())
    }
}

impl FileSupervisedDriver {
    pub fn drive_tower_request_from_file<S, F>(
        &mut self,
        session: &mut PublicationPeerSession,
        request: u64,
        source: &mut S,
        clock: F,
        budget: DriveBudget,
    ) -> Result<FileActorPeerDrive, FileActorPeerDriveError>
    where
        S: EvidenceFile + ?Sized,
        F: FnMut() -> ElapsedTick,
    {
        let result = self
            .supervisor_mut()
            .drive_tower_request_from_file(session, request, source, clock, budget);
        self.reap_helpers();
        result
    }

    pub fn drive_tower_request_observe(
        &mut self,
        session: &mut PublicationPeerSession,
        request: u64,
        budget: DriveBudget,
    ) -> Result<DriveReport, FileActorPeerDriveError> {
        let result = self
            .supervisor_mut()
            .drive_tower_request_observe(session, request, budget);
        self.reap_helpers();
        result
    }
}
