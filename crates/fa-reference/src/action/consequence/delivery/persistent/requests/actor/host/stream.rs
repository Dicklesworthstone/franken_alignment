//! Actor message intents feed the SAME durable request owner and outcome port.
use super::super::{ActorError, FileActorPort, FileActorTicket, redact};
use super::super::super::RequestBook;
use crate::action::ActionSpec;
use crate::action::consequence::delivery::persistent::observed::{FileOversight, stream::FileStreamProposal};
use crate::action::consequence::delivery::stream::MAX_MESSAGE_BYTES;

impl RequestBook {
    // Private original data, including admissions that did not create an action.
    // Only the existing owner can frame an exact actor retry from this record.
    pub(in crate::action::consequence::delivery::persistent) fn original_spec(&self, request: u64) -> Option<&ActionSpec> {
        self.records.get(&request).map(|record| &record.spec)
    }
}

impl FileActorPort<FileOversight> {
    /// Submit one complete message or explicit finish without access to the host,
    /// private evidence, helpers, human reviewer, clock or publication endpoint.
    /// The host supplies cumulative context; the intent pins its target and epoch.
    /// Exact retries use original frames, not today's prefix, and need no new
    /// admission snapshot. Poll and cancel use the unchanged restricted port.
    ///
    /// This is the in-process actor API, not an added wire verb or a transport.
    ///
    /// ```compile_fail,E0599
    /// use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
    /// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileActorPort;
    /// fn bypass(port: FileActorPort<FileOversight>) { port.publish(1); }
    /// ```
    pub fn submit_stream(&self, request: u64, intent: &FileStreamProposal)
        -> Result<FileActorTicket<FileOversight>, ActorError>
    {
        if request == 0 || intent.message.as_deref() == Some("") { return Err(ActorError::MalformedProposal); }
        if intent.message.as_ref().is_some_and(|message| message.len() > MAX_MESSAGE_BYTES) {
            return Err(ActorError::Capacity);
        }
        let owner = self.owner.upgrade().ok_or(ActorError::Unavailable)?;
        let proposal = {
            let state = owner.try_borrow().map_err(|_| ActorError::Unavailable)?;
            state.host.stream_actor_proposal(request, intent).map_err(redact)?
        };
        // No callback, await or external work can interleave these operations on
        // this Rc-owned gateway. Release the read borrow before original submit.
        self.submit(request, &proposal)
    }
}
