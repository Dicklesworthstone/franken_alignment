//! Source-only stream releases through the SAME actor protocol and request book.
use super::{ActorError, ActorOutcome, ActorProposal, ActorRequestPort, ElapsedTick,
    FileActorTicket, FileLearnedTextStreamActorPort, FileOversight, Knowledge,
    LearnedTextRelease, ResolvedTarget, backend, check_intent};

const DOMAIN: &[u8; 8] = b"FALREL\0\x01";
// A canonical ENVELOPE marker, never the effect target. The existing Submit
// codec requires these fields; the original stream builder derives actual
// scope/target/version/epoch. Reject all variations rather than silently ignore
// actor-selected values. The trusted port, not this marker, selects the route.
const MARKER: ResolvedTarget = ResolvedTarget { adapter: 1, object: 1,
    contract_version: 1, expected_version: 1, generation: 1 };

impl FileLearnedTextStreamActorPort {
    /// Eight domain bytes, eight request bytes and one Message/Finish tag.
    pub const INTENT_BYTES: usize = 17;

    /// Encode ONLY a release kind and deadline under the original request key.
    /// All other envelope fields are canonical placeholders, not actor choices.
    /// The original stream builder charges the entire cumulative frame, not the
    /// smaller intent; finish still requires a receipt-confirmed generated append.
    pub fn encode_release(request: u64, release: LearnedTextRelease, deadline: ElapsedTick)
        -> Result<ActorProposal, ActorError>
    {
        if request == 0 { return Err(ActorError::MalformedProposal); }
        let mut payload = Vec::with_capacity(Self::INTENT_BYTES);
        payload.extend_from_slice(DOMAIN);
        payload.extend_from_slice(&request.to_be_bytes());
        payload.push(match release { LearnedTextRelease::Message => 0, LearnedTextRelease::Finish => 1 });
        Ok(ActorProposal { target: MARKER, payload, expected_policy_epoch: 0,
            deadline, units: Self::INTENT_BYTES as u64 })
    }
}

pub(super) fn decode(request: u64, proposal: &ActorProposal) -> Result<LearnedTextRelease, ActorError> {
    check_intent(request, proposal, DOMAIN, FileLearnedTextStreamActorPort::INTENT_BYTES)?;
    if proposal.target != MARKER || proposal.expected_policy_epoch != 0 {
        return Err(ActorError::MalformedProposal);
    }
    match proposal.payload[16] {
        0 => Ok(LearnedTextRelease::Message),
        1 => Ok(LearnedTextRelease::Finish),
        _ => Err(ActorError::MalformedProposal),
    }
}

impl backend::Sealed for FileLearnedTextStreamActorPort {}
impl ActorRequestPort for FileLearnedTextStreamActorPort {
    type Ticket = FileActorTicket<FileOversight>;
    fn submit(&self, request: u64, proposal: &ActorProposal) -> Result<Self::Ticket, ActorError> {
        FileLearnedTextStreamActorPort::submit(self, request, decode(request, proposal)?, proposal.deadline)
    }
    fn poll(&self, ticket: &Self::Ticket) -> Knowledge<ActorOutcome> {
        FileLearnedTextStreamActorPort::poll(self, ticket)
    }
    fn cancel(&self, ticket: &Self::Ticket) -> Result<(), ActorError> {
        FileLearnedTextStreamActorPort::cancel(self, ticket)
    }
}
