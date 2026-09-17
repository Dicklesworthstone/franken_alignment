//! Compose stream-intent validation with the ORIGINAL registered-source intake.
//! No actor-supplied source, clock, callback or alternative request ledger.
use super::{FileOversight, FileStreamActorPort, FileActorSupervisor, JournalError};
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::requests::actor::{FileActorExchange, FileActorFeed};
use crate::action::consequence::delivery::persistent::observed::driver::FileSupervisedDriver;
use crate::action::consequence::oversight::actor_wire::{ActorWire, ActorChannel};
use crate::action::consequence::oversight::evidence_source::EvidenceFile;
#[cfg(target_os = "linux")]
use crate::action::consequence::oversight::evidence_source::EvidenceIdentity;
#[cfg(target_os = "linux")]
use crate::action::consequence::delivery::persistent::observed::driver::evidence::FileEvidenceReport;
#[cfg(target_os = "linux")]
use crate::action::consequence::oversight::actor_peer::PeerSession;
#[cfg(target_os = "linux")]
use crate::action::consequence::oversight::actor_transport::{DriveBudget, DriveReport};
#[cfg(target_os = "linux")]
use crate::action::consequence::oversight::actor_wire::WireError;

/// Supervisor-only intake diagnostics in completed-frame order. Responses sent
/// to the actor retain only the original redacted knowledge/error projection.
#[cfg(target_os = "linux")]
#[derive(Debug)]
pub struct FileStreamPeerDrive {
    pub drive: DriveReport,
    pub intakes: Vec<FileEvidenceReport<EvidenceIdentity>>,
}
#[cfg(target_os = "linux")]
#[derive(Debug)]
pub enum FileStreamPeerDriveError { Journal(JournalError), Wire(WireError) }
#[cfg(target_os = "linux")]
impl From<JournalError> for FileStreamPeerDriveError {
    fn from(error: JournalError) -> Self { Self::Journal(error) }
}
#[cfg(target_os = "linux")]
impl From<WireError> for FileStreamPeerDriveError {
    fn from(error: WireError) -> Self { Self::Wire(error) }
}

impl FileActorSupervisor<FileOversight> {
    /// Validate the complete intent before acquiring source evidence. Only a NEW
    /// request uses the original one-use intake slot. Poll, cancel, exact and
    /// conflicting retries remain read-free. No source diagnostics reach the wire.
    pub fn exchange_stream_actor_from_file<S, F>(&mut self, wire: &mut ActorWire<FileStreamActorPort>,
        document: &[u8], source: &mut S, mut clock: F) -> Result<FileActorExchange, JournalError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.check_source_wire(&wire.request_port().port)?;
        let mut intake = None;
        let response = wire.exchange_with_admission(document, |port, request, proposal| {
            port.validate_proposal(proposal)?;
            self.prepare_wire_submission(request, source, &mut clock, &mut intake)
        });
        Ok(FileActorExchange { response, intake })
    }

    /// Share original newline framing and response write/flush backpressure.
    /// Incomplete input and a blocked response cannot pre-read the next source.
    pub fn feed_stream_actor_from_file<S, F>(&mut self, channel: &mut ActorChannel<FileStreamActorPort>,
        bytes: &[u8], source: &mut S, mut clock: F) -> Result<FileActorFeed, JournalError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.check_source_wire(&channel.request_port().port)?;
        let mut intake = None;
        let feed = channel.feed_with_admission(bytes, |port, request, proposal| {
            port.validate_proposal(proposal)?;
            self.prepare_wire_submission(request, source, &mut clock, &mut intake)
        });
        Ok(FileActorFeed { feed, intake })
    }

    /// The ORIGINAL kernel-peer gate must admit the socket before any frame or
    /// source can be read. Domain identity is checked before driving it. Socket
    /// budgets do not bound the separate synchronous source/journal operations.
    #[cfg(target_os = "linux")]
    pub fn drive_stream_peer_from_file<S, F>(&mut self, session: &mut PeerSession<FileStreamActorPort>,
        source: &mut S, mut clock: F, budget: DriveBudget) -> Result<FileStreamPeerDrive, FileStreamPeerDriveError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.check_source_wire(&session.request_port().port)?;
        let mut intakes = Vec::new();
        let drive = session.drive_with_admission(budget, |port, request, proposal| {
            port.validate_proposal(proposal)?;
            let mut intake = None;
            let result = self.prepare_wire_submission(request, source, &mut clock, &mut intake);
            if let Some(report) = intake { intakes.push(report); }
            result
        })?;
        Ok(FileStreamPeerDrive { drive, intakes })
    }
}

impl FileSupervisedDriver {
    pub fn exchange_stream_actor_from_file<S, F>(&mut self, wire: &mut ActorWire<FileStreamActorPort>,
        document: &[u8], source: &mut S, clock: F) -> Result<FileActorExchange, JournalError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let result = self.supervisor_mut().exchange_stream_actor_from_file(wire, document, source, clock);
        self.reap_helpers();
        result
    }
    pub fn feed_stream_actor_from_file<S, F>(&mut self, channel: &mut ActorChannel<FileStreamActorPort>,
        bytes: &[u8], source: &mut S, clock: F) -> Result<FileActorFeed, JournalError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let result = self.supervisor_mut().feed_stream_actor_from_file(channel, bytes, source, clock);
        self.reap_helpers();
        result
    }
    #[cfg(target_os = "linux")]
    pub fn drive_stream_peer_from_file<S, F>(&mut self, session: &mut PeerSession<FileStreamActorPort>,
        source: &mut S, clock: F, budget: DriveBudget) -> Result<FileStreamPeerDrive, FileStreamPeerDriveError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let result = self.supervisor_mut().drive_stream_peer_from_file(session, source, clock, budget);
        self.reap_helpers();
        result
    }
}
