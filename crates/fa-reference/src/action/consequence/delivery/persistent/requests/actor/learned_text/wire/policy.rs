//! Live policy-only intake on the ORIGINAL wire, channel and authenticated peer.
//! Neither source selection nor the observation callback is an actor capability.
use super::{ActorError, ActorProposal, ActorRequestPort, ElapsedTick,
    FileLearnedTextActorPort, FileLearnedTextStreamActorPort, FileOversight, decode, stream};
use super::super::super::{FileActorPort, FileActorSupervisor, FileActorExchange,
    FileActorFeed, JournalError, redact};
use crate::action::consequence::delivery::persistent::observed::driver::evidence::FileEvidenceReport;
use crate::action::consequence::oversight::actor_wire::{ActorChannel, ActorWire};
use crate::action::consequence::oversight::evidence_source::{EvidenceFile, EvidenceIdentity};
use crate::Error;
#[cfg(target_os = "linux")]
mod peer;

// Private and closed to these two concrete adapters. No new public backend,
// original-port accessor, model reader or caller-supplied admission hook exists.
trait PolicyPort: ActorRequestPort {
    fn original_port(&self) -> &FileActorPort<FileOversight>;
    fn preflight(&self, host: &FileOversight, request: u64, proposal: &ActorProposal)
        -> Result<(), ActorError>;
}
impl PolicyPort for FileLearnedTextActorPort {
    fn original_port(&self) -> &FileActorPort<FileOversight> { &self.port }
    fn preflight(&self, host: &FileOversight, request: u64, proposal: &ActorProposal)
        -> Result<(), ActorError> {
        host.prepare_learned_text_request(request, decode(request, proposal)?)
            .map(|_| ()).map_err(redact)
    }
}
impl PolicyPort for FileLearnedTextStreamActorPort {
    fn original_port(&self) -> &FileActorPort<FileOversight> { &self.port }
    fn preflight(&self, host: &FileOversight, request: u64, proposal: &ActorProposal)
        -> Result<(), ActorError> {
        host.prepare_learned_text_stream_request(request, stream::decode(request, proposal)?, proposal.deadline)
            .map(|_| ()).map_err(redact)
    }
}

fn prepare<P, S, F>(supervisor: &mut FileActorSupervisor<FileOversight>, port: &P,
    request: u64, proposal: &ActorProposal, source: &mut S, clock: &mut F,
    intake: &mut Option<FileEvidenceReport<EvidenceIdentity>>) -> Result<(), ActorError>
where P: PolicyPort, S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
    {
        let host = supervisor.host().map_err(redact)?;
        // Decode and ORIGINAL source/retry checks before any clock, I/O or
        // snapshot mutation. Exact historical retries never need live output.
        port.preflight(&host, request, proposal)?;
        match host.request_status(request) {
            Ok(_) => return Ok(()),
            Err(JournalError::Contract(Error::Missing)) => {}
            Err(error) => return Err(redact(error)),
        }
    }
    let report = supervisor.prepare_learned_policy_intake(source, clock);
    // Private source binding failures are not actor idempotency conflicts.
    let result = match &report.result {
        Ok(_) => Ok(()),
        Err(JournalError::Contract(Error::Limit | Error::Overflow)) => Err(ActorError::Capacity),
        Err(_) => Err(ActorError::Unavailable),
    };
    *intake = Some(report);
    result
}

fn exchange<P, S, F>(supervisor: &mut FileActorSupervisor<FileOversight>,
    wire: &mut ActorWire<P>, document: &[u8], source: &mut S, mut clock: F)
    -> Result<FileActorExchange, JournalError>
where P: PolicyPort, S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
    supervisor.check_source_wire(wire.request_port().original_port())?;
    let mut intake = None;
    let response = wire.exchange_with_admission(document, |port, request, proposal| {
        prepare(supervisor, port, request, proposal, source, &mut clock, &mut intake)
    });
    Ok(FileActorExchange { response, intake })
}

fn feed<P, S, F>(supervisor: &mut FileActorSupervisor<FileOversight>,
    channel: &mut ActorChannel<P>, bytes: &[u8], source: &mut S, mut clock: F)
    -> Result<FileActorFeed, JournalError>
where P: PolicyPort, S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
    supervisor.check_source_wire(channel.request_port().original_port())?;
    let mut intake = None;
    let feed = channel.feed_with_admission(bytes, |port, request, proposal| {
        prepare(supervisor, port, request, proposal, source, &mut clock, &mut intake)
    });
    Ok(FileActorFeed { feed, intake })
}

impl FileActorSupervisor<FileOversight> {
    /// One original exchange with fresh policy-only intake for a NEW, complete
    /// learned-text request. Malformed input, unavailable output, poll, cancel
    /// and exact/conflicting retries do no file/clock work. A foreign owner
    /// refuses before parsing. Private intake diagnostics never enter response.
    /// Successful preparation and durable submission are distinct operations.
    pub fn exchange_learned_text_actor_from_policy_file<S, F>(&mut self,
        wire: &mut ActorWire<FileLearnedTextActorPort>, document: &[u8], source: &mut S, clock: F)
        -> Result<FileActorExchange, JournalError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        exchange(self, wire, document, source, clock)
    }

    /// Same original framing/backpressure. A fragment, pending write or pending
    /// flush cannot acquire an observation for the next request. The returned
    /// consumed prefix remains authoritative; unconsumed bytes stay caller-owned.
    pub fn feed_learned_text_actor_from_policy_file<S, F>(&mut self,
        channel: &mut ActorChannel<FileLearnedTextActorPort>, bytes: &[u8], source: &mut S, clock: F)
        -> Result<FileActorFeed, JournalError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        feed(self, channel, bytes, source, clock)
    }

    /// Original cumulative stream admission. A premature finish or unresolved
    /// append refuses before source I/O; only original receipt confirmation can
    /// advance the release. No actor bytes become a prefix or a policy snapshot.
    pub fn exchange_learned_text_stream_actor_from_policy_file<S, F>(&mut self,
        wire: &mut ActorWire<FileLearnedTextStreamActorPort>, document: &[u8], source: &mut S, clock: F)
        -> Result<FileActorExchange, JournalError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        exchange(self, wire, document, source, clock)
    }

    /// Same bounded channel and policy-only acquisition for stream releases.
    pub fn feed_learned_text_stream_actor_from_policy_file<S, F>(&mut self,
        channel: &mut ActorChannel<FileLearnedTextStreamActorPort>, bytes: &[u8], source: &mut S, clock: F)
        -> Result<FileActorFeed, JournalError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        feed(self, channel, bytes, source, clock)
    }
}
