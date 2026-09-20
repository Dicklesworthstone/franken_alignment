//! Explicit, nonpermitting catch-up in the ORIGINAL supervised state machine.
//! No internal retry loop, new key, changed review, or deadline extension.
use super::{Callback, CommitteeContract, DriverEvidence, ElapsedTick, Error,
    EvidenceFile, EvidenceProvider, FileCaptureError, FileCaptureIdentity,
    FileCredentialPermit, FileDriverEvent, FileEvidenceReport, FileHumanPermit,
    FileOversight, FileProvider, FilePublicationDriverReport, FileSupervisedDriver,
    FrozenAction, JournalError, PublicationInputFile, PublicationProvider, validate};
use super::feed::{FeedProvider, FileFeedDriverReport};
use super::super::super::Phase;
use crate::action::ActionState;
use crate::action::consequence::delivery::persistent::observed::publication::capture::deferred::FileCaptureObservation;
use crate::action::consequence::delivery::persistent::observed::publication::capture::heartbeat::feed::{
    PublicationFeedFile, PublicationFeedReport,
};
use crate::action::consequence::oversight::human::HumanDisposition;
use std::rc::Rc;

/// The native gate's result is NEVER converted into a successful driver event.
/// `waiting_for_producer` only identifies an acknowledged lag observation on a
/// still-reviewed, undispatched job with an unexpired matching human key. The
/// caller must bound retries and continue servicing stop/cancellation requests.
#[derive(Debug)]
pub struct FileDeferrableDriverReport {
    pub captures: Vec<FileCaptureObservation>,
    pub step: FileFeedDriverReport<FileDriverEvent>,
    waiting: Option<FileCaptureObservation>,
}
impl FileDeferrableDriverReport {
    pub fn waiting_for_producer(&self) -> Option<FileCaptureObservation> { self.waiting }
}

impl FileSupervisedDriver {
    /// Opt in to the native capture-or-defer operation for a cut-bound source.
    /// Only a valid monotone snapshot behind a COMPLETE feed may defer. Invalid
    /// identity, equivocation, rollback and missing coverage still refuse. Each
    /// call performs at most the original two acquisitions, with real withdrawal
    /// and reads. Existing strict APIs and atomic completion stay strict.
    pub fn step_with_publication_deferral<F, P>(&mut self, source: &PublicationInputFile,
        feed: Option<&PublicationFeedFile>, clock: F, provider: P,
        human: Option<&FileHumanPermit>, credential: Option<&FileCredentialPermit>)
        -> FileDeferrableDriverReport
    where F: FnMut() -> ElapsedTick,
        P: FnMut(&FrozenAction, &CommitteeContract) -> Result<DriverEvidence, Error>,
    {
        let mut reads = Vec::with_capacity(2);
        let mut feeds = Vec::with_capacity(2);
        let mut captures = Vec::with_capacity(2);
        let attempt = self.job.as_ref().map(|job| job.attempt);
        let result = self.step_with_provider(clock, &mut DeferrableProvider {
            inner: Callback(provider), attempt,
            source, feed, reads: &mut reads, feeds: &mut feeds, captures: &mut captures,
        }, human, credential);
        let waiting = self.publication_waiting(&result, &reads, &captures, human);
        FileDeferrableDriverReport { captures, waiting, step: FileFeedDriverReport {
            feeds, publication: FilePublicationDriverReport { reads, evidence: FileEvidenceReport {
                observations: Vec::new(), source_updates: Vec::new(), result,
            } },
        } }
    }

