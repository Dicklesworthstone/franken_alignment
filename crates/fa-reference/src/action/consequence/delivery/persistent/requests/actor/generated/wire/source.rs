//! Fresh registered observations at the original source-only actor boundary.
//! The source reference is validated before any clock, file read or snapshot use.
use super::{ActorError, ActorProposal, ElapsedTick, FileGeneratedTextActorPort, FileOversight, decode};
use super::super::super::{FileActorExchange, FileActorFeed, FileActorSupervisor, JournalError};
use crate::action::consequence::delivery::persistent::observed::driver::FileSupervisedDriver;
use crate::action::consequence::delivery::persistent::observed::driver::evidence::FileEvidenceReport;
use crate::action::consequence::oversight::actor_wire::{ActorChannel, ActorWire};
use crate::action::consequence::oversight::evidence_source::{EvidenceFile, EvidenceIdentity};

impl FileActorSupervisor<FileOversight> {
    // One private adapter for complete Submit frames. Reuse BOTH existing
    // validators; never derive a fresh observation from a source ID or old slot.
    fn prepare_generated_submission<S, F>(&mut self, request: u64, proposal: &ActorProposal,
        source: &mut S, clock: &mut F, intake: &mut Option<FileEvidenceReport<EvidenceIdentity>>)
        -> Result<(), ActorError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        decode(request, proposal)?;
        self.prepare_wire_submission(request, source, clock, intake)
    }

    /// Acquire the registered file source only for a complete, well-formed NEW
    /// source-reference or finish submission, then invoke the original port.
    /// The original read-start lease and post-read clock check still apply.
    /// Malformed input, poll, cancel and recorded retries do no source/clock work.
    /// Conflicting recorded requests remain subject to original exact binding.
    ///
    /// Source preparation and submission are separate acknowledged transactions:
    /// a successful intake report does not mean the following proposal committed.
    /// Private producer/IO diagnostics remain in intake, never the actor response.
    /// A foreign gateway refuses before parsing, observation or host mutation.
    pub fn exchange_generated_actor_from_file<S, F>(&mut self,
        wire: &mut ActorWire<FileGeneratedTextActorPort>, document: &[u8], source: &mut S,
        mut clock: F) -> Result<FileActorExchange, JournalError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.check_source_wire(&wire.request_port().port)?;
        let mut intake = None;
        let response = wire.exchange_with_admission(document, |_, request, proposal| {
            self.prepare_generated_submission(request, proposal, source, &mut clock, &mut intake)
        });
        Ok(FileActorExchange { response, intake })
    }

    /// Use original bounded newline framing and write-then-flush backpressure.
    /// Fragments, an unterminated document, or a pending response cannot trigger
    /// a read for the next request. FeedResult retains its exact consumed prefix.
    /// No unbounded queue, observation callback supplied by an actor, implicit
    /// generation step or extra effect authority is introduced.
    pub fn feed_generated_actor_from_file<S, F>(&mut self,
        channel: &mut ActorChannel<FileGeneratedTextActorPort>, bytes: &[u8], source: &mut S,
        mut clock: F) -> Result<FileActorFeed, JournalError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.check_source_wire(&channel.request_port().port)?;
        let mut intake = None;
        let feed = channel.feed_with_admission(bytes, |_, request, proposal| {
            self.prepare_generated_submission(request, proposal, source, &mut clock, &mut intake)
        });
        Ok(FileActorFeed { feed, intake })
    }
}

impl FileSupervisedDriver {
    /// Same intake, with existing helper cleanup after success or refusal.
    pub fn exchange_generated_actor_from_file<S, F>(&mut self,
        wire: &mut ActorWire<FileGeneratedTextActorPort>, document: &[u8], source: &mut S,
        clock: F) -> Result<FileActorExchange, JournalError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let result = self.supervisor_mut().exchange_generated_actor_from_file(wire, document, source, clock);
        self.reap_helpers();
        result
    }

    pub fn feed_generated_actor_from_file<S, F>(&mut self,
        channel: &mut ActorChannel<FileGeneratedTextActorPort>, bytes: &[u8], source: &mut S,
        clock: F) -> Result<FileActorFeed, JournalError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let result = self.supervisor_mut().feed_generated_actor_from_file(channel, bytes, source, clock);
        self.reap_helpers();
        result
    }
}

#[cfg(test)]
mod tests;
