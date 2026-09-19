//! Source-bound publication captures inside the ORIGINAL supervised driver.
//! Reuse its review, authorization, two-key dispatch and publish/reconcile phases.
//! This is an acquisition adapter, not another driver or a permitting fallback.
use super::{FileEvidenceReport, FileProvider};
use super::super::{CommitteeContract, DriverEvidence, ElapsedTick, Error,
    FileCredentialPermit, FileDriverEvent, FileHumanPermit, FileOversight,
    FileSupervisedDriver, FrozenAction, JournalError};
use super::super::provider::{Callback, EvidenceProvider, validate};
use super::super::super::publication::capture::{
    FileCaptureError, FileCaptureIdentity, PublicationInputFile,
};
use crate::action::consequence::oversight::evidence_source::EvidenceFile;

/// Actual witness-file reads alongside the original committee-source report.
/// At most two acquisitions occur in one driver step. A successful read identity
/// is NOT an acknowledged update: inspect evidence.result for installation or
/// publication failures. An empty reads vector can mean no boundary needed I/O,
/// a failed preflight, or that the committee provider failed before the file read.
#[derive(Debug)]
pub struct FilePublicationDriverReport<T> {
    pub reads: Vec<Result<FileCaptureIdentity, FileCaptureError>>,
    pub evidence: FileEvidenceReport<T>,
}

impl FileSupervisedDriver {
    /// Reopen the concrete witness source at every evidence boundary of the
    /// existing callback driver. Bind the source/recipe on the durable host before
    /// using this path. The callback supplies committee evidence only; it cannot
    /// supply, replace, or bypass the source-bound publication capture.
    ///
    /// Idle, reconciliation and already resolved/expired publication do not read
    /// either provider. Credential capability remains optional here and required
    /// by a credential-guarded host at its original first-publication boundary.
    pub fn step_with_publication_source<F, P>(&mut self, source: &PublicationInputFile,
        clock: F, provider: P, human: Option<&FileHumanPermit>,
        credential: Option<&FileCredentialPermit>) -> FilePublicationDriverReport<FileDriverEvent>
    where F: FnMut() -> ElapsedTick,
        P: FnMut(&FrozenAction, &CommitteeContract) -> Result<DriverEvidence, Error>,
    {
        let attempt = self.job.as_ref().map(|job| job.attempt);
        let mut reads = Vec::with_capacity(2);
        let result = self.step_with_provider(clock, &mut PublicationProvider {
            inner: Callback(provider), attempt, source, reads: &mut reads,
        }, human, credential);
        FilePublicationDriverReport { reads, evidence: FileEvidenceReport {
            observations: Vec::new(), source_updates: Vec::new(), result,
        } }
    }

    /// Both sources use concrete readers. The inner adapter is the SAME native
    /// file/source gate used by step_from_file, including source failure and
    /// generation/freshness handling; no callback can substitute a cached image.
    /// The two reads are sequential, not an atomic cross-producer snapshot.
    pub fn step_from_files_with_publication_source<S, F>(&mut self,
        evidence_source: &mut S, source: &PublicationInputFile, clock: F,
        human: Option<&FileHumanPermit>, credential: Option<&FileCredentialPermit>)
        -> FilePublicationDriverReport<FileDriverEvent>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let attempt = self.job.as_ref().map(|job| job.attempt);
        let mut observations = Vec::with_capacity(2);
        let mut source_updates = Vec::with_capacity(2);
        let mut reads = Vec::with_capacity(2);
        let result = self.step_with_provider(clock, &mut PublicationProvider {
            inner: FileProvider { source: evidence_source, observations: &mut observations,
                updates: &mut source_updates },
            attempt, source, reads: &mut reads,
        }, human, credential);
        FilePublicationDriverReport { reads, evidence: FileEvidenceReport {
            observations, source_updates, result,
        } }
    }
}

struct PublicationProvider<'a, P> {
    inner: P,
    attempt: Option<u64>,
    source: &'a PublicationInputFile,
    reads: &'a mut Vec<Result<FileCaptureIdentity, FileCaptureError>>,
}
impl<P: EvidenceProvider> EvidenceProvider for PublicationProvider<'_, P> {
    fn capture<F>(&mut self, host: &mut FileOversight, action: &FrozenAction, clock: &mut F)
        -> Result<Result<DriverEvidence, Error>, JournalError>
    where F: FnMut() -> ElapsedTick {
        let attempt = self.attempt.ok_or(Error::Missing)?;
        // Commit withdrawal BEFORE either external observation, including before
        // a committee callback can unwind. Neither an outer nor inner failure can
        // leave a previous witness capture eligible for a later direct publish.
        let revision = host.revision();
        let expected = host.begin_publication_capture(revision, attempt, self.source.source())?;
        let captured = self.inner.capture(host, action, clock)?
            .and_then(|evidence| validate(evidence, action, &host.profile.committee));
        let evidence = match captured { Ok(evidence) => evidence, Err(error) => return Ok(Err(error)) };
        let capture = match self.source.read_capture() {
            Ok(capture) => { self.reads.push(Ok(capture.identity())); capture }
            Err(error) => { self.reads.push(Err(error)); return Ok(Err(error.contract_error())); }
        };
        // Source/action/generation and immutable requirements are checked by the
        // original journal candidate. A failed install is an OUTER journal error,
        // never a missing-source fallback that could acknowledge another result.
        host.finish_publication_capture(attempt, expected, capture)?;
        Ok(Ok(evidence))
    }
}
