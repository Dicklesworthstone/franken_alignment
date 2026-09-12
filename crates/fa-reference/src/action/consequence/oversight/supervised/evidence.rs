//! Fresh observation-provider reads at the ORIGINAL driver's review and dispatch
//! boundaries. Source failures invalidate input eligibility, not effect history.

use super::{DriverError, DriverEvent, ProcessReviewError, ProcessReviewLaunch, ReviewLaunch, SupervisedDriver};
use crate::action::consequence::oversight::evidence_source::{EvidenceError, EvidenceIdentity, EvidenceSnapshot, EvidenceFile};
use crate::action::consequence::oversight::helper_processes::HelperProgram;
use crate::action::consequence::oversight::helper_workers::HelperLimits;
use crate::action::consequence::oversight::human::HumanPermit;
use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput, ReviewWindow};
use crate::action::{ElapsedTick, FrozenAction};
use crate::{Error, Snapshot};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::os::unix::net::UnixStream;
use std::rc::Rc;

/// Observations from one trusted provider call, not approval. Neither the provider
/// interface nor a complete flag proves authenticity or a complete external world.
#[derive(Clone, Debug)]
pub struct DriverEvidence {
    pub snapshot: Snapshot,
    pub inputs: Option<CommitteeInput>,
}

pub(super) enum EvidenceFeed<'a> {
    Fixed { snapshot: &'a Snapshot, current: Option<&'a CommitteeInput> },
    Refresh(&'a mut dyn FnMut(&FrozenAction, &CommitteeContract) -> Result<DriverEvidence, Error>),
}

pub(super) struct EvidenceSample<'a> {
    pub snapshot: Cow<'a, Snapshot>,
    pub current: Option<Cow<'a, CommitteeInput>>,
    pub failure: Option<Error>,
}

impl<'a> EvidenceFeed<'a> {
    pub fn dynamic(&self) -> bool { matches!(self, Self::Refresh(_)) }
    fn read(&mut self, action: &FrozenAction, contracts: &CommitteeContract) -> Result<EvidenceSample<'a>, Error> {
        match self {
            Self::Fixed { snapshot, current } => Ok(EvidenceSample {
                snapshot: Cow::Borrowed(*snapshot), current: (*current).map(Cow::Borrowed), failure: None,
            }),
            Self::Refresh(provider) => {
                let observed = provider(action, contracts)?;
                if !observed.snapshot.complete { return Err(Error::Incomplete); }
                observed.inputs.as_ref().ok_or(Error::Incomplete)?.validate_for(action, contracts)?;
                Ok(EvidenceSample { snapshot: Cow::Owned(observed.snapshot),
                    current: observed.inputs.map(Cow::Owned), failure: None })
            }
        }
    }
}

/// The host chooses the same launch fields as before; actual inputs and the
/// reference root now come from a fresh, scoped file observation.
pub struct FileReviewLaunch<T> {
    pub request: u64,
    pub round: u64,
    pub window: ReviewWindow,
    pub expected_input_revision: u64,
    pub workers: BTreeMap<String, T>,
    pub limits: HelperLimits,
}

#[derive(Debug)]
pub enum FileReviewError {
    Source(EvidenceError),
    Control(Error),
    Connected(DriverError),
    Processes(ProcessReviewError),
}
impl From<Error> for FileReviewError { fn from(error: Error) -> Self { Self::Control(error) } }

/// Supervisor-only source diagnostics accompany (not replace) the driver result.
/// A failed second read can follow a successful reservation; that reservation is
/// retained. There are at most two source reads in one driver step.
#[derive(Debug)]
pub struct FileDriverStep {
    pub observations: Vec<Result<EvidenceIdentity, EvidenceError>>,
    pub result: Result<DriverEvent, DriverError>,
}

impl SupervisedDriver {
    /// Unlike step_with_clock's explicitly supplied fixed snapshot, this calls
    /// the provider AFTER helper I/O and again AFTER reserving, immediately before
    /// the final dispatch checks. Each provider call gets a fresh subsequent
    /// controller-clock observation. The original ledger does all authorization.
    pub fn step_with_evidence<F, P>(
        &mut self, clock: F, mut provider: P, human: Option<&HumanPermit>,
    ) -> Result<DriverEvent, DriverError>
    where F: FnMut() -> ElapsedTick,
        P: FnMut(&FrozenAction, &CommitteeContract) -> Result<DriverEvidence, Error>,
    {
        self.step_with_feed(clock, EvidenceFeed::Refresh(&mut provider), human)
    }

