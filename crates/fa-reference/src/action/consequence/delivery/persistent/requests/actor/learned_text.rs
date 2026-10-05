//! Source-only intake for the ORIGINAL durable learned-text generator.
//! Request keys name the first completed message admitted under that key, not
//! a fresh read on every retry. No actor-supplied output or source is accepted.
use super::{FileActorPort, FileActorSupervisor, FileActorTicket, redact};
use super::super::super::{JournalError, observed::FileOversight};
use crate::action::{ActionSpec, ElapsedTick, ResolvedTarget};
use crate::action::consequence::oversight::actor::{ActorError, ActorOutcome, Knowledge};
use crate::{Error, Snapshot};

mod stream;
pub use stream::{FileLearnedTextStreamActorPort, LearnedTextRelease};

/// Destination and bounds only. The original completed generator supplies the
/// entire payload, and the supervisor supplies the single-use policy snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LearnedTextProposal {
    pub target: ResolvedTarget,
    pub expected_policy_epoch: u64,
    pub deadline: ElapsedTick,
    pub units: u64,
}

/// The same weak actor gateway, without any byte-bearing submission method.
/// It cannot keep an abandoned authority lock alive, observe private numerical
/// state, supply a snapshot, or approve/review/dispatch its own request.
///
/// ```compile_fail,E0609
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::LearnedTextProposal;
/// fn inject(proposal: &mut LearnedTextProposal) { proposal.payload = b"injected".to_vec(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileLearnedTextActorPort;
/// fn escape(port: FileLearnedTextActorPort) { port.host_mut(); }
/// ```
#[derive(Clone, Debug)]
pub struct FileLearnedTextActorPort { port: FileActorPort<FileOversight> }

impl FileOversight {
    /// Adopt this EXACT durable owner. Recovered owners remain fenced and their
    /// generators remain paused. Raw text and framed streams are distinct routes.
    pub fn into_learned_text_actor_gateway(self)
        -> Result<(FileLearnedTextActorPort, FileActorSupervisor<FileOversight>), JournalError>
    {
        if !self.learned_text_required() || self.learned_text_stream_required() {
            return Err(Error::Binding.into());
        }
        let (port, supervisor) = self.into_actor_gateway();
        Ok((FileLearnedTextActorPort { port }, supervisor))
    }
}

impl FileLearnedTextActorPort {
    /// First submission captures the current complete original message once.
    /// Exact retries resolve the ORIGINAL request even after cancellation,
    /// policy change, source loss or recovery; changed destination/bounds conflict.
    /// No inference, new observation or fresh admission is performed on a retry.
    pub fn submit(&self, request: u64, proposal: LearnedTextProposal)
        -> Result<FileActorTicket<FileOversight>, ActorError>
    {
        submit_source(&self.port, request, |host| host.prepare_learned_text_request(request, proposal))
    }

    pub fn poll(&self, ticket: &FileActorTicket<FileOversight>) -> Knowledge<ActorOutcome> {
        self.port.poll(ticket)
    }
    pub fn cancel(&self, ticket: &FileActorTicket<FileOversight>) -> Result<(), ActorError> {
        self.port.cancel(ticket)
    }
}

// Only the two concrete source adapters call this; no public callback can
// supply output. Holding the SAME owner borrow makes derivation and the original
// request transaction indivisible relative to actor/supervisor mutations.
fn submit_source<F>(port: &FileActorPort<FileOversight>, request: u64, derive: F)
    -> Result<FileActorTicket<FileOversight>, ActorError>
where F: FnOnce(&FileOversight) -> Result<ActionSpec, JournalError> {
    if request == 0 { return Err(ActorError::MalformedProposal); }
    let owner = port.owner.upgrade().ok_or(ActorError::Unavailable)?;
    let mut state = owner.try_borrow_mut().map_err(|_| ActorError::Unavailable)?;
    let existing = match state.host.request_status(request) {
        Ok(_) => true,
        Err(JournalError::Contract(Error::Missing)) => false,
        Err(_) => return Err(ActorError::Unavailable),
    };
    if !existing && state.snapshot.is_none() { return Err(ActorError::Unavailable); }
    // Resolve under the SAME owner borrow. Do not consume the observation
    // for a malformed request or unavailable/incomplete numerical source.
    let spec = derive(&state.host).map_err(redact)?;
    let snapshot = if existing { Snapshot::default() }
        else { state.snapshot.take().ok_or(ActorError::Unavailable)? };
    let revision = state.host.revision();
    state.host.submit_request(revision, request, spec, snapshot).map_err(redact)?;
    Ok(FileActorTicket { owner: port.owner.clone(), request })
}
