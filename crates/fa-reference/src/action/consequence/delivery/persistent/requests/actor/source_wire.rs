//! Source acquisition at the original decoded actor submission boundary.
//! Parsing, tickets, idempotency and redaction stay in the original wire/port.
use super::{ActorError, Error, FileActorPort, FileActorSupervisor, JournalError, Rc, Weak};
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::observed::FileOversight;
use crate::action::consequence::delivery::persistent::observed::driver::FileSupervisedDriver;
use crate::action::consequence::delivery::persistent::observed::driver::evidence::FileEvidenceReport;
use crate::action::consequence::oversight::actor_wire::{ActorChannel, ActorWire, FeedResult, WireResponse};
use crate::action::consequence::oversight::evidence_source::{EvidenceFile, EvidenceIdentity};

type Port = FileActorPort<FileOversight>;

/// Only response is actor-visible. Intake records source PREPARATION, not whether
/// the ensuing distinct durable submission committed. None means no preparation
/// was needed/attempted, not that an effect was accepted or evidence is fresh.
///
/// ```compile_fail,E0624
/// use fa_reference::action::consequence::oversight::actor_wire::ActorWire;
/// fn override_admission(wire: &mut ActorWire) {
///     wire.exchange_with_admission(b"{}", |_, _, _| Ok(()));
/// }
/// ```
#[derive(Debug)]
pub struct FileActorExchange {
    pub response: WireResponse,
    pub intake: Option<FileEvidenceReport<EvidenceIdentity>>,
}

/// Feed the consumed prefix to the original transport accounting. Pending output
/// still drains through ActorChannel; source diagnostics never enter its bytes.
#[derive(Debug)]
pub struct FileActorFeed {
    pub feed: FeedResult,
    pub intake: Option<FileEvidenceReport<EvidenceIdentity>>,
}

impl FileActorSupervisor<FileOversight> {
    fn check_source_wire(&self, port: &Port) -> Result<(), JournalError> {
        if !Weak::ptr_eq(&port.owner, &Rc::downgrade(&self.owner)) { return Err(Error::Binding.into()); }
        Ok(())
    }

    fn prepare_wire_submission<S, F>(&mut self, request: u64, source: &mut S, clock: &mut F,
        intake: &mut Option<FileEvidenceReport<EvidenceIdentity>>) -> Result<(), ActorError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let status = self.host().and_then(|host| host.request_status(request));
        let report = match status {
            // Exact bytes are STILL checked by the original submit_request.
            // This includes conflicting retries and recorded admission refusals.
            Ok(_) => return Ok(()),
            Err(JournalError::Contract(Error::Missing)) => self.prepare_file_intake(source, clock),
            Err(error) => FileEvidenceReport { observations: Vec::new(), source_updates: Vec::new(), result: Err(error) },
        };
        // Do not mislabel a source Binding failure as actor idempotency conflict,
        // or serialize private paths, producer IDs, policy values or I/O details.
        let result = match &report.result {
            Ok(_) => Ok(()),
            Err(JournalError::Contract(Error::Limit | Error::Overflow)) => Err(ActorError::Capacity),
            Err(_) => Err(ActorError::Unavailable),
        };
        *intake = Some(report);
        result
    }

    /// One original bounded JSON exchange. Only a valid NEW submit refreshes the
    /// mandatory source and consumes its one-use slot, in this synchronous call.
    /// Poll/cancel, exact/conflicting retries and malformed documents do not read
    /// a source or sample time. A foreign gateway refuses before any operation.
    /// Source and submission are separate journal transactions, never rolled back
    /// together. The original port returns the actual submission disposition.
    pub fn exchange_actor_from_file<S, F>(&mut self, wire: &mut ActorWire<Port>,
        document: &[u8], source: &mut S, mut clock: F) -> Result<FileActorExchange, JournalError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.check_source_wire(wire.request_port())?;
        let mut intake = None;
        let response = wire.exchange_with_admission(document, |_, request, _| {
            self.prepare_wire_submission(request, source, &mut clock, &mut intake)
        });
        Ok(FileActorExchange { response, intake })
    }

    /// Original newline framing, one-response backpressure and exchange limit.
    /// A partial frame cannot consume a source read; pending write/flush prevents
    /// the NEXT frame from acquiring another snapshot. Never pre-read per socket
    /// packet, since packets can contain fragments, retries or multiple commands.
    pub fn feed_actor_from_file<S, F>(&mut self, channel: &mut ActorChannel<Port>,
        bytes: &[u8], source: &mut S, mut clock: F) -> Result<FileActorFeed, JournalError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.check_source_wire(channel.request_port())?;
        let mut intake = None;
        let feed = channel.feed_with_admission(bytes, |_, request, _| {
            self.prepare_wire_submission(request, source, &mut clock, &mut intake)
        });
        Ok(FileActorFeed { feed, intake })
    }
}

impl FileSupervisedDriver {
    pub fn exchange_actor_from_file<S, F>(&mut self, wire: &mut ActorWire<Port>,
        document: &[u8], source: &mut S, clock: F) -> Result<FileActorExchange, JournalError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let result = self.supervisor_mut().exchange_actor_from_file(wire, document, source, clock);
        self.reap_helpers();
        result
    }
    pub fn feed_actor_from_file<S, F>(&mut self, channel: &mut ActorChannel<Port>,
        bytes: &[u8], source: &mut S, clock: F) -> Result<FileActorFeed, JournalError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let result = self.supervisor_mut().feed_actor_from_file(channel, bytes, source, clock);
        self.reap_helpers();
        result
    }
}
