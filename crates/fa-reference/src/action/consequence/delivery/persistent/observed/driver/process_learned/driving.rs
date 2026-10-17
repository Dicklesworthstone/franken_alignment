//! Current original learned provenance and policy around external helper work.
//! Process replies pass only through the existing source-bound socket protocol.
use super::{FileProcessDriverProgress, FileProcessSupervisedDriver, FileProcessFailure,
    LearnedProcessStatus, unfinished};
use super::super::{FileDriverEvent, FileHumanPermit, FileHumanRequest, FileCredentialPermit,
    JournalError, Phase, observe, sample, stage};
use super::super::evidence::FileEvidenceReport;
use super::super::learned::evidence::{current_provider, policy_file_provider};
use super::super::provider::EvidenceProvider;
use crate::action::{ActionState, ElapsedTick};
use crate::action::consequence::oversight::evidence_source::EvidenceFile;
use crate::action::consequence::oversight::helper_processes::ProcessFailure;
use crate::action::consequence::oversight::helper_workers::io::WorkerIoError;
use crate::{Error, Snapshot};

/// Original supervisor progress; no event is a publication key or an actor
/// success reply. Socket/process failure details remain in the retained review.
#[derive(Debug)]
pub enum FileProcessDriverEvent {
    Review(FileProcessDriverProgress),
    Driver(FileDriverEvent),
}

impl FileProcessSupervisedDriver {
    /// One original process-review quantum or one original effect transition.
    /// Every source capture resolves the current acknowledged learned input.
    /// A completed review yields before either publication key is requested.
    pub fn step<F, P>(&mut self, clock: F, snapshot: P, human: Option<&FileHumanPermit>)
        -> Result<FileProcessDriverEvent, JournalError>
    where F: FnMut() -> ElapsedTick, P: FnMut() -> Result<Snapshot, Error> {
        self.step_current(clock, snapshot, human, None)
    }

    pub fn step_with_credential<F, P>(&mut self, clock: F, snapshot: P,
        human: Option<&FileHumanPermit>, credential: &FileCredentialPermit)
        -> Result<FileProcessDriverEvent, JournalError>
    where F: FnMut() -> ElapsedTick, P: FnMut() -> Result<Snapshot, Error> {
        self.step_current(clock, snapshot, human, Some(credential))
    }

