//! Three actual source boundaries without a policy/committee callback.
use super::{CapturedCompletionKeys, CompletionDriverEvidence, DriverEvidence,
    ElapsedTick, Error, EvidenceProvider, FeedCompletionReport, FileCaptureError,
    FileCaptureIdentity, FileCredentialPermit, FileHumanPermit, FileSupervisedDriver,
    FrozenAction, JournalError, PublicationFeedFile, PublicationFeedReport, PublicationInputFile};
use super::super::super::FileProvider;
use crate::action::consequence::delivery::persistent::observed::{FileOversight,
    source::FileSourceError, publication::capture::completion::files::FilesCompletionReport};
use crate::action::consequence::oversight::evidence_source::{EvidenceError, EvidenceFile, EvidenceIdentity};

/// Authorization source events are acknowledged separately. Final policy/feed
/// updates appear only with the whole completion cut. Reads alone grant nothing.
#[derive(Debug)]
pub struct FileFilesCompletionReport {
    pub authorization_observations: Vec<Result<EvidenceIdentity, EvidenceError>>,
    pub authorization_source_updates: Vec<Result<EvidenceIdentity, FileSourceError>>,
    pub authorization_feeds: Vec<Result<PublicationFeedReport, FileCaptureError>>,
    pub authorization_reads: Vec<Result<FileCaptureIdentity, FileCaptureError>>,
    pub completion: FilesCompletionReport,
}
impl FileSupervisedDriver {
    /// Use the original concrete file provider for authorization, then refresh
    /// native policy/committee evidence inside original atomic completion. The
    /// same retained automatic permit, independent human key and job transitions
    /// are shared with callback completion. Optional feed catch-up precedes the
    /// witness revision at ALL boundaries. No callback or cached packet fallback.
    ///
    /// The native file-source gate must be configured before proposals. Success
    /// closes the settled job; a caught completion unwind retires its send phase.
    /// Source read/clock/install failures follow native withdrawal or quarantine,
    /// never reauthorization under new committee inputs or unknown-effect refunds.
    pub fn complete_from_files_with_publication<S, F>(&mut self, policy_source: &mut S,
        source: &PublicationInputFile, feed: Option<&PublicationFeedFile>, clock: F,
        human: &FileHumanPermit, credential: Option<&FileCredentialPermit>) -> FileFilesCompletionReport
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let mut provider = FilesEvidence { source: policy_source,
            authorization_observations: Vec::with_capacity(1), authorization_updates: Vec::with_capacity(1),
            observations: Vec::new(), committed_source_updates: Vec::new() };
        let report = self.complete_source_job(source, feed, clock, &mut provider, human, credential);
        FileFilesCompletionReport {
            authorization_observations: provider.authorization_observations,
            authorization_source_updates: provider.authorization_updates,
            authorization_feeds: report.authorization_feeds,
            authorization_reads: report.authorization_reads,
            completion: FilesCompletionReport { observations: provider.observations,
                committed_source_updates: provider.committed_source_updates, completion: report.completion },
        }
    }
}

struct FilesEvidence<'a, S: ?Sized> {
    source: &'a mut S,
    authorization_observations: Vec<Result<EvidenceIdentity, EvidenceError>>,
    authorization_updates: Vec<Result<EvidenceIdentity, FileSourceError>>,
    observations: Vec<Result<EvidenceIdentity, EvidenceError>>,
    committed_source_updates: Vec<Result<EvidenceIdentity, Error>>,
}
impl<S: EvidenceFile + ?Sized> EvidenceProvider for FilesEvidence<'_, S> {
    fn capture<F: FnMut() -> ElapsedTick>(&mut self, host: &mut FileOversight,
        action: &FrozenAction, clock: &mut F) -> Result<Result<DriverEvidence, Error>, JournalError>
    {
        // Reuse native read-start timing, durable refusal and source-interruption
        // semantics. Authorization does not use the speculative completion owner.
        FileProvider { source: &mut *self.source, observations: &mut self.authorization_observations,
            updates: &mut self.authorization_updates }.capture(host, action, clock)
    }
}
impl<S: EvidenceFile + ?Sized> CompletionDriverEvidence for FilesEvidence<'_, S> {
    fn preflight(&self, host: &FileOversight) -> Result<(), Error> {
        if !host.file_source_required() || host.source_interrupted { return Err(Error::Incomplete); }
        Ok(())
    }
    fn complete<F: FnMut() -> ElapsedTick>(&mut self, host: &mut FileOversight,
        revision: u64, keys: CapturedCompletionKeys<'_>, source: &PublicationInputFile,
        feed: Option<&PublicationFeedFile>, clock: &mut F) -> FeedCompletionReport
    {
        let report = host.complete_publication_from_files(revision, keys, &mut *self.source, source, feed, clock);
        self.observations = report.observations;
        self.committed_source_updates = report.committed_source_updates;
        report.completion
    }
}
