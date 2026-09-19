//! Feed catch-up inside the SAME private dispatch/publication/accounting cut.
//! Only the final acknowledgment can expose installed feed reports.
use super::{CapturedCompletionKeys, CapturedCompletionReport, Completion, SourceCut, CallbackEvidence, CompletionEvidence,
    CommitteeContract, DriverEvidence, ElapsedTick, Error, Event, FileCaptureError,
    FileOversight, FreshnessEvent, FrozenAction, JournalError, PublicationFeedFile,
    PublicationFeedReport, PublicationHeartbeat, PublicationInputFile, WitnessEvent, failure};
use super::super::heartbeat::feed::{PublicationFeedBatch, ingest::check_feed_overlap};

/// At most two actual feed reads, alongside the completion's witness reads.
/// Reads are not installations. `committed` is empty on failure; otherwise its
/// reports were acknowledged together with publication and original settlement.
/// Their eligibility is historical; the final gate checks time after all captures.
#[derive(Debug)]
pub struct FeedCompletionReport {
    pub reads: Vec<Result<PublicationHeartbeat, FileCaptureError>>,
    pub committed: Vec<PublicationFeedReport>,
    pub completion: CapturedCompletionReport,
}

impl FileOversight {
    /// Catch up the concrete change feed before EACH witness acquisition, then
    /// execute the original two-key dispatch, final validation and settlement.
    /// Feed records can invalidate the active attempt, so its capture revision
    /// is obtained AFTER catch-up. No heartbeat or notice skips exact validation.
    ///
    /// Feed and witness withdrawal are separate durable writes before I/O. All
    /// subsequent feed records, heartbeat observations, native dispatch, sink
    /// publication and receipt accounting share one canonical replacement. A
    /// second feed outage seals through the original endpoint. A conflicting
    /// overlap or failed install quarantines instead of fabricating a receipt.
    ///
    /// This callback does not refresh a separately configured policy-source
    /// lease. No cross-source snapshot, remote atomicity or authenticity is claimed.
    pub fn complete_publication_from_feed<F, P>(&mut self, revision: u64,
        keys: CapturedCompletionKeys<'_>, source: &PublicationInputFile,
        feed: &PublicationFeedFile, clock: F, provider: P) -> FeedCompletionReport
    where F: FnMut() -> ElapsedTick,
        P: FnMut(&FrozenAction, &CommitteeContract) -> Result<DriverEvidence, Error>,
    {
        let mut completion = Completion { keys, source, clock, provider: CallbackEvidence(provider),
            reads: Vec::with_capacity(2), evidence_failure: None,
            feed: Some(feed), feed_reads: Vec::with_capacity(2), feeds: Vec::with_capacity(2) };
        let result = completion.run(self, revision);
        let committed = if result.is_ok() { completion.feeds } else { Vec::new() };
        FeedCompletionReport { reads: completion.feed_reads, committed, completion: CapturedCompletionReport {
            reads: completion.reads, evidence_failure: completion.evidence_failure, result,
        } }
    }
}

impl<F, P> Completion<'_, F, P>
where F: FnMut() -> ElapsedTick, P: CompletionEvidence,
{
    pub(super) fn capture_feed(&mut self, host: &mut FileOversight, cut: &mut SourceCut)
        -> Result<Result<(), FileCaptureError>, JournalError>
    {
        let source = self.feed.ok_or(Error::Incomplete)?;
        let batch = match source.read_batch() {
            Ok(batch) => batch,
            Err(error) => { self.feed_reads.push(Err(error)); return Ok(Err(error)); }
        };
        self.feed_reads.push(Ok(batch.heartbeat()));
        // Do not forget successfully read producer data if the clock, overlap
        // check, replay, allocation or storage fails. Recovery fences old keys.
        host.fault = Some(failure(false));
        let now = (self.clock)();
        let report = cut.stage_feed(host, &batch, now)?;
        self.feeds.push(report);
        Ok(Ok(()))
    }
}

impl SourceCut {
    fn stage_feed(&mut self, host: &FileOversight, batch: &PublicationFeedBatch,
        now: ElapsedTick) -> Result<PublicationFeedReport, JournalError>
    {
        let state = self.machine.broker.publication_change_status()?;
        // Check against this cut's whole history, INCLUDING the first staged
        // acquisition. Two conflicting reads cannot hide behind the live prefix.
        // The concrete live ingester uses the exact same overlap/suffix law.
        check_feed_overlap(&self.history, batch, state.through)?;
        if batch.heartbeat().source != state.source { return Err(Error::Binding.into()); }
        let notices = batch.records().iter().filter(|notice| {
            batch.after() <= state.through && notice.sequence > state.through
        });
        let added = notices.clone().count();
        let count = self.history.len().checked_add(added)
            .and_then(|count| count.checked_add(1)).ok_or(Error::Limit)?;
        if count > host.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        let mut changes = Vec::new();
        changes.try_reserve_exact(added).map_err(|_| Error::Limit)?;
        for notice in notices {
            self.stage(host, Event::PublicationWitness(WitnessEvent::Change(*notice)))?;
            changes.push(self.machine.broker.publication_change_report()?.ok_or(Error::Incomplete)?);
        }
        self.stage(host, Event::PublicationWitness(WitnessEvent::Freshness(
            FreshnessEvent::Observed(batch.heartbeat(), now))))?;
        Ok(PublicationFeedReport { heartbeat: batch.heartbeat(), before: state.through, changes,
            status: self.machine.broker.publication_change_status()?,
            freshness: self.machine.broker.publication_change_freshness()?,
        })
    }
}
