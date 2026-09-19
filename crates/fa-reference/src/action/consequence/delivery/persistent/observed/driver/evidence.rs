//! Concrete file observations on the ORIGINAL durable review/dispatch path.
//! The sealed reader interface cannot be replaced by a caller's cached snapshot.
mod launch;
mod intake;
pub mod publication;
use super::{FileDriverEvent, FileDriverProcessError, FileHumanPermit,
    FileHumanRequest, FileOversight, FileSupervisedDriver, JournalError, Phase, observe, sample, stage};
use super::super::credential::FileCredentialPermit;
use super::super::helpers::FileHelperSetupError;
use super::super::source::FileSourceError;
use super::provider::EvidenceProvider;
use crate::action::consequence::oversight::CommitteeContract;
use crate::action::consequence::oversight::evidence_source::{EvidenceError, EvidenceFile,
    EvidenceIdentity, EvidenceSnapshot};
use crate::action::consequence::oversight::supervised::DriverEvidence;
pub use crate::action::consequence::oversight::supervised::FileReviewLaunch;
use crate::action::{ActionState, ElapsedTick, FrozenAction};
use crate::Error;
use std::rc::Rc;

/// Supervisor-only observations accompany the actual driver result, including
/// a second capture failure AFTER reservation. No observation is an effect key.
/// Step reports contain at most two reads; human-request reports at most one.
#[derive(Debug)]
pub struct FileEvidenceReport<T> {
    pub observations: Vec<Result<EvidenceIdentity, EvidenceError>>,
    /// Native durable source operations, empty for an unconfigured legacy host.
    /// Includes committed refusals and BOTH a read failure and failed withdrawal.
    /// A failed preflight can appear here without a file read. An outer journal
    /// failure supplies no fabricated identity in observations; inspect this field.
    pub source_updates: Vec<Result<EvidenceIdentity, FileSourceError>>,
    pub result: Result<T, JournalError>,
}

/// Source loss and a failed durable eligibility withdrawal are separate facts.
/// A None withdrawal error means the original unavailable event was acknowledged.
/// A later setup failure retains the successfully captured source identity.
#[derive(Debug)]
pub enum FileSourceReviewError {
    Control(JournalError),
    Source { error: EvidenceError, withdrawal: Option<JournalError> },
    /// Native observation refusal or failure to persist the source transition.
    /// A refused observation can already have advanced the durable producer floor.
    DurableSource(FileSourceError),
    Sockets { observation: EvidenceIdentity, error: FileHelperSetupError },
    Processes { observation: EvidenceIdentity, error: FileDriverProcessError },
}
impl From<JournalError> for FileSourceReviewError {
    fn from(error: JournalError) -> Self { Self::Control(error) }
}
impl From<Error> for FileSourceReviewError {
    fn from(error: Error) -> Self { Self::Control(error.into()) }
}

impl FileSupervisedDriver {
    /// Reopen the concrete file at every original evidence boundary. Configured
    /// sources also persist every observation through the SAME native gate.
    /// First publication rechecks it; reconciliation never reads or renews it.
    pub fn step_from_file<S, F>(&mut self, source: &mut S, clock: F,
        human: Option<&FileHumanPermit>) -> FileEvidenceReport<FileDriverEvent>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let mut observations = Vec::with_capacity(2);
        let mut source_updates = Vec::with_capacity(2);
        let result = self.step_with_provider(clock,
            &mut FileProvider { source, observations: &mut observations, updates: &mut source_updates }, human, None);
        FileEvidenceReport { observations, source_updates, result }
    }

    /// Same concrete file/source state machine with the process-local credential
    /// capability available only at first publication. Earlier source reads,
    /// review, human approval and dispatch are unchanged.
    pub fn step_from_file_with_credential<S, F>(&mut self, source: &mut S, clock: F,
        human: Option<&FileHumanPermit>, credential: &FileCredentialPermit)
        -> FileEvidenceReport<FileDriverEvent>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let mut observations = Vec::with_capacity(2);
        let mut source_updates = Vec::with_capacity(2);
        let result = self.step_with_provider(clock,
            &mut FileProvider { source, observations: &mut observations, updates: &mut source_updates },
            human, Some(credential));
        FileEvidenceReport { observations, source_updates, result }
    }

    /// Freeze the original independent human request only after rereading the
    /// source and checking this job's original input/control cut. This does NOT
    /// approve a request, reserve automatic rights, or contact a human endpoint.
    /// A changed/missing source withdraws eligibility through the same sample
    /// operation used at dispatch. The original context-deduplication rule stays.
    pub fn request_human_approval_from_file<S, F>(&mut self, source: &mut S, key: u64,
        expires_at: ElapsedTick, mut clock: F) -> FileEvidenceReport<FileHumanRequest>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.reap_helpers();
        let mut observations = Vec::with_capacity(1);
        let mut source_updates = Vec::with_capacity(1);
        let result = (|| {
            let job = self.job.as_ref().ok_or(Error::Missing)?;
            let mut host = self.supervisor.host_mut()?;
            job.check_owner(&host)?;
            if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
            if job.phase != Phase::Ready
                || !matches!(stage(&host, job.request)?, ActionState::Reviewing | ActionState::Authorized)
            { return Err(Error::WrongState.into()); }
            // Retained data is used only to reject a stale job before reading,
            // never as the fresh observation for the ensuing request.
            job.check_ready(&host, job.inputs.as_ref())?;
            observe(&mut host, clock())?;
            let captured = sample(&mut host, job,
                &mut FileProvider { source, observations: &mut observations, updates: &mut source_updates }, &mut clock)?;
            observe(&mut host, clock())?;
            if let Some(error) = captured.failure { return Err(error.into()); }
            job.check_ready(&host, captured.evidence.inputs.as_ref())?;
            let revision = host.revision();
            host.request_human_approval(revision, key, job.attempt,
                captured.evidence.inputs.as_ref().ok_or(Error::Incomplete)?, expires_at)
        })();
        self.reap_helpers();
        FileEvidenceReport { observations, source_updates, result }
    }
}

