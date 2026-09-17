//! Bounded binary intent carried in the existing version-one payload_hex field.
//! No extra JSON parser, ticket book, review frame or journal format is added.
use super::{ActorError, ActorProposal, FileStreamProposal, StreamProfile};
use crate::action::{ElapsedTick, ResolvedTarget};

const DOMAIN: &[u8; 8] = b"FASINT\0\x01";
pub const STREAM_INTENT_HEADER_BYTES: usize = 41;

/// Encode only actor-controlled intent. Transport `units` is exactly its byte
/// length and is NOT an effect reservation; the native stream builder derives
/// and charges the full cumulative release frame at admission. None means an
/// explicit finish. Empty messages, empty payloads and EOF do not mean finish.
pub fn encode_stream_proposal(profile: StreamProfile, intent: &FileStreamProposal)
    -> Result<ActorProposal, ActorError>
{
    routing(intent.target, intent.deadline)?;
    if intent.message.as_deref() == Some("") { return Err(ActorError::MalformedProposal); }
    let added = intent.message.as_ref().map_or(0, String::len);
    if added > profile.max_message_bytes() { return Err(ActorError::Capacity); }
    let mut payload = Vec::new();
    payload.try_reserve_exact(STREAM_INTENT_HEADER_BYTES + added).map_err(|_| ActorError::Capacity)?;
    payload.extend_from_slice(DOMAIN);
    for value in [profile.id(), profile.generation()] { payload.extend_from_slice(&value.to_be_bytes()); }
    for value in [profile.max_messages(), profile.max_message_bytes(), profile.max_stream_bytes()] {
        payload.extend_from_slice(&(value as u32).to_be_bytes());
    }
    payload.push(u8::from(intent.message.is_none()));
    payload.extend_from_slice(&(added as u32).to_be_bytes());
    if let Some(message) = &intent.message { payload.extend_from_slice(message.as_bytes()); }
    Ok(ActorProposal { target: intent.target, units: payload.len() as u64, payload,
        expected_policy_epoch: intent.expected_policy_epoch, deadline: intent.deadline })
}

pub(super) fn message(profile: StreamProfile, proposal: &ActorProposal) -> Result<Option<&str>, ActorError> {
    routing(proposal.target, proposal.deadline)?;
    let bytes = &proposal.payload;
    if bytes.len() > STREAM_INTENT_HEADER_BYTES + profile.max_message_bytes() {
        return Err(ActorError::Capacity);
    }
    if bytes.len() < STREAM_INTENT_HEADER_BYTES || proposal.units != bytes.len() as u64
        || &bytes[..8] != DOMAIN { return Err(ActorError::MalformedProposal); }
    let mut offset = 8;
    for value in [profile.id(), profile.generation()] {
        if bytes[offset..offset + 8] != value.to_be_bytes() { return Err(ActorError::MalformedProposal); }
        offset += 8;
    }
    for value in [profile.max_messages(), profile.max_message_bytes(), profile.max_stream_bytes()] {
        if bytes[offset..offset + 4] != (value as u32).to_be_bytes() { return Err(ActorError::MalformedProposal); }
        offset += 4;
    }
    let message = &bytes[STREAM_INTENT_HEADER_BYTES..];
    if bytes[37..41] != (message.len() as u32).to_be_bytes() { return Err(ActorError::MalformedProposal); }
    match bytes[36] {
        0 if !message.is_empty() => std::str::from_utf8(message).map(Some).map_err(|_| ActorError::MalformedProposal),
        1 if message.is_empty() => Ok(None),
        _ => Err(ActorError::MalformedProposal),
    }
}

fn routing(target: ResolvedTarget, deadline: ElapsedTick) -> Result<(), ActorError> {
    if [target.adapter, target.object, target.contract_version, target.expected_version,
        target.generation, deadline.0].contains(&0) { return Err(ActorError::MalformedProposal); }
    Ok(())
}

#[cfg(test)]
mod tests;
