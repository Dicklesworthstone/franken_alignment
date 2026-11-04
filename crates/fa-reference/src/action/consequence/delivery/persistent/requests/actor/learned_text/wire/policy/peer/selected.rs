//! One selected learned release on the original authenticated peer engine.
use super::{FileActorSupervisor, FileOversight, FileLearnedTextStreamActorPort,
    PeerSession, EvidenceFile, ElapsedTick, DriveBudget, FileActorPeerDrive,
    FileActorPeerDriveError, WireError, PolicyPort, prepare};
use crate::action::consequence::delivery::persistent::requests::actor::learned_intake::LearnedStreamAdmission;
use crate::action::consequence::oversight::{actor::ActorError, actor_transport::DriveReport};

impl FileActorSupervisor<FileOversight> {
    /// Restrict Submit to one independently selected key, release and maximum
    /// deadline BEFORE source preflight, clock, policy I/O or snapshot use.
    /// A valid new release still uses the original numerical source and policy
    /// gate. Recorded retries compare original identity without reacquisition.
    ///
    /// Use a dedicated fresh PeerSession; previously issued tickets retain their
    /// original Poll/Cancel custody. No actor frame can change this selection.
    /// The original bounded transport and redaction are not replaced here.
    pub fn drive_learned_text_stream_request_from_policy_file<S, F>(&mut self,
        session: &mut PeerSession<FileLearnedTextStreamActorPort>, selected: LearnedStreamAdmission,
        source: &mut S, mut clock: F, budget: DriveBudget)
        -> Result<FileActorPeerDrive, FileActorPeerDriveError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.check_source_wire(session.request_port().original_port())?;
        budget.validate()?;
        let mut intakes = Vec::new();
        intakes.try_reserve_exact(budget.frames).map_err(|_| WireError::Capacity)?;
        let drive = session.drive_with_admission(budget, |port, request, proposal| {
            selected.check(request, proposal)?;
            let mut intake = None;
            let result = prepare(self, port, request, proposal, source, &mut clock, &mut intake);
            if let Some(report) = intake { intakes.push(report); }
            result
        })?;
        Ok(FileActorPeerDrive { drive, intakes })
    }

    /// Keep the same dedicated peer alive during review and publication, but
    /// remove new admission entirely. This method accepts NO source or clock.
    /// An unused snapshot cannot admit a missing request. Exact recorded retries
    /// are checked against the original output binding, including its deadline.
    /// Poll/Cancel retain original ticket and unknown-liability semantics.
    pub fn drive_learned_text_stream_request_observe(&mut self,
        session: &mut PeerSession<FileLearnedTextStreamActorPort>, selected: LearnedStreamAdmission,
        budget: DriveBudget) -> Result<DriveReport, FileActorPeerDriveError>
    {
        self.check_source_wire(session.request_port().original_port())?;
        Ok(session.drive_with_admission(budget, |port, request, proposal| {
            selected.check(request, proposal)?;
            let host = self.host().map_err(|_| ActorError::Unavailable)?;
            host.request_status(request).map_err(|_| ActorError::Unavailable)?;
            port.preflight(&host, request, proposal)
        })?)
    }
}
