//! Concrete file observations on the ORIGINAL durable review/dispatch path.
//! The sealed reader interface cannot be replaced by a caller's cached snapshot.
use super::{FileDriverEvent, FileDriverLaunch, FileDriverProcessError, FileHumanPermit,
    FileHumanRequest, FileSupervisedDriver, JournalError, Phase, admitted, observe, sample, stage};
use super::super::helpers::FileHelperSetupError;
use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput};
use crate::action::consequence::oversight::evidence_source::{EvidenceError, EvidenceFile,
    EvidenceIdentity, EvidenceSnapshot};
use crate::action::consequence::oversight::helper_processes::HelperProgram;
use crate::action::consequence::oversight::supervised::DriverEvidence;
pub use crate::action::consequence::oversight::supervised::FileReviewLaunch;
use crate::action::{ActionState, ElapsedTick, FrozenAction};
use crate::Error;
use std::os::unix::net::UnixStream;
use std::rc::Rc;

/// Supervisor-only observations accompany the actual driver result, including
/// a second capture failure AFTER reservation. No observation is an effect key.
/// Step reports contain at most two reads; human-request reports at most one.
#[derive(Debug)]
pub struct FileEvidenceReport<T> {
    pub observations: Vec<Result<EvidenceIdentity, EvidenceError>>,
    pub result: Result<T, JournalError>,
}

/// Source loss and a failed durable eligibility withdrawal are separate facts.
/// A None withdrawal error means the original unavailable event was acknowledged.
/// A later setup failure retains the successfully captured source identity.
#[derive(Debug)]
pub enum FileSourceReviewError {
    Control(JournalError),
    Source { error: EvidenceError, withdrawal: Option<JournalError> },
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
    /// Use one fresh file capture for the original helper packet and review root.
    /// Neither actor input nor a retained source.current() value can supply it.
    /// The original start method samples trusted time AFTER this file read.
    pub fn start_file_review<S, F>(&mut self, source: &mut S,
        launch: FileReviewLaunch<UnixStream>, clock: F) -> Result<EvidenceIdentity, FileSourceReviewError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let (capture, inputs) = self.file_review_inputs(source, &launch)?;
        let observation = capture.identity();
        self.start_review(FileDriverLaunch {
            request: launch.request, round: launch.round, evidence_root: capture.reference_root(),
            window: launch.window, expected_input_revision: launch.expected_input_revision,
            inputs, workers: launch.workers, limits: launch.limits,
        }, capture.snapshot().clone(), clock)
            .map_err(|error| FileSourceReviewError::Sockets { observation, error })?;
        Ok(observation)
    }

    /// Same capture contract, with original executable-helper admission, retained
    /// partial child ownership and the original fresh post-spawn deadline check.
    pub fn start_file_process_review<S, F>(&mut self, source: &mut S,
        launch: FileReviewLaunch<HelperProgram>, clock: F) -> Result<EvidenceIdentity, FileSourceReviewError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let (capture, inputs) = self.file_review_inputs(source, &launch)?;
        let observation = capture.identity();
        self.start_process_review(FileDriverLaunch {
            request: launch.request, round: launch.round, evidence_root: capture.reference_root(),
            window: launch.window, expected_input_revision: launch.expected_input_revision,
            inputs, workers: launch.workers, limits: launch.limits,
        }, capture.snapshot().clone(), clock)
            .map_err(|error| FileSourceReviewError::Processes { observation, error })?;
        Ok(observation)
    }

    fn file_review_inputs<S, W>(&mut self, source: &mut S, launch: &FileReviewLaunch<W>)
        -> Result<(Rc<EvidenceSnapshot>, CommitteeInput), FileSourceReviewError>
    where S: EvidenceFile + ?Sized {
        if self.job.is_some() { return Err(Error::WrongState.into()); }
        self.ensure_child_slot()?;
        let mut host = self.supervisor.host_mut()?;
        if host.storage_failure().is_some() { return Err(JournalError::Unavailable.into()); }
        let (attempt, state) = admitted(host.request_status(launch.request)?)?;
        if state != ActionState::Reviewing { return Err(Error::WrongState.into()); }
        if host.input_revision(attempt)? != launch.expected_input_revision { return Err(Error::Stale.into()); }
        if !launch.workers.keys().eq(host.profile.committee.members().keys()) { return Err(Error::Binding.into()); }
        // Keep the original owner exclusively borrowed across provider I/O.
        // Reentrant actor/supervisor calls cannot replace the admitted request.
        let captured = capture(source, host.request_action(launch.request)?, &host.profile.committee);
        match captured {
            Ok(captured) => Ok(captured),
            Err(error) => {
                let revision = host.revision();
                let withdrawal = host.inputs_unavailable(revision, attempt, launch.expected_input_revision).err();
                Err(FileSourceReviewError::Source { error, withdrawal })
            }
        }
    }

    /// Reopen the concrete file at every original evidence boundary. In guarded
    /// mode this includes first publication; reconciliation never reads a file.
    /// File identities/errors are diagnostic and do not override native outcomes.
    pub fn step_from_file<S, F>(&mut self, source: &mut S, clock: F,
        human: Option<&FileHumanPermit>) -> FileEvidenceReport<FileDriverEvent>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let mut observations = Vec::with_capacity(2);
        let result = self.step_with_evidence(clock,
            |action, contracts| observed(source, action, contracts, &mut observations), human);
        FileEvidenceReport { observations, result }
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
                &mut |action, contracts| observed(source, action, contracts, &mut observations))?;
            observe(&mut host, clock())?;
            if let Some(error) = captured.failure { return Err(error.into()); }
            job.check_ready(&host, captured.evidence.inputs.as_ref())?;
            let revision = host.revision();
            host.request_human_approval(revision, key, job.attempt,
                captured.evidence.inputs.as_ref().ok_or(Error::Incomplete)?, expires_at)
        })();
        self.reap_helpers();
        FileEvidenceReport { observations, result }
    }
}

fn capture<S: EvidenceFile + ?Sized>(source: &mut S, action: &FrozenAction,
    contracts: &CommitteeContract) -> Result<(Rc<EvidenceSnapshot>, CommitteeInput), EvidenceError>
{
    let capture = source.read_evidence()?;
    if !capture.snapshot().complete { return Err(Error::Incomplete.into()); }
    let inputs = capture.inputs_for(action, contracts)?;
    Ok((capture, inputs))
}
fn observed<S: EvidenceFile + ?Sized>(source: &mut S, action: &FrozenAction,
    contracts: &CommitteeContract, observations: &mut Vec<Result<EvidenceIdentity, EvidenceError>>)
    -> Result<DriverEvidence, Error>
{
    match capture(source, action, contracts) {
        Ok((capture, inputs)) => {
            observations.push(Ok(capture.identity()));
            Ok(DriverEvidence { snapshot: capture.snapshot().clone(), inputs: Some(inputs) })
        }
        Err(error) => {
            observations.push(Err(error));
            Err(match error { EvidenceError::Data(error) => error, EvidenceError::Io(_) => Error::Incomplete })
        }
    }
}
