//! Source-bound actual actor processes, using the same raw/stream request owners.
//! The original durable authority stops effects; child termination is separate.
use super::{FileActorPort, FileActorSupervisor, FileOversight, FileStreamActorPort, JournalError};
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::observed::driver::FileSupervisedDriver;
use crate::action::consequence::delivery::persistent::observed::driver::evidence::FileEvidenceReport;
use crate::action::consequence::oversight::actor::{ActorError, ActorProposal};
use crate::action::consequence::oversight::actor_process::{ActorProcess, ActorProcessStatus};
use crate::action::consequence::oversight::actor_transport::{DriveBudget, DriveReport};
use crate::action::consequence::oversight::actor_wire::{ActorRequestPort, WireError};
use crate::action::consequence::oversight::evidence_source::{EvidenceFile, EvidenceIdentity};

/// Why this integration withdrew process ingress. None of these is a terminal
/// effect outcome. OwnerUnavailable does not assert a committed native stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActorProcessWithdrawal { TerminalStop, OwnerUnavailable, InterruptedDrive }

/// Completed socket work, original source reports, and child status stay separate.
/// No transport drive is attempted on withdrawal or an already-closed ingress.
/// A source refusal is an actor-redacted response and remains in `intakes`.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::observed::stream::actor_wire::process::FileActorProcessDrive;
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// fn authorize(report: FileActorProcessDrive) -> FilePermit { report }
/// ```
#[derive(Debug)]
pub struct FileActorProcessDrive {
    pub transport: Result<Option<DriveReport>, WireError>,
    pub process: ActorProcessStatus,
    pub withdrawal: Option<ActorProcessWithdrawal>,
    pub intakes: Vec<FileEvidenceReport<EvidenceIdentity>>,
}
#[derive(Debug)]
pub enum FileActorProcessError { Journal(JournalError), Wire(WireError) }
impl From<JournalError> for FileActorProcessError {
    fn from(error: JournalError) -> Self { Self::Journal(error) }
}
impl From<WireError> for FileActorProcessError {
    fn from(error: WireError) -> Self { Self::Wire(error) }
}

impl FileActorSupervisor<FileOversight> {
    /// Only complete NEW raw submits acquire the original concrete file source.
    /// Verify the original gateway BEFORE inspecting or terminating a process.
    pub fn drive_actor_process_from_file<S, F>(&mut self,
        process: &mut ActorProcess<FileActorPort<FileOversight>>, source: &mut S,
        mut clock: F, budget: DriveBudget) -> Result<FileActorProcessDrive, FileActorProcessError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.check_source_wire(process.request_port())?;
        self.drive_owned_actor(process, budget, |host, _, request, _, reports| {
            let mut intake = None;
            let result = host.prepare_wire_submission(request, source, &mut clock, &mut intake);
            if let Some(report) = intake { reports.push(report); }
            result
        })
    }

    /// The original stream decoder runs before source/clock access. Fragments,
    /// poll/cancel and exact/conflicting retries keep their read-free semantics.
    pub fn drive_stream_process_from_file<S, F>(&mut self,
        process: &mut ActorProcess<FileStreamActorPort>, source: &mut S,
        mut clock: F, budget: DriveBudget) -> Result<FileActorProcessDrive, FileActorProcessError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.check_source_wire(&process.request_port().port)?;
        self.drive_owned_actor(process, budget, |host, port, request, proposal, reports| {
            port.validate_proposal(proposal)?;
            let mut intake = None;
            let result = host.prepare_wire_submission(request, source, &mut clock, &mut intake);
            if let Some(report) = intake { reports.push(report); }
            result
        })
    }

    fn process_withdrawal(&self) -> Option<ActorProcessWithdrawal> {
        match self.host() {
            Err(_) => Some(ActorProcessWithdrawal::OwnerUnavailable),
            Ok(host) if host.storage_failure().is_some() => Some(ActorProcessWithdrawal::OwnerUnavailable),
            Ok(host) if host.inspect().stop.is_some() => Some(ActorProcessWithdrawal::TerminalStop),
            Ok(_) => None,
        }
    }

    // Used only after each public entry point's exact gateway check. There is
    // no public callback, configurable backend or actor-selected evidence source.
    fn drive_owned_actor<P, A>(&mut self, process: &mut ActorProcess<P>, budget: DriveBudget,
        mut prepare: A) -> Result<FileActorProcessDrive, FileActorProcessError>
    where P: ActorRequestPort,
        A: FnMut(&mut Self, &P, u64, &ActorProposal, &mut Vec<FileEvidenceReport<EvidenceIdentity>>) -> Result<(), ActorError> {
        budget.validate()?;
        let withdrawal = if process.status().interrupted_drive {
            Some(ActorProcessWithdrawal::InterruptedDrive)
        } else { self.process_withdrawal() };
        if withdrawal.is_some() {
            return Ok(FileActorProcessDrive { transport: Ok(None), process: process.request_stop(),
                withdrawal, intakes: Vec::new() });
        }
        if process.status().ingress_closed {
            return Ok(FileActorProcessDrive { transport: Ok(None), process: process.poll(),
                withdrawal: None, intakes: Vec::new() });
        }
        let mut intakes = Vec::new();
        let transport = process.drive_with_admission(budget, |port, request, proposal| {
            prepare(self, port, request, proposal, &mut intakes)
        }).map(|result| Some(result.drive));
        // A source/journal failure may have made the owner unavailable DURING
        // this drive. Withdraw ingress in the same call, retaining actual work.
        let withdrawal = self.process_withdrawal();
        let status = if withdrawal.is_some() { process.request_stop() } else { process.status() };
        Ok(FileActorProcessDrive { transport, process: status, withdrawal, intakes })
    }
}

impl FileSupervisedDriver {
    /// Keep existing helper-child maintenance; do not infer effect disposition
    /// from actor status or bypass original source-aware review and publication.
    pub fn drive_actor_process_from_file<S, F>(&mut self,
        process: &mut ActorProcess<FileActorPort<FileOversight>>, source: &mut S,
        clock: F, budget: DriveBudget) -> Result<FileActorProcessDrive, FileActorProcessError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let result = self.supervisor_mut().drive_actor_process_from_file(process, source, clock, budget);
        self.reap_helpers();
        result
    }
    pub fn drive_stream_process_from_file<S, F>(&mut self,
        process: &mut ActorProcess<FileStreamActorPort>, source: &mut S,
        clock: F, budget: DriveBudget) -> Result<FileActorProcessDrive, FileActorProcessError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let result = self.supervisor_mut().drive_stream_process_from_file(process, source, clock, budget);
        self.reap_helpers();
        result
    }
}