struct FileProvider<'a, S: ?Sized> {
    source: &'a mut S,
    observations: &'a mut Vec<Result<EvidenceIdentity, EvidenceError>>,
    updates: &'a mut Vec<Result<EvidenceIdentity, FileSourceError>>,
}
impl<S: EvidenceFile + ?Sized> EvidenceProvider for FileProvider<'_, S> {
    fn capture<F>(&mut self, host: &mut FileOversight, action: &FrozenAction, clock: &mut F)
        -> Result<Result<DriverEvidence, Error>, JournalError>
    where F: FnMut() -> ElapsedTick {
        if !host.file_source_required() {
            // Legacy file profiles preserve their original callback/clock path.
            return Ok(observed(self.source, action, &host.profile.committee, self.observations));
        }
        let result = durable_capture(host, self.source, clock);
        self.updates.push(result.as_ref().map(|capture| capture.identity()).map_err(Clone::clone));
        match result {
            Ok(capture) => Ok(record_capture(capture, action, &host.profile.committee, self.observations)),
            Err(FileSourceError::Refused(error)) => {
                self.observations.push(Err(EvidenceError::Data(error)));
                Ok(Err(error))
            }
            Err(FileSourceError::Read { error, withdrawal }) => {
                self.observations.push(Err(error));
                match withdrawal { Some(error) => Err(error), None => Ok(Err(evidence_error(error))) }
            }
            Err(FileSourceError::Journal(error)) => Err(error),
        }
    }
}

// Called only after the concrete host's mandatory-source selection. The same
// read-start convention applies to launch, human requests and driver steps.
fn durable_capture<S, F>(host: &mut FileOversight, source: &mut S, clock: &mut F)
    -> Result<Rc<EvidenceSnapshot>, FileSourceError>
where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
    let started = clock();
    host.refresh_file_source(host.revision(), source, started)
}
fn evidence_error(error: EvidenceError) -> Error {
    match error { EvidenceError::Data(error) => error, EvidenceError::Io(_) => Error::Incomplete }
}
fn record_capture(capture: Rc<EvidenceSnapshot>, action: &FrozenAction, contracts: &CommitteeContract,
    observations: &mut Vec<Result<EvidenceIdentity, EvidenceError>>) -> Result<DriverEvidence, Error>
{
    let result = (|| {
        if !capture.snapshot().complete { return Err(Error::Incomplete); }
        let inputs = capture.inputs_for(action, contracts)?;
        Ok(DriverEvidence { snapshot: capture.snapshot().clone(), inputs: Some(inputs) })
    })();
    observations.push(result.as_ref().map(|_| capture.identity()).map_err(|error| EvidenceError::Data(*error)));
    result
}
fn observed<S: EvidenceFile + ?Sized>(source: &mut S, action: &FrozenAction,
    contracts: &CommitteeContract, observations: &mut Vec<Result<EvidenceIdentity, EvidenceError>>)
    -> Result<DriverEvidence, Error>
{
    match source.read_evidence() {
        Ok(capture) => record_capture(capture, action, contracts, observations),
        Err(error) => { observations.push(Err(error)); Err(evidence_error(error)) }
    }
}
