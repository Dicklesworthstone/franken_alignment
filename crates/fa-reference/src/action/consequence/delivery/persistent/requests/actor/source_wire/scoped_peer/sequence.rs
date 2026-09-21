//! Advance a bounded operator schedule without replacing the authenticated peer,
//! ticket table, authority owner or transport's lifetime counters.
use super::*;
use crate::action::ActionState;
use crate::action::consequence::delivery::persistent::requests::FileRequestDisposition;

impl FileSupervisedDriver {
    /// Maximum number of distinct keys in one explicitly selected schedule.
    /// This does not enlarge any journal, rights, helper or transport limit.
    pub const MAX_REQUEST_SEQUENCE: usize = 64;

    /// Only the current key may acquire fresh source evidence. Earlier keys must
    /// already be terminal in the ORIGINAL journal and remain exact-retry-only.
    pub fn drive_peer_sequence_from_file<S, F>(&mut self, session: &mut PeerSession<Port>,
        request: u64, completed: &[u64], source: &mut S, clock: F, budget: DriveBudget)
        -> Result<FileActorPeerDrive, FileActorPeerDriveError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let result = self.supervisor_mut().drive_peer_sequence_from_file(
            session, request, completed, source, clock, budget);
        self.reap_helpers();
        result
    }

    /// Read-free service during review, cleanup and final response delivery.
    pub fn drive_peer_sequence_observe(&mut self, session: &mut PeerSession<Port>,
        request: u64, completed: &[u64], budget: DriveBudget)
        -> Result<DriveReport, FileActorPeerDriveError>
    {
        let result = self.supervisor_mut().drive_peer_sequence_observe(session, request, completed, budget);
        self.reap_helpers();
        result
    }
}

impl FileActorSupervisor<FileOversight> {
    fn check_sequence(&self, session: &PeerSession<Port>, request: u64, completed: &[u64])
        -> Result<(), JournalError>
    {
        self.check_selected_peer(session, request)?;
        if completed.len() >= FileSupervisedDriver::MAX_REQUEST_SEQUENCE { return Err(Error::Limit.into()); }
        for (index, &previous) in completed.iter().enumerate() {
            if previous == 0 || previous == request || completed[..index].contains(&previous) {
                return Err(Error::InvalidInput.into());
            }
        }
        let host = self.host()?;
        if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
        for &previous in completed {
            let terminal = match host.request_status(previous)?.disposition {
                FileRequestDisposition::NotAdmitted(_) => true,
                FileRequestDisposition::Admitted { stage, .. } => matches!(stage,
                    ActionState::Cancelled | ActionState::Denied
                    | ActionState::Confirmed | ActionState::ConfirmedNotExecuted),
            };
            if !terminal { return Err(Error::WrongState.into()); }
        }
        Ok(())
    }

    /// The schedule is privileged routing DATA. It cannot provide an old outcome
    /// or change an existing request. Future/unlisted keys refuse before capture.
    pub fn drive_peer_sequence_from_file<S, F>(&mut self, session: &mut PeerSession<Port>,
        request: u64, completed: &[u64], source: &mut S, mut clock: F, budget: DriveBudget)
        -> Result<FileActorPeerDrive, FileActorPeerDriveError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.check_sequence(session, request, completed)?;
        let mut intakes = Vec::new();
        let drive = session.drive_with_admission(budget, |_, selected, _| {
            if selected != request {
                // The completed prefix was checked against the original journal
                // before I/O. Original submission still compares every byte.
                return if completed.contains(&selected) { Ok(()) } else { Err(ActorError::Withheld) };
            }
            let mut intake = None;
            let result = self.prepare_wire_submission(selected, source, &mut clock, &mut intake);
            if let Some(report) = intake { intakes.push(report); }
            result
        })?;
        Ok(FileActorPeerDrive { drive, intakes })
    }

    /// New current work refuses in this mode even with an old admission slot.
    /// Original poll/cancel ownership and response redaction remain unchanged.
    pub fn drive_peer_sequence_observe(&mut self, session: &mut PeerSession<Port>,
        request: u64, completed: &[u64], budget: DriveBudget)
        -> Result<DriveReport, FileActorPeerDriveError>
    {
        self.check_sequence(session, request, completed)?;
        Ok(session.drive_with_admission(budget, |_, selected, _| {
            if selected != request && !completed.contains(&selected) { return Err(ActorError::Withheld); }
            match self.host().and_then(|host| host.request_status(selected)) {
                Ok(_) => Ok(()),
                Err(_) => Err(ActorError::Unavailable),
            }
        })?)
    }
}
