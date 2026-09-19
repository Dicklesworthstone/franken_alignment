//! Ingest real change records before the original witness-acquisition cycle.
//! Catch-up can change input revisions; never install a witness using a token
//! captured before that catch-up. Feed/committee failures remain distinct.
use super::{Callback, CommitteeContract, DriverEvidence, ElapsedTick, Error,
    EvidenceFile, EvidenceProvider, FileCaptureError, FileCredentialPermit,
    FileDriverEvent, FileEvidenceReport, FileHumanPermit, FileOversight,
    FileProvider, FilePublicationDriverReport, FileSupervisedDriver, FrozenAction,
    JournalError, PublicationInputFile, PublicationProvider};
use super::super::super::super::publication::capture::heartbeat::feed::{
    PublicationFeedFile, PublicationFeedReport,
};

/// Actual acknowledged ingestion results, not heartbeat-only coverage claims.
/// At most two feed reads per step. No report is fabricated for failed journal
/// installation, and current authority is always checked after all acquisitions.
#[derive(Debug)]
pub struct FileFeedDriverReport<T> {
    pub feeds: Vec<Result<PublicationFeedReport, FileCaptureError>>,
    pub publication: FilePublicationDriverReport<T>,
}
impl FileSupervisedDriver {
    /// Acquire a complete change window and heartbeat, then capture the committee
    /// and concrete witness input at each original evidence boundary. The feed
    /// withdraws its global eligibility before I/O; the original witness provider
    /// withdraws its attempt before calling external committee code.
    pub fn step_with_publication_feed<F, P>(&mut self,
        source: &PublicationInputFile, feed: &PublicationFeedFile,
        clock: F, provider: P, human: Option<&FileHumanPermit>,
        credential: Option<&FileCredentialPermit>) -> FileFeedDriverReport<FileDriverEvent>
    where F: FnMut() -> ElapsedTick,
        P: FnMut(&FrozenAction, &CommitteeContract) -> Result<DriverEvidence, Error>,
    {
        let attempt = self.job.as_ref().map(|job| job.attempt);
        let mut feeds = Vec::with_capacity(2);
        let mut reads = Vec::with_capacity(2);
        let result = self.step_with_provider(clock, &mut FeedProvider {
            inner: PublicationProvider { inner: Callback(provider), attempt, source, reads: &mut reads },
            source: feed, reports: &mut feeds,
        }, human, credential);
        FileFeedDriverReport { feeds, publication: FilePublicationDriverReport {
            reads, evidence: FileEvidenceReport { observations: Vec::new(), source_updates: Vec::new(), result },
        } }
    }

    /// Concrete change-feed, native policy/committee and witness readers. Reuse
    /// the original policy-source lease/floor checks rather than a cached callback.
    /// These reads are sequential, not an atomic snapshot across producers.
    pub fn step_from_files_with_publication_feed<S, F>(&mut self,
        evidence_source: &mut S, source: &PublicationInputFile,
        feed: &PublicationFeedFile, clock: F,
        human: Option<&FileHumanPermit>, credential: Option<&FileCredentialPermit>)
        -> FileFeedDriverReport<FileDriverEvent>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let attempt = self.job.as_ref().map(|job| job.attempt);
        let mut feeds = Vec::with_capacity(2);
        let mut observations = Vec::with_capacity(2);
        let mut source_updates = Vec::with_capacity(2);
        let mut reads = Vec::with_capacity(2);
        let result = self.step_with_provider(clock, &mut FeedProvider {
            inner: PublicationProvider {
                inner: FileProvider { source: evidence_source, observations: &mut observations, updates: &mut source_updates },
                attempt, source, reads: &mut reads,
            },
            source: feed, reports: &mut feeds,
        }, human, credential);
        FileFeedDriverReport { feeds, publication: FilePublicationDriverReport {
            reads, evidence: FileEvidenceReport { observations, source_updates, result },
        } }
    }
}
pub(super) struct FeedProvider<'a, P> {
    pub(super) inner: P,
    pub(super) source: &'a PublicationFeedFile,
    pub(super) reports: &'a mut Vec<Result<PublicationFeedReport, FileCaptureError>>,
}
impl<P: EvidenceProvider> EvidenceProvider for FeedProvider<'_, P> {
    fn capture<F>(&mut self, host: &mut FileOversight, action: &FrozenAction, clock: &mut F)
        -> Result<Result<DriverEvidence, Error>, JournalError>
    where F: FnMut() -> ElapsedTick {
        // Unlike a heartbeat-only refresh, notices can invalidate the active
        // attempt and increment its publication-input revision. Feed catch-up
        // must finish BEFORE PublicationProvider begins that attempt's capture.
        // Its durable global withdrawal protects the preceding external I/O.
        let report = host.refresh_publication_feed(host.revision(), self.source, &mut *clock)?;
        self.reports.push(report);
        // Missing/stalled feeds refuse in the native effect gate, not by inventing
        // committee drift. This keeps an existing unspent permit retryable after
        // genuine catch-up. An outer install failure never invokes the inner path.
        self.inner.capture(host, action, clock)
    }
}
