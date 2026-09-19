//! Concrete policy/committee and witness capture inside original atomic completion.
//! Native source observations are staged, not separately committed or replaced
//! by a callback pretending to have refreshed the source lease.
use super::{CapturedCompletionKeys, CapturedCompletionReport, Completion, CompletionEvidence,
    DriverEvidence, ElapsedTick, Error, Event, FileOversight, FrozenAction, JournalError,
    PublicationFeedFile, PublicationInputFile, SourceCut, Transition, failure};
use super::feed::FeedCompletionReport;
use super::super::super::super::source::SourceEvent;
use crate::action::consequence::oversight::evidence_source::{EvidenceError, EvidenceFile, EvidenceIdentity};

/// Actual reads survive failed completion. `committed_source_updates` and the
/// nested feed's `committed` are empty unless the ENTIRE effect cut was written.
/// A native source refusal is distinguishable from its successful file read.
#[derive(Debug)]
pub struct FilesCompletionReport {
    pub observations: Vec<Result<EvidenceIdentity, EvidenceError>>,
    pub committed_source_updates: Vec<Result<EvidenceIdentity, Error>>,
    pub completion: FeedCompletionReport,
}

impl FileOversight {
    /// Complete with two actual policy/committee reads and two witness reads,
    /// optionally catching up the concrete feed before each acquisition. Requires
    /// the original native file-source gate and an uninterrupted original owner.
    /// A source-only profile passes None; a configured feed remains mandatory in
    /// the original effect gate even when not refreshed through this entry point.
    ///
    /// Identical source bytes renew only their native observation lease; a newer
    /// source or read loss uses ORIGINAL invalidation/revocation. It cannot rebase
    /// the committee review or human approval. Policy leases begin BEFORE reading,
    /// and both native effect checks recheck time after all captures.
    ///
    /// Once policy acquisition begins, a failed first capture or installation
    /// quarantines this owner. A failed second read stages native withdrawal and
    /// seals/reconciles through the original publisher. No candidate source update
    /// becomes acknowledged before the final canonical replacement succeeds.
    pub fn complete_publication_from_files<S, F>(&mut self, revision: u64,
        keys: CapturedCompletionKeys<'_>, policy_source: &mut S,
        source: &PublicationInputFile, feed: Option<&PublicationFeedFile>, clock: F)
        -> FilesCompletionReport
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let mut completion = Completion { keys, source, feed, clock,
            provider: FileCompletionEvidence { source: policy_source,
                observations: Vec::with_capacity(2), updates: Vec::with_capacity(2) },
            reads: Vec::with_capacity(2), evidence_failure: None,
            feed_reads: Vec::with_capacity(2), feeds: Vec::with_capacity(2) };
        let result = completion.run(self, revision);
        let committed = result.is_ok();
        FilesCompletionReport {
            observations: completion.provider.observations,
            committed_source_updates: if committed { completion.provider.updates } else { Vec::new() },
            completion: FeedCompletionReport {
                reads: completion.feed_reads,
                committed: if committed { completion.feeds } else { Vec::new() },
                completion: CapturedCompletionReport {
                    reads: completion.reads, evidence_failure: completion.evidence_failure, result,
                },
            },
        }
    }
}

struct FileCompletionEvidence<'a, S: ?Sized> {
    source: &'a mut S,
    observations: Vec<Result<EvidenceIdentity, EvidenceError>>,
    updates: Vec<Result<EvidenceIdentity, Error>>,
}
impl<S: EvidenceFile + ?Sized> CompletionEvidence for FileCompletionEvidence<'_, S> {
    fn preflight(&self, host: &FileOversight) -> Result<(), Error> {
        if !host.file_source_required() || host.source_interrupted { return Err(Error::Incomplete); }
        Ok(())
    }
    fn fixed_events(&self) -> usize { 2 }
    fn capture<F: FnMut() -> ElapsedTick>(&mut self, host: &mut FileOversight,
        cut: &mut SourceCut, action: &FrozenAction, clock: &mut F)
        -> Result<Result<DriverEvidence, Error>, JournalError>
    {
        // The original witness (and optional feed) withdrawal is already durable.
        // Close BOTH live and candidate admission before clock/reader/allocator
        // code. Only a staged native source transition clears the candidate;
        // only the whole-cut acknowledgment clears the LIVE source interruption.
        host.source_interrupted = true;
        host.fault = Some(failure(false));
        cut.source_interrupted = true;
        let observed_at = clock();
        let read = self.source.read_evidence();
        self.observations.push(read.as_ref().map(|capture| capture.identity()).map_err(|error| *error));
        let captured = match read {
            Ok(captured) => captured,
            Err(error) => {
                cut.stage(host, Event::Source(SourceEvent::Withdraw))?;
                let error = match error { EvidenceError::Data(error) => error, EvidenceError::Io(_) => Error::Incomplete };
                self.updates.push(Err(error));
                return Ok(Err(error));
            }
        };
        let identity = captured.identity();
        let transition = cut.stage(host, Event::Source(SourceEvent::Observe(captured.clone(), observed_at)))?;
        let Transition::SourceObserved(applied) = transition else { return Err(Error::Binding.into()); };
        self.updates.push(applied.as_ref().map(|_| identity).map_err(|error| *error));
        if let Err(error) = applied { return Ok(Err(error)); }
        Ok(captured.inputs_for(action, &host.profile.committee).map(|inputs| DriverEvidence {
            inputs: Some(inputs), snapshot: captured.snapshot().clone(),
        }))
    }
}
