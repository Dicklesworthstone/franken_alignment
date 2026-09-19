//! Source-acquired completion for the ORIGINAL supervised request/job owner.
//! Reuse native worker review and automatic authorization, then the host's
//! two-read atomic effect/accounting cut. No duplicated effect state machine.
use super::PublicationProvider;
use super::feed::FeedProvider;
use super::super::super::super::publication::capture::heartbeat::feed::{PublicationFeedFile, PublicationFeedReport};
use super::super::super::super::publication::capture::completion::feed::FeedCompletionReport;
use super::super::super::{CommitteeContract, DriverEvidence, ElapsedTick, Error,
    FileCredentialPermit, FileHumanPermit, FileSupervisedDriver, FrozenAction,
    JournalError, Phase, observe, sample, stage};
use super::super::super::provider::Callback;
use super::super::super::super::publication::capture::{FileCaptureError, FileCaptureIdentity, PublicationInputFile};
use super::super::super::super::publication::capture::completion::{CapturedCompletionKeys, CapturedCompletionReport};
use crate::action::ActionState;
use std::rc::Rc;

/// Separate acquisition costs: at most one authorization read, then the host's
/// at most two completion reads. The original human key is never synthesized.
/// Only completion.result=Ok acknowledges a completed, settled effect.
#[derive(Debug)]
pub struct FileSourceCompletionReport {
    pub authorization_reads: Vec<Result<FileCaptureIdentity, FileCaptureError>>,
    pub completion: CapturedCompletionReport,
}

/// Authorization reports are independently acknowledged before a permit exists.
/// Completion feed reports appear only with the host's final acknowledged cut.
#[derive(Debug)]
pub struct FileFeedCompletionReport {
    pub authorization_feeds: Vec<Result<PublicationFeedReport, FileCaptureError>>,
    pub authorization_reads: Vec<Result<FileCaptureIdentity, FileCaptureError>>,
    pub completion: FeedCompletionReport,
}

impl FileSupervisedDriver {
    /// Complete this already reviewed job with an independently supplied human
    /// key. If no automatic key is retained, the ORIGINAL capture/sample/authorize
    /// path obtains it once. Native source-bound dispatch and publication then
    /// receive two independent acquisitions inside the host's single effect cut.
    ///
    /// Success closes the job: no extra publish or reconciliation step is needed.
    /// A returned pre-read failure with a healthy, still-authorized owner keeps
    /// the same reservation retryable. A caught unwind after entering completion
    /// retires the driver send path; quarantine/ambiguous replacement requires
    /// original recovery, never another attempt or automatic permission grant.
    ///
    /// This callback supplies committee/policy observations, not a native source
    /// lease renewal. Stronger configured source checks remain mandatory. The
    /// existing paired-file step API retains its separate transition semantics.
    pub fn complete_with_publication_source<F, P>(&mut self, source: &PublicationInputFile,
        clock: F, provider: P, human: &FileHumanPermit,
        credential: Option<&FileCredentialPermit>) -> FileSourceCompletionReport
    where F: FnMut() -> ElapsedTick,
        P: FnMut(&FrozenAction, &CommitteeContract) -> Result<DriverEvidence, Error>,
    {
        let report = self.complete_source_job(source, None, clock, provider, human, credential);
        FileSourceCompletionReport { authorization_reads: report.authorization_reads,
            completion: report.completion.completion }
    }

    /// Complete a reviewed job with feed catch-up before every effect boundary.
    /// Reuse the native authorization provider, then the host's two-acquisition
    /// atomic completion. Existing unspent permits are retained on healthy
    /// pre-read refusal; success closes the already-settled job without another
    /// publication/sweep turn. This callback does not renew native policy leases.
    pub fn complete_with_publication_feed<F, P>(&mut self, source: &PublicationInputFile,
        feed: &PublicationFeedFile, clock: F, provider: P, human: &FileHumanPermit,
        credential: Option<&FileCredentialPermit>) -> FileFeedCompletionReport
    where F: FnMut() -> ElapsedTick,
        P: FnMut(&FrozenAction, &CommitteeContract) -> Result<DriverEvidence, Error>,
    {
        self.complete_source_job(source, Some(feed), clock, provider, human, credential)
    }

