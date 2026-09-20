//! One physical observation of a coupled producer, then one acknowledged install.
//! No read cache or caller-supplied positive observation importer is exposed.
use super::{FileCaptureError, FilePublicationCapture, PublicationInputFile,
    PublicationProducerImage, FrozenAction, MAX_PRODUCER_BYTES};
use super::super::heartbeat::feed::{PublicationFeedBatch, PublicationFeedFile, PublicationFeedReport};
use super::super::{FileCaptureIdentity, FileOversight, JournalError, JournalFailure, JournalIo};
use crate::action::ElapsedTick;
use crate::Error;

impl PublicationInputFile {
    pub(in crate::action::consequence::delivery::persistent::observed) fn shares_producer(
        &self, feed: &PublicationFeedFile) -> bool
    {
        self.producer.as_ref().is_some_and(|(profile, _, _)| feed.pairs_with(&self.path, *profile))
    }

    pub(in crate::action::consequence::delivery::persistent::observed) fn check_producer_binding(
        &self, feed: &PublicationFeedFile, attempt: u64, action: &FrozenAction) -> Result<(), Error>
    {
        if !self.shares_producer(feed) { return Err(Error::Binding); }
        let (_, bound_attempt, bound_action) = self.producer.as_ref().ok_or(Error::Binding)?;
        if *bound_attempt != attempt || bound_action != action { return Err(Error::Binding); }
        Ok(())
    }

    pub(in crate::action::consequence::delivery::persistent::observed) fn read_coupled(
        &self, feed: &PublicationFeedFile) -> Result<(FilePublicationCapture, PublicationFeedBatch), FileCaptureError>
    {
        if !self.shares_producer(feed) { return Err(Error::Binding.into()); }
        let (profile, attempt, action) = self.producer.as_ref().ok_or(Error::Binding)?;
        // Reuse the original bounded regular-file/metadata/sentinel read. Both
        // projections come from this SAME decoded image, with no second open.
        let image = PublicationProducerImage::from_bytes(&self.read_bytes(MAX_PRODUCER_BYTES)?)?;
        if image.profile() != *profile { return Err(Error::Binding.into()); }
        Ok((image.capture(*attempt, action)?, image.batch().clone()))
    }
}

impl FileOversight {
    /// Read a matched pair of explicit producer-bundle readers ONCE. Withdraw
    /// feed and witness eligibility before I/O, then acknowledge the derived
    /// notice suffix, heartbeat and exact input image in one canonical replacement.
    /// A read failure retains both withdrawals. A clock/install/storage failure
    /// after successful decode quarantines this owner until original recovery.
    /// The returned identity/feed report are observations, never effect rights.
    pub fn refresh_publication_from_producer<F>(&mut self, revision: u64, attempt: u64,
        source: &PublicationInputFile, feed: &PublicationFeedFile, mut clock: F)
        -> Result<Result<(FileCaptureIdentity, PublicationFeedReport), FileCaptureError>, JournalError>
    where F: FnMut() -> ElapsedTick {
        self.begin_producer_capture(revision, attempt, source, feed)?;
        self.finish_producer_capture(source, feed, &mut clock)
    }

    pub(in crate::action::consequence::delivery::persistent::observed) fn begin_producer_capture(
        &mut self, revision: u64, attempt: u64, source: &PublicationInputFile, feed: &PublicationFeedFile)
        -> Result<(), JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        source.check_producer_binding(feed, attempt, self.machine.actions.get(&attempt).ok_or(Error::Missing)?)?;
        let bound = self.publication_source(attempt)?.ok_or(Error::Incomplete)?;
        if bound.source != source.source() || self.publication_change_status()?.source != feed.source() {
            return Err(Error::Binding.into());
        }
        if self.publication_input_cut(attempt)?.is_none() { return Err(Error::Incomplete.into()); }
        self.publication_change_freshness()?;
        self.publication_changes_unavailable(revision, feed.source())?;
        self.begin_publication_capture(self.revision(), attempt, source.source())?;
        Ok(())
    }

    pub(in crate::action::consequence::delivery::persistent::observed) fn finish_producer_capture<F>(
        &mut self, source: &PublicationInputFile, feed: &PublicationFeedFile, clock: &mut F)
        -> Result<Result<(FileCaptureIdentity, PublicationFeedReport), FileCaptureError>, JournalError>
    where F: FnMut() -> ElapsedTick {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        let (capture, batch) = match source.read_coupled(feed) {
            Ok(pair) => pair,
            Err(error) => return Ok(Err(error)),
        };
        let identity = capture.identity();
        let report = self.install_producer_observation(capture, batch, clock)?;
        Ok(Ok((identity, report)))
    }

    pub(in crate::action::consequence::delivery::persistent::observed) fn install_producer_observation<F>(
        &mut self, capture: FilePublicationCapture, batch: PublicationFeedBatch, clock: &mut F)
        -> Result<PublicationFeedReport, JournalError>
    where F: FnMut() -> ElapsedTick {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        // From this point no unwind or failed install may resurrect an older
        // permitting generation. Canonical recovery fences all old effect keys.
        self.fault = Some(JournalFailure { operation: JournalIo::Stage,
            kind: std::io::ErrorKind::Other, replacement_may_be_visible: false });
        let now = clock();
        self.install_feed_observation(batch, now, Some(capture))
    }
}