    /// The original complete policy-only file contract: exact source scope,
    /// complete roster and empty helper contexts. The trusted snapshot cannot
    /// replace current learned evidence or submit any external helper's vote.
    pub fn step_from_policy_file<S, F>(&mut self, source: &mut S, clock: F,
        human: Option<&FileHumanPermit>) -> FileEvidenceReport<FileProcessDriverEvent>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.step_policy_file(source, clock, human, None)
    }

    pub fn step_from_policy_file_with_credential<S, F>(&mut self, source: &mut S,
        clock: F, human: Option<&FileHumanPermit>, credential: &FileCredentialPermit)
        -> FileEvidenceReport<FileProcessDriverEvent>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.step_policy_file(source, clock, human, Some(credential))
    }

    pub fn request_human_approval(&mut self, key: u64, expires_at: ElapsedTick,
        now: ElapsedTick) -> Result<FileHumanRequest, JournalError>
    {
        if unfinished(self.review.status()) { return Err(Error::WrongState.into()); }
        self.driver.request_learned_human_approval(key, expires_at, now)
    }

    pub fn request_human_approval_from_policy_file<S, F>(&mut self, source: &mut S,
        key: u64, expires_at: ElapsedTick, clock: F) -> FileEvidenceReport<FileHumanRequest>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        if unfinished(self.review.status()) {
            return FileEvidenceReport { observations: Vec::new(), source_updates: Vec::new(),
                result: Err(Error::WrongState.into()) };
        }
        self.driver.request_learned_human_approval_from_policy_file(source, key, expires_at, clock)
    }

    /// An original protocol cutoff, not permission to block until that time.
    /// External replies and child exits require intermediate nonblocking polls.
    pub fn next_review_deadline(&self) -> Option<ElapsedTick> {
        if !unfinished(self.review.status()) { return None; }
        let index = self.rounds.iter().position(|round| round.round == self.review.round())?;
        if self.review.status() == LearnedProcessStatus::Draining {
            return self.rounds.get(index + 1).map(|round| round.window.commit_by);
        }
        let window = self.rounds[index].window;
        let now = self.supervisor().host().ok()?.inspect().control.ledger.elapsed?;
        let awaiting_commit = self.review.records().get(&self.review.round())?
            .socket.workers.values().any(|status| !status.committed);
        Some(if now < window.commit_by && awaiting_commit { window.commit_by } else { window.reveal_by })
    }

    fn step_current<F, P>(&mut self, clock: F, snapshot: P, human: Option<&FileHumanPermit>,
        credential: Option<&FileCredentialPermit>) -> Result<FileProcessDriverEvent, JournalError>
    where F: FnMut() -> ElapsedTick, P: FnMut() -> Result<Snapshot, Error> {
        let expected = self.driver.job.as_ref().map(|job| (job.attempt, job.input_revision));
        self.step_with_provider(clock, &mut current_provider(expected, snapshot), human, credential)
    }

    fn step_policy_file<S, F>(&mut self, source: &mut S, clock: F,
        human: Option<&FileHumanPermit>, credential: Option<&FileCredentialPermit>)
        -> FileEvidenceReport<FileProcessDriverEvent>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let mut observations = Vec::with_capacity(2);
        let mut source_updates = Vec::with_capacity(2);
        let expected = self.driver.job.as_ref().map(|job| (job.attempt, job.input_revision));
        let result = self.step_with_provider(clock,
            &mut policy_file_provider(expected, source, &mut observations, &mut source_updates), human, credential);
        FileEvidenceReport { observations, source_updates, result }
    }

    fn step_with_provider<F, P>(&mut self, mut clock: F, provider: &mut P,
        human: Option<&FileHumanPermit>, credential: Option<&FileCredentialPermit>)
        -> Result<FileProcessDriverEvent, JournalError>
    where F: FnMut() -> ElapsedTick, P: EvidenceProvider {
        self.reap_helpers();
        if !unfinished(self.review.status()) {
            if let Some(job) = &mut self.driver.job {
                if job.phase == Phase::Review { job.close(); }
            }
            if self.driver.job.as_ref().is_some_and(|job| job.phase == Phase::Closed) {
                self.driver.job = None;
            }
            return self.driver.step_with_provider(clock, provider, human, credential)
                .map(FileProcessDriverEvent::Driver);
        }
        let mut pending = ProcessObservation { owner: self, acknowledged: false };
        let result = pending.owner.advance_with_provider(&mut clock, provider);
        pending.acknowledged = result.is_ok();
        result.map(FileProcessDriverEvent::Review)
    }

    fn advance_with_provider<F, P>(&mut self, clock: &mut F, provider: &mut P)
        -> Result<FileProcessDriverProgress, JournalError>
    where F: FnMut() -> ElapsedTick, P: EvidenceProvider {
        let revision = self.review.revision();
        let live = {
            let host = self.supervisor().host()?;
            let job = self.driver.job.as_ref().ok_or(Error::Missing)?;
            job.check_owner(&host)?;
            host.storage_failure().is_none() && host.inspect().stop.is_none()
                && stage(&host, self.request)? == ActionState::Reviewing
        };
        if !live {
            // Stop/cancellation/storage retirement invokes neither unneeded
            // source nor clock callbacks and preserves unknown-effect custody.
            return self.advance(revision, ElapsedTick(0), Snapshot::default()).map_err(journal_failure);
        }
        let snapshot = {
            let job = self.driver.job.as_ref().ok_or(Error::Missing)?;
            let mut host = self.driver.supervisor.host_mut()?;
            observe(&mut host, clock())?;
            let captured = sample(&mut host, job, provider, clock)?;
            if let Some(error) = captured.failure { return Err(error.into()); }
            if captured.evidence.inputs.as_ref() != job.inputs.as_ref() { return Err(Error::Stale.into()); }
            observe(&mut host, clock())?;
            captured.evidence.snapshot
        };
        // Pass the LIVE clock into the original process sequence. A post-I/O
        // reply cannot acquire a timestamp from before a slow policy read,
        // journal barrier, launch or preceding member's socket operation.
        self.advance_with_clock(revision, clock, snapshot).map_err(journal_failure)
    }
}

fn journal_failure(error: FileProcessFailure) -> JournalError {
    match error {
        FileProcessFailure::Journal(error) => error,
        FileProcessFailure::Transport(WorkerIoError::Protocol(error))
        | FileProcessFailure::Launch { failure: ProcessFailure::Refused(error), .. } => error.into(),
        _ => Error::Incomplete.into(),
    }
}

/// Refused policy/clock observations and caught unwinds terminate automatic
/// review custody. The original process owner retains observed exits and any
/// unresolved startup, so absence of a child owner is never reported as reaping.
struct ProcessObservation<'a> {
    owner: &'a mut FileProcessSupervisedDriver,
    acknowledged: bool,
}
impl Drop for ProcessObservation<'_> {
    fn drop(&mut self) {
        if self.acknowledged { return; }
        if unfinished(self.owner.review.status()) {
            let _ = self.owner.review.cancel(self.owner.review.revision());
        }
        if let Some(job) = &mut self.owner.driver.job {
            if job.phase != Phase::Reconcile { job.close(); }
        }
        self.owner.reap_helpers();
    }
}
