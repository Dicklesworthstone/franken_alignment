//! Original publication-witness acquisition around current learned evidence.
//! Native helper inputs still come only from the owned learned sidecar. These
//! adapters reuse the same capture/feed withdrawal, validation and final seal.
use super::super::super::{FileSupervisedDriver, FileDriverEvent};
use super::super::super::{FileHumanPermit, FileCredentialPermit};
use super::super::super::evidence::FileEvidenceReport;
use super::super::super::evidence::publication::{PublicationProvider, FilePublicationDriverReport,
    feed::{FeedProvider, FileFeedDriverReport},
    deferred::{deferrable_provider, FileDeferrableDriverReport}};
use super::super::super::learned::evidence::policy_file_provider;
use super::super::super::super::publication::capture::{PublicationInputFile,
    heartbeat::feed::PublicationFeedFile};
use crate::action::ElapsedTick;
use crate::action::consequence::oversight::evidence_source::EvidenceFile;

impl FileSupervisedDriver {
    /// Observe a valid lagging capture without replacing current learned helper
    /// input or ending its original review. Only an acknowledged deferral while
    /// Ready, with the original live human key, yields a waiting diagnostic.
    /// The caller must bound retries and service Stop/cancellation between them.
    /// Dispatched and coupled-producer acquisition retain the strict path.
    pub fn step_computed_from_policy_file_with_publication_deferral<S, F>(&mut self,
        evidence: &mut S, source: &PublicationInputFile, feed: Option<&PublicationFeedFile>,
        clock: F, human: Option<&FileHumanPermit>, credential: Option<&FileCredentialPermit>)
        -> FileDeferrableDriverReport
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let expected = self.job.as_ref().map(|job| (job.attempt, job.input_revision));
        let mut observations = Vec::with_capacity(2);
        let mut source_updates = Vec::with_capacity(2);
        let mut reads = Vec::with_capacity(2);
        let mut feeds = Vec::with_capacity(2);
        let mut captures = Vec::with_capacity(2);
        let result = self.step_computed_with_provider(clock, &mut deferrable_provider(
            policy_file_provider(expected, evidence, &mut observations, &mut source_updates),
            expected.map(|(attempt, _)| attempt), source, feed,
            &mut reads, &mut feeds, &mut captures), human, credential);
        let waiting = self.publication_waiting(&result, &reads, &captures, human);
        FileDeferrableDriverReport { captures, waiting, step: FileFeedDriverReport {
            feeds, publication: FilePublicationDriverReport {
                reads, evidence: FileEvidenceReport { observations, source_updates, result },
            },
        } }
    }

    /// Reacquire the concrete witness file at the original evidence boundaries,
    /// retaining actual learned input before and after the policy-only file read.
    /// The original source must already be bound to the exact admitted attempt.
    /// Missing evidence never selects the unchecked or ordinary-helper path.
    pub fn step_computed_from_policy_file_with_publication_source<S, F>(&mut self,
        evidence: &mut S, source: &PublicationInputFile, clock: F,
        human: Option<&FileHumanPermit>, credential: Option<&FileCredentialPermit>)
        -> FilePublicationDriverReport<FileDriverEvent>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let expected = self.job.as_ref().map(|job| (job.attempt, job.input_revision));
        let mut observations = Vec::with_capacity(2);
        let mut source_updates = Vec::with_capacity(2);
        let mut reads = Vec::with_capacity(2);
        let result = self.step_computed_with_provider(clock, &mut PublicationProvider {
            inner: policy_file_provider(expected, evidence, &mut observations, &mut source_updates),
            attempt: expected.map(|(attempt, _)| attempt), source, reads: &mut reads,
        }, human, credential);
        FilePublicationDriverReport { reads, evidence: FileEvidenceReport {
            observations, source_updates, result,
        } }
    }

    /// Acquire the original change tail/lease and witness input without replacing
    /// learned helper evidence. A coupled producer is still one atomic read;
    /// separate producers keep their existing explicitly sequential contract.
    /// Query-only settlement and stopped/expired preflights retain source-free
    /// behavior through the same underlying driver.
    pub fn step_computed_from_policy_file_with_publication_feed<S, F>(&mut self,
        evidence: &mut S, source: &PublicationInputFile, feed: &PublicationFeedFile,
        clock: F, human: Option<&FileHumanPermit>, credential: Option<&FileCredentialPermit>)
        -> FileFeedDriverReport<FileDriverEvent>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let expected = self.job.as_ref().map(|job| (job.attempt, job.input_revision));
        let mut observations = Vec::with_capacity(2);
        let mut source_updates = Vec::with_capacity(2);
        let mut reads = Vec::with_capacity(2);
        let mut feeds = Vec::with_capacity(2);
        let result = self.step_computed_with_provider(clock, &mut FeedProvider {
            inner: PublicationProvider {
                inner: policy_file_provider(expected, evidence, &mut observations, &mut source_updates),
                attempt: expected.map(|(attempt, _)| attempt), source, reads: &mut reads,
            }, source: feed, reports: &mut feeds,
        }, human, credential);
        FileFeedDriverReport { feeds, publication: FilePublicationDriverReport {
            reads, evidence: FileEvidenceReport { observations, source_updates, result },
        } }
    }
}
