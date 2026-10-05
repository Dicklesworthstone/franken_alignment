//! One native-model review loop through the ORIGINAL source and effect gates.
//! Numerical custody is retained across live policy reads and publication. No
//! caller supplies helper packets, a verdict, a permit or a replacement source.
use super::{FileNativeDriverProgress, FileNativeSupervisedDriver, NativeReviewStatus};
use super::super::{FileDriverEvent, FileHumanPermit, FileHumanRequest, FileCredentialPermit,
    JournalError, Phase, observe, sample, stage};
use super::super::evidence::FileEvidenceReport;
use super::super::learned::evidence::{current_provider, policy_snapshot};
use super::super::provider::EvidenceProvider;
use crate::action::{ActionState, ElapsedTick};
use crate::action::consequence::oversight::evidence_source::EvidenceFile;
use crate::{Error, Snapshot};

/// Supervisor-only progress. Original actor tickets keep their existing
/// Knowledge projection; this event cannot be used as either publication key.
#[derive(Debug)]
pub enum FileNativeDriverEvent {
    Review(FileNativeDriverProgress),
    Driver(FileDriverEvent),
}

impl FileNativeSupervisedDriver {
    /// One original review quantum OR one original effect transition. Resolve
    /// current learned input before and after each policy callback, including
    /// every refinement and both captures around automatic reservation. A
    /// completed native review yields before requesting or consuming either key.
    pub fn step<F, P>(&mut self, clock: F, snapshot: P, human: Option<&FileHumanPermit>)
        -> Result<FileNativeDriverEvent, JournalError>
    where F: FnMut() -> ElapsedTick, P: FnMut() -> Result<Snapshot, Error> {
        self.step_current(clock, snapshot, human, None)
    }

    /// The existing credential is consulted only by the original publication
    /// gate. Numerical work, human review and reservation do not consume it.
    pub fn step_with_credential<F, P>(&mut self, clock: F, snapshot: P,
        human: Option<&FileHumanPermit>, credential: &FileCredentialPermit)
        -> Result<FileNativeDriverEvent, JournalError>
    where F: FnMut() -> ElapsedTick, P: FnMut() -> Result<Snapshot, Error> {
        self.step_current(clock, snapshot, human, Some(credential))
    }