    pub fn step_from_file<F, S: EvidenceFile + ?Sized>(
        &mut self, source: &mut S, clock: F, human: Option<&HumanPermit>,
    ) -> FileDriverStep
    where F: FnMut() -> ElapsedTick {
        let mut observations = Vec::with_capacity(2);
        let result = self.step_with_evidence(clock, |action, contracts| {
            let captured = source.read_evidence().and_then(|capture| {
                let inputs = capture.inputs_for(action, contracts).map_err(EvidenceError::from)?;
                if !capture.snapshot().complete { return Err(Error::Incomplete.into()); }
                Ok((capture, inputs))
            });
            match captured {
                Ok((capture, inputs)) => {
                    observations.push(Ok(capture.identity()));
                    Ok(DriverEvidence { snapshot: capture.snapshot().clone(), inputs: Some(inputs) })
                }
                Err(error) => {
                    observations.push(Err(error));
                    Err(match error { EvidenceError::Data(error) => error, EvidenceError::Io(_) => Error::Incomplete })
                }
            }
        }, human);
        FileDriverStep { observations, result }
    }

    pub fn start_file_review<S: EvidenceFile + ?Sized>(
        &mut self, source: &mut S, launch: FileReviewLaunch<UnixStream>,
    ) -> Result<(), FileReviewError> {
        let (capture, inputs) = self.file_review_inputs(source, launch.request, launch.expected_input_revision)?;
        self.start_review(ReviewLaunch { request: launch.request, round: launch.round,
            evidence_root: capture.reference_root(), window: launch.window,
            expected_input_revision: launch.expected_input_revision, inputs,
            streams: launch.workers, limits: launch.limits }, capture.snapshot()).map_err(FileReviewError::Connected)
    }

    pub fn start_file_process_review<S: EvidenceFile + ?Sized>(
        &mut self, source: &mut S, launch: FileReviewLaunch<HelperProgram>,
    ) -> Result<(), FileReviewError> {
        let (capture, inputs) = self.file_review_inputs(source, launch.request, launch.expected_input_revision)?;
        self.start_process_review(ProcessReviewLaunch { request: launch.request, round: launch.round,
            evidence_root: capture.reference_root(), window: launch.window,
            expected_input_revision: launch.expected_input_revision, inputs,
            programs: launch.workers, limits: launch.limits }, capture.snapshot()).map_err(FileReviewError::Processes)
    }

    fn file_review_inputs<S: EvidenceFile + ?Sized>(
        &mut self, source: &mut S, request: u64, expected: u64,
    ) -> Result<(Rc<EvidenceSnapshot>, CommitteeInput), FileReviewError> {
        if self.job.is_some() { return Err(Error::WrongState.into()); }
        let attempt = self.supervisor.attempt(request)?;
        if self.supervisor.broker().input_revision(attempt)? != expected { return Err(Error::Stale.into()); }
        let action = self.supervisor.action(request)?;
        let captured = source.read_evidence().and_then(|capture| {
            if !capture.snapshot().complete { return Err(Error::Incomplete.into()); }
            let inputs = capture.inputs_for(action, self.supervisor.broker().contracts()).map_err(EvidenceError::from)?;
            Ok((capture, inputs))
        });
        if captured.is_err() { self.supervisor.broker_mut().inputs_unavailable(attempt, expected)?; }
        captured.map_err(FileReviewError::Source)
    }

    pub(super) fn sample_evidence<'a>(
        &mut self, feed: &mut EvidenceFeed<'a>, request: u64,
    ) -> Result<EvidenceSample<'a>, DriverError> {
        let observed = feed.read(self.supervisor.action(request)?, self.supervisor.broker().contracts());
        match observed {
            Ok(observed) => {
                if feed.dynamic() && self.job.as_ref().is_some_and(|job| {
                    observed.current.as_deref() != Some(&job.inputs)
                }) {
                    let attempt = self.supervisor.attempt(request)?;
                    let revision = self.supervisor.broker().input_revision(attempt)?;
                    self.supervisor.broker_mut().inputs_unavailable(attempt, revision)?;
                }
                Ok(observed)
            }
            Err(error) => {
                let attempt = self.supervisor.attempt(request)?;
                let revision = self.supervisor.broker().input_revision(attempt)?;
                self.supervisor.broker_mut().inputs_unavailable(attempt, revision)?;
                // Restrictive completed reviews may still apply their frozen
                // evidence. The unavailable sample can never authorize dispatch.
                Ok(EvidenceSample { snapshot: Cow::Owned(Snapshot::default()), current: None, failure: Some(error) })
            }
        }
    }
}
