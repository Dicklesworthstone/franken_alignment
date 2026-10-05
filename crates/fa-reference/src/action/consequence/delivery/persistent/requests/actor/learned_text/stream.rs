//! Source-only complete-message append and separately reviewed stream finish.
use super::{FileActorPort, FileActorSupervisor, FileActorTicket, FileOversight,
    JournalError, ActorError, ActorOutcome, Knowledge, ElapsedTick, Error, submit_source};

/// Content and frame coordinates are chosen by the original source/stream
/// builder, never by the actor. Finish requires the original confirmed append.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LearnedTextRelease { Message, Finish }

/// Same weak request gateway; no caller message, cumulative frame, audience
/// history, target version, policy snapshot or claimed byte charge is accepted.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileLearnedTextStreamActorPort;
/// fn inject(port: FileLearnedTextStreamActorPort) { port.submit_bytes(b"injected"); }
/// ```
#[derive(Clone, Debug)]
pub struct FileLearnedTextStreamActorPort { pub(super) port: FileActorPort<FileOversight> }
impl FileOversight {
    pub fn into_learned_text_stream_actor_gateway(self)
        -> Result<(FileLearnedTextStreamActorPort, FileActorSupervisor<FileOversight>), JournalError>
    {
        if !self.learned_text_stream_required() { return Err(Error::Binding.into()); }
        let (port, supervisor) = self.into_actor_gateway();
        Ok((FileLearnedTextStreamActorPort { port }, supervisor))
    }
}
impl FileLearnedTextStreamActorPort {
    /// A first request derives the ORIGINAL complete cumulative frame and full
    /// charge. A retry matches its release kind/deadline against the stored
    /// frame, even when today's audience has advanced or recovery paused source.
    /// Unknown append/finish outcomes can be observed, never converted to resend.
    pub fn submit(&self, request: u64, release: LearnedTextRelease, deadline: ElapsedTick)
        -> Result<FileActorTicket<FileOversight>, ActorError>
    {
        submit_source(&self.port, request, |host|
            host.prepare_learned_text_stream_request(request, release, deadline))
    }
    pub fn poll(&self, ticket: &FileActorTicket<FileOversight>) -> Knowledge<ActorOutcome> {
        self.port.poll(ticket)
    }
    pub fn cancel(&self, ticket: &FileActorTicket<FileOversight>) -> Result<(), ActorError> {
        self.port.cancel(ticket)
    }
}