    fn complete_source_job<F, P>(&mut self, source: &PublicationInputFile,
        feed: Option<&PublicationFeedFile>, mut clock: F, mut provider: P,
        human: &FileHumanPermit, credential: Option<&FileCredentialPermit>) -> FileFeedCompletionReport
    where F: FnMut() -> ElapsedTick,
        P: FnMut(&FrozenAction, &CommitteeContract) -> Result<DriverEvidence, Error>,
    {
        self.reap_helpers();
        let mut authorization_feeds = Vec::with_capacity(usize::from(feed.is_some()));
        let mut feed_reads = Vec::new();
        let mut committed = Vec::new();
        let mut authorization_reads = Vec::with_capacity(1);
        let mut reads = Vec::with_capacity(2);
        let mut evidence_failure = None;
        let result = (|| {
            let mut host = self.supervisor.host_mut()?;
            if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
            let job = self.job.as_mut().ok_or(Error::Missing)?;
            job.check_owner(&host)?;
            if job.phase != Phase::Ready || !matches!(stage(&host, job.request)?,
                ActionState::Reviewing | ActionState::Authorized) { return Err(Error::WrongState.into()); }
            job.check_ready(&host, job.inputs.as_ref())?;
            if !Rc::ptr_eq(&host.issuer, &human.issuer) || human.attempt != job.attempt {
                return Err(Error::Binding.into());
            }
            let bound = host.publication_source(job.attempt)?.ok_or(Error::Incomplete)?;
            if bound.source != source.source() { return Err(Error::Binding.into()); }
            match credential {
                Some(permit) => host.check_credential_permit(permit)?,
                None if host.credential_policy().is_some() => return Err(Error::Incomplete.into()),
                None => {}
            }
            if let Some(feed) = feed {
                host.publication_change_freshness()?;
                if host.publication_change_status()?.source != feed.source() { return Err(Error::Binding.into()); }
            }
            if job.permit.is_none() {
                let mut capture = PublicationProvider {
                    inner: Callback(&mut provider), attempt: Some(job.attempt), source,
                    reads: &mut authorization_reads,
                };
                let captured = match feed {
                    Some(feed) => sample(&mut host, job, &mut FeedProvider {
                        inner: capture, source: feed, reports: &mut authorization_feeds,
                    }, &mut clock)?,
                    None => sample(&mut host, job, &mut capture, &mut clock)?,
                };
                observe(&mut host, clock())?;
                if let Some(error) = captured.failure {
                    evidence_failure = Some(error);
                    return Err(error.into());
                }
                job.check_ready(&host, captured.evidence.inputs.as_ref())?;
                let revision = host.revision();
                job.permit = Some(host.authorize(revision, job.attempt,
                    captured.evidence.inputs.as_ref().ok_or(Error::Incomplete)?, captured.evidence.snapshot)?);
            }
            // Retire BEFORE external completion code. Even a caller-caught panic
            // cannot leave this job poised to issue another dispatch automatically.
            job.phase = Phase::Reconcile;
            let revision = host.revision();
            let keys = CapturedCompletionKeys {
                automatic: job.permit.as_ref().ok_or(Error::Incomplete)?, human, credential,
            };
            let completed = match feed {
                Some(feed) => {
                    let report = host.complete_publication_from_feed(revision, keys, source, feed, &mut clock, &mut provider);
                    feed_reads = report.reads;
                    committed = report.committed;
                    report.completion
                }
                None => host.complete_publication_from_source(revision, keys, source, &mut clock, &mut provider),
            };
            reads = completed.reads;
            evidence_failure = completed.evidence_failure;
            if completed.result.is_ok() {
                job.close();
            } else if host.storage_failure().is_none()
                && host.inspect().control.ledger.stages.get(&job.attempt) == Some(&ActionState::Authorized) {
                // Original authority proves the keys remain unspent. Keep that
                // SAME permit; the next call still performs two new acquisitions.
                job.phase = Phase::Ready;
            } else {
                job.permit = None;
            }
            completed.result
        })();
        self.reap_helpers();
        if self.job.as_ref().is_some_and(|job| job.phase == Phase::Closed) { self.job = None; }
        FileFeedCompletionReport { authorization_feeds, authorization_reads,
            completion: FeedCompletionReport { reads: feed_reads, committed,
                completion: CapturedCompletionReport { reads, evidence_failure, result } } }
    }
}
