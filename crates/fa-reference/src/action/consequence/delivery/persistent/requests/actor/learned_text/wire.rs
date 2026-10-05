//! Source-only learned output on the ORIGINAL bounded actor wire and transports.
//! These fixed intents are never passed to the publication broker as payloads.
mod stream;
#[cfg(test)]
mod tests;

use super::{ActorError, ActorOutcome, FileActorTicket, FileLearnedTextActorPort,
    FileLearnedTextStreamActorPort, FileOversight, Knowledge, LearnedTextProposal,
    LearnedTextRelease, ElapsedTick, ResolvedTarget};
use crate::action::consequence::oversight::actor::ActorProposal;
use crate::action::consequence::oversight::actor_wire::{ActorRequestPort, backend};

const DOMAIN: &[u8; 8] = b"FALTXT\0\x01";

impl FileLearnedTextActorPort {
    /// Domain, original request key and effect units, all full-width. The outer
    /// wire units cover this intent only; they never become the effect's charge.
    pub const INTENT_BYTES: usize = 24;

    /// Put a source-only request into the unchanged ActorWire Submit envelope.
    /// Target, epoch and deadline keep their original meanings. The original
    /// generator supplies the payload, including when it is shorter than this
    /// intent. No source identity, output, evidence or verdict is accepted here.
    pub fn encode_request(request: u64, proposal: LearnedTextProposal)
        -> Result<ActorProposal, ActorError>
    {
        if request == 0 { return Err(ActorError::MalformedProposal); }
        let mut payload = Vec::with_capacity(Self::INTENT_BYTES);
        payload.extend_from_slice(DOMAIN);
        payload.extend_from_slice(&request.to_be_bytes());
        payload.extend_from_slice(&proposal.units.to_be_bytes());
        Ok(ActorProposal { target: proposal.target, payload,
            expected_policy_epoch: proposal.expected_policy_epoch, deadline: proposal.deadline,
            units: Self::INTENT_BYTES as u64 })
    }
}

// Fixed shape and outer/inner request binding are checked BEFORE borrowing the
// authority or consuming a snapshot. A suffix is not an optional output field.
fn check_intent(request: u64, proposal: &ActorProposal, domain: &[u8; 8], size: usize)
    -> Result<(), ActorError>
{
    if request == 0 || proposal.payload.len() != size || proposal.units != size as u64
        || proposal.payload.get(..8) != Some(domain.as_slice())
        || proposal.payload.get(8..16) != Some(request.to_be_bytes().as_slice())
    { return Err(ActorError::MalformedProposal); }
    Ok(())
}

fn decode(request: u64, proposal: &ActorProposal) -> Result<LearnedTextProposal, ActorError> {
    check_intent(request, proposal, DOMAIN, FileLearnedTextActorPort::INTENT_BYTES)?;
    let units = u64::from_be_bytes(proposal.payload[16..24].try_into()
        .map_err(|_| ActorError::MalformedProposal)?);
    Ok(LearnedTextProposal { target: proposal.target,
        expected_policy_epoch: proposal.expected_policy_epoch, deadline: proposal.deadline, units })
}

impl backend::Sealed for FileLearnedTextActorPort {}
impl ActorRequestPort for FileLearnedTextActorPort {
    type Ticket = FileActorTicket<FileOversight>;
    fn submit(&self, request: u64, proposal: &ActorProposal) -> Result<Self::Ticket, ActorError> {
        FileLearnedTextActorPort::submit(self, request, decode(request, proposal)?)
    }
    fn poll(&self, ticket: &Self::Ticket) -> Knowledge<ActorOutcome> {
        FileLearnedTextActorPort::poll(self, ticket)
    }
    fn cancel(&self, ticket: &Self::Ticket) -> Result<(), ActorError> {
        FileLearnedTextActorPort::cancel(self, ticket)
    }
}