    /// Same original policy-only file contract as the computed-probe driver:
    /// exact scope and complete roster with EMPTY helper contexts. A live file
    /// observation is not a durable producer-floor acknowledgment. Source loss
    /// is checked before reading; query-only settlement never reads the file.
    pub fn step_from_policy_file<S, F>(&mut self, source: &mut S, clock: F,
        human: Option<&FileHumanPermit>) -> FileEvidenceReport<FileNativeDriverEvent>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.step_policy_file(source, clock, human, None)
    }

    pub fn step_from_policy_file_with_credential<S, F>(&mut self, source: &mut S,
        clock: F, human: Option<&FileHumanPermit>, credential: &FileCredentialPermit)
        -> FileEvidenceReport<FileNativeDriverEvent>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.step_policy_file(source, clock, human, Some(credential))
    }

    /// Request, never approve, the independent human key using the original
    /// CURRENT learned input. The separately held reviewer still supplies it.
    pub fn request_human_approval(&mut self, key: u64, expires_at: ElapsedTick,
        now: ElapsedTick) -> Result<FileHumanRequest, JournalError>
    {
        if self.review.status() == NativeReviewStatus::Running { return Err(Error::WrongState.into()); }
        self.driver.request_learned_human_approval(key, expires_at, now)
    }

    pub fn next_review_deadline(&self) -> Option<ElapsedTick> {
        if self.review.status() != NativeReviewStatus::Running { return None; }
        let now = self.supervisor().host().ok()?.inspect().control.ledger.elapsed?;
        let window = self.review.round().window;
        Some(if now < window.commit_by { window.commit_by } else { window.reveal_by })
    }

    fn step_current<F, P>(&mut self, clock: F, snapshot: P, human: Option<&FileHumanPermit>,
        credential: Option<&FileCredentialPermit>) -> Result<FileNativeDriverEvent, JournalError>
    where F: FnMut() -> ElapsedTick, P: FnMut() -> Result<Snapshot, Error> {
        let expected = self.driver.job.as_ref().map(|job| (job.attempt, job.input_revision));
        self.step_with_provider(clock, &mut current_provider(expected, snapshot), human, credential)
    }

    fn step_policy_file<S, F>(&mut self, source: &mut S, clock: F,
        human: Option<&FileHumanPermit>, credential: Option<&FileCredentialPermit>)
        -> FileEvidenceReport<FileNativeDriverEvent>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let mut observations = Vec::with_capacity(2);
        let result = (|| {
            let (scope, members, full_context_source) = {
                let host = self.supervisor().host()?;
                (host.profile.delivery.scope, host.profile.committee.members().keys().cloned().collect::<Vec<_>>(),
                    host.file_source_required())
            };
            let snapshot = || policy_snapshot(source, scope, &members, full_context_source, &mut observations);
            self.step_current(clock, snapshot, human, credential)
        })();
        FileEvidenceReport { observations, source_updates: Vec::new(), result }
    }

    fn step_with_provider<F, P>(&mut self, mut clock: F, provider: &mut P,
        human: Option<&FileHumanPermit>, credential: Option<&FileCredentialPermit>)
        -> Result<FileNativeDriverEvent, JournalError>
    where F: FnMut() -> ElapsedTick, P: EvidenceProvider {
        self.driver.reap_helpers();
        if self.review.status() != NativeReviewStatus::Running {
            // A caught original numerical unwind may leave a Reviewing job,
            // never a usable result. Preserve Ready and query-only obligations.
            if let Some(job) = &mut self.driver.job {
                if job.phase == Phase::Review { job.close(); }
            }
            if self.driver.job.as_ref().is_some_and(|job| job.phase == Phase::Closed) {
                self.driver.job = None;
            }
            return self.driver.step_with_provider(clock, provider, human, credential)
                .map(FileNativeDriverEvent::Driver);
        }
        let mut pending = NativeObservation { owner: self, acknowledged: false };
        let result = pending.owner.advance_with_provider(&mut clock, provider);
        pending.acknowledged = result.is_ok();
        result.map(FileNativeDriverEvent::Review)
    }

    fn advance_with_provider<F, P>(&mut self, clock: &mut F, provider: &mut P)
        -> Result<FileNativeDriverProgress, JournalError>
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
            // The original advance checks actual cancellation/stop/storage BEFORE
            // snapshot or time admission. Do not invoke either external callback
            // merely to retire numerical custody or preserve an unknown effect.
            return self.advance(revision, ElapsedTick(0), Snapshot::default());
        }
        let (now, snapshot) = {
            let job = self.driver.job.as_ref().ok_or(Error::Missing)?;
            let mut host = self.driver.supervisor.host_mut()?;
            observe(&mut host, clock())?;
            let captured = sample(&mut host, job, provider, clock)?;
            if let Some(error) = captured.failure { return Err(error.into()); }
            if captured.evidence.inputs.as_ref() != job.inputs.as_ref() { return Err(Error::Stale.into()); }
            let now = clock();
            observe(&mut host, now)?;
            (now, captured.evidence.snapshot)
        };
        // Only the actual privately owned native sequence evaluates its models,
        // queues its commitments/reveals, refines and applies the congress result.
        self.advance(revision, now, snapshot)
    }
}

/// A policy/clock refusal or caught unwind ends this automatic review attempt.
/// Original numerical work/future reservations remain inspectable; no request
/// cancellation, refund, permitting receipt or replacement native roster is made.
struct NativeObservation<'a> {
    owner: &'a mut FileNativeSupervisedDriver,
    acknowledged: bool,
}
impl Drop for NativeObservation<'_> {
    fn drop(&mut self) {
        if self.acknowledged { return; }
        if self.owner.review.status() == NativeReviewStatus::Running {
            let _ = self.owner.review.cancel(self.owner.review.revision());
        }
        if let Some(job) = &mut self.owner.driver.job {
            if job.phase != Phase::Reconcile { job.close(); }
        }
        self.owner.driver.reap_helpers();
    }
}