    /// Same operation with the ORIGINAL concrete policy-source adapter. Its
    /// source generation/lease checks are not replaced by witness deferral.
    /// Matching producer bundles keep their one-read paired acquisition; they
    /// cannot legitimately lag their own simultaneously acquired complete feed.
    pub fn step_from_files_with_publication_deferral<S, F>(&mut self,
        evidence_source: &mut S, source: &PublicationInputFile,
        feed: Option<&PublicationFeedFile>, clock: F,
        human: Option<&FileHumanPermit>, credential: Option<&FileCredentialPermit>)
        -> FileDeferrableDriverReport
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let mut observations = Vec::with_capacity(2);
        let mut source_updates = Vec::with_capacity(2);
        let mut reads = Vec::with_capacity(2);
        let mut feeds = Vec::with_capacity(2);
        let mut captures = Vec::with_capacity(2);
        let attempt = self.job.as_ref().map(|job| job.attempt);
        let result = self.step_with_provider(clock, &mut DeferrableProvider {
            inner: FileProvider { source: evidence_source, observations: &mut observations,
                updates: &mut source_updates },
            attempt, source, feed,
            reads: &mut reads, feeds: &mut feeds, captures: &mut captures,
        }, human, credential);
        let waiting = self.publication_waiting(&result, &reads, &captures, human);
        FileDeferrableDriverReport { captures, waiting, step: FileFeedDriverReport {
            feeds, publication: FilePublicationDriverReport { reads, evidence: FileEvidenceReport {
                observations, source_updates, result,
            } },
        } }
    }

    fn publication_waiting(&self, result: &Result<FileDriverEvent, JournalError>,
        reads: &[Result<FileCaptureIdentity, FileCaptureError>], captures: &[FileCaptureObservation],
        human: Option<&FileHumanPermit>) -> Option<FileCaptureObservation>
    {
        if !matches!(result, Err(JournalError::Contract(Error::Incomplete))) { return None; }
        let capture = *captures.last()?;
        if !capture.outcome.deferred() || reads.last()? != &Ok(capture.identity) { return None; }
        let job = self.job.as_ref()?;
        if job.phase != Phase::Ready { return None; }
        let host = self.supervisor.host().ok()?;
        let state = host.inspect();
        if host.storage_failure().is_some() || state.stop.is_some() || state.control.suspended
            || !matches!(state.control.ledger.stages.get(&job.attempt),
                Some(ActionState::Reviewing | ActionState::Authorized)) { return None; }
        job.check_ready(&host, job.inputs.as_ref()).ok()?;
        let now = state.control.ledger.elapsed?;
        if now >= job.action.spec().deadline { return None; }
        let human = human?;
        if !Rc::ptr_eq(&human.issuer, &host.issuer) || human.attempt != job.attempt { return None; }
        let status = host.human_status(human.request).ok()?;
        if status.disposition != HumanDisposition::Approved || now >= status.expires_at { return None; }
        Some(capture)
    }
}

struct Borrowed<'a, P>(&'a mut P);
impl<P: EvidenceProvider> EvidenceProvider for Borrowed<'_, P> {
    fn capture<F: FnMut() -> ElapsedTick>(&mut self, host: &mut FileOversight,
        action: &FrozenAction, clock: &mut F) -> Result<Result<DriverEvidence, Error>, JournalError>
    { self.0.capture(host, action, clock) }
}

struct DeferrableProvider<'a, P> {
    inner: P,
    attempt: Option<u64>,
    source: &'a PublicationInputFile,
    feed: Option<&'a PublicationFeedFile>,
    reads: &'a mut Vec<Result<FileCaptureIdentity, FileCaptureError>>,
    feeds: &'a mut Vec<Result<PublicationFeedReport, FileCaptureError>>,
    captures: &'a mut Vec<FileCaptureObservation>,
}
impl<P: EvidenceProvider> EvidenceProvider for DeferrableProvider<'_, P> {
    fn capture<F: FnMut() -> ElapsedTick>(&mut self, host: &mut FileOversight,
        action: &FrozenAction, clock: &mut F) -> Result<Result<DriverEvidence, Error>, JournalError>
    {
        let attempt = self.attempt.ok_or(Error::Missing)?;
        let undispatched = matches!(host.inspect().control.ledger.stages.get(&attempt),
            Some(ActionState::Reviewing | ActionState::Authorized));
        // Do NOT open a new retry window after dispatch. Preserve the existing
        // exact final publication, sealing, and receipt-only recovery behavior.
        if !undispatched || self.feed.is_some_and(|feed| self.source.shares_producer(feed)) {
            let mut strict = PublicationProvider { inner: Borrowed(&mut self.inner),
                attempt: Some(attempt), source: self.source, reads: self.reads };
            return match self.feed {
                Some(feed) => FeedProvider { inner: strict, source: feed, reports: self.feeds }
                    .capture(host, action, clock),
                None => strict.capture(host, action, clock),
            };
        }
        // Refuse legacy bindings BEFORE a feed read can mutate the owner. This
        // is explicit opt-in, not automatic weakening of strict capture callers.
        host.publication_input_cut(attempt)?.ok_or(Error::Binding)?;
        let bound = host.publication_source(attempt)?.ok_or(Error::Incomplete)?;
        if bound.source != self.source.source() { return Err(Error::Binding.into()); }
        if let Some(feed) = self.feed {
            self.feeds.push(host.refresh_publication_feed(host.revision(), feed, &mut *clock)?);
        }
        let expected = host.begin_deferrable_publication_capture(host.revision(), attempt, self.source.source())?;
        let evidence = match self.inner.capture(host, action, clock)?
            .and_then(|evidence| validate(evidence, action, &host.profile.committee)) {
            Ok(evidence) => evidence,
            Err(error) => return Ok(Err(error)),
        };
        let capture = match self.source.read_capture() {
            Ok(capture) => { self.reads.push(Ok(capture.identity())); capture }
            Err(error) => {
                self.reads.push(Err(error));
                // Missing witness bytes do not assert that the independent
                // committee packet changed. The native gate sees no fresh input.
                return Ok(Ok(evidence));
            }
        };
        self.captures.push(host.finish_publication_capture_or_defer(attempt, expected, capture)?);
        // Deferred installed NO current witness. Returning the actual committee
        // packet preserves its review; original authorize/dispatch still refuses.
        Ok(Ok(evidence))
    }
}
