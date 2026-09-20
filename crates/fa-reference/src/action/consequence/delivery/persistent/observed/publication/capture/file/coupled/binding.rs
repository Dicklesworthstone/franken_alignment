//! Capture original requirements at the SAME producer cut as their change feed.
//! Existing recipes cannot be replaced and this operation creates no current input.
use super::{FileCaptureError, FileCaptureIdentity, FileOversight, JournalError,
    JournalFailure, JournalIo, PublicationFeedFile, PublicationFeedReport,
    PublicationInputFile};
use super::super::super::SourceBinding;
use crate::action::{ActionState, ElapsedTick};
use crate::witness::{WitnessRequest, MAX_WITNESSES};
use crate::Error;
use std::rc::Rc;

impl FileOversight {
    /// Bind a NEW attempt from one actual producer read, including all available
    /// change records before deriving its original requirements. Unlike refresh,
    /// this does not require a previously bound source and never makes it fresh.
    /// Call before launching helpers. No historical judgment is rebased.
    ///
    /// Feed withdrawal is durable before I/O. The suffix, heartbeat and original
    /// source binding then share one canonical replacement. A missing retained
    /// prefix is not inferred from the snapshot: native cut admission refuses it.
    /// Missing/malformed files retain withdrawal without binding. After a read,
    /// invalid requirements, clock, allocation, replay or storage failure closes
    /// this owner until recovery; no candidate binding or report is acknowledged.
    /// The ordinary capture/authorize/dispatch/publication path must still run.
    pub fn bind_publication_from_producer<F>(&mut self, revision: u64, attempt: u64,
        source: &PublicationInputFile, feed: &PublicationFeedFile,
        requests: Vec<WitnessRequest>, mut clock: F)
        -> Result<Result<(FileCaptureIdentity, PublicationFeedReport), FileCaptureError>, JournalError>
    where F: FnMut() -> ElapsedTick {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        if self.inspect().control.ledger.stages.get(&attempt) != Some(&ActionState::Reviewing) {
            return Err(Error::WrongState.into());
        }
        if requests.len() > MAX_WITNESSES { return Err(Error::Limit.into()); }
        match self.retained_publication_evidence(attempt) {
            Ok(_) => return Err(Error::Duplicate.into()),
            Err(JournalError::Contract(Error::Missing)) => {}
            Err(error) => return Err(error),
        }
        if self.machine.sessions.values().any(|(id, _)| *id == attempt) {
            return Err(Error::WrongState.into());
        }
        let action = self.machine.actions.get(&attempt).ok_or(Error::Missing)?;
        source.check_producer_binding(feed, attempt, action)?;
        if self.publication_source(attempt)?.is_some() { return Err(Error::Duplicate.into()); }
        if self.publication_change_status()?.source != feed.source() { return Err(Error::Binding.into()); }
        self.publication_change_freshness()?;
        // There is no current bound witness to withdraw yet. The mandatory
        // unbound slot cannot authorize; existing attempts lose feed eligibility.
        self.publication_changes_unavailable(revision, feed.source())?;
        let (capture, batch) = match source.read_coupled(feed) {
            Ok(pair) => pair,
            Err(error) => return Ok(Err(error)),
        };
        self.fault = Some(JournalFailure { operation: JournalIo::Stage,
            kind: std::io::ErrorKind::Other, replacement_may_be_visible: false });
        // Empty structured recipes are supported only by whole-input evidence.
        // Preserve the newer supervisor profile's mandatory opaque lane.
        if requests.is_empty() && capture.inputs().opaque().is_none() {
            return Err(Error::Incomplete.into());
        }
        let identity = capture.identity();
        let binding = Rc::new(SourceBinding::new(capture, requests)?);
        let now = clock();
        let report = self.install_feed_binding(batch, now, attempt, binding)?;
        Ok(Ok((identity, report)))
    }
}
