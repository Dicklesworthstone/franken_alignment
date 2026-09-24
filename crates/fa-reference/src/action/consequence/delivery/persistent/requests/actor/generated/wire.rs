//! Fixed-size source references inside the ORIGINAL actor Submit envelope.
//! Framing, ticket custody, response redaction and Unix I/O stay unchanged.
use super::{ActorError, ActorOutcome, ElapsedTick, FileActorTicket, FileGeneratedTextActorPort,
    FileOversight, FileTextMessageRequest, Knowledge, ResolvedTarget, redact};
use crate::action::consequence::oversight::actor::ActorProposal;
use crate::action::consequence::oversight::actor_wire::{ActorRequestPort, backend};

const DOMAIN: &[u8; 8] = b"FAGREF\0\x01";

impl FileGeneratedTextActorPort {
    /// Eight domain bytes and three big-endian u64s: request, generation,
    /// generation revision. Generation zero is reserved for explicit finish,
    /// whose revision MUST also be zero. No variable output bytes are carried.
    pub const INTENT_BYTES: usize = 32;

    /// Encode an intent for the unchanged actor-wire Submit command. Its outer
    /// request ID must equal request.request. This is only bounded input data,
    /// not proof that the named generation exists or has a publishable result.
    /// The original command codec validates the outer target/deadline structure.
    /// `units` is fixed to the INTENT size; the actual effect is still charged
    /// for its entire native-derived cumulative frame by the original broker.
    pub fn encode_message(request: &FileTextMessageRequest) -> Result<ActorProposal, ActorError> {
        request.check().map_err(|error| redact(error.into()))?;
        Ok(encode(request.request, request.generation, request.generation_revision,
            request.target, request.policy_epoch, request.deadline))
    }

    /// Encode explicit finish, not an empty message or a cancelled generation.
    /// The original stream builder, review and two-key pipeline still decide
    /// admission/publication. The actor supplies no prior-prefix or message bytes.
    pub fn encode_finish(request: u64, target: ResolvedTarget, policy_epoch: u64,
        deadline: ElapsedTick) -> Result<ActorProposal, ActorError>
    {
        if request == 0 { return Err(ActorError::MalformedProposal); }
        Ok(encode(request, 0, 0, target, policy_epoch, deadline))
    }
}

fn encode(request: u64, generation: u64, revision: u64, target: ResolvedTarget,
    policy_epoch: u64, deadline: ElapsedTick) -> ActorProposal
{
    let mut payload = Vec::with_capacity(FileGeneratedTextActorPort::INTENT_BYTES);
    payload.extend_from_slice(DOMAIN);
    for value in [request, generation, revision] { payload.extend_from_slice(&value.to_be_bytes()); }
    ActorProposal { target, payload, expected_policy_epoch: policy_epoch, deadline,
        units: FileGeneratedTextActorPort::INTENT_BYTES as u64 }
}

// Validate the entire fixed shape BEFORE gateway lookup or snapshot consumption.
// The outer JSON parser remains the original duplicate-key/bounds authority.
fn decode(request: u64, proposal: &ActorProposal) -> Result<Option<FileTextMessageRequest>, ActorError> {
    if request == 0 || proposal.payload.len() != FileGeneratedTextActorPort::INTENT_BYTES
        || proposal.units != FileGeneratedTextActorPort::INTENT_BYTES as u64
        || proposal.payload.get(..DOMAIN.len()) != Some(DOMAIN.as_slice())
    { return Err(ActorError::MalformedProposal); }
    let word = |start: usize| -> Result<u64, ActorError> {
        Ok(u64::from_be_bytes(proposal.payload[start..start + 8].try_into()
            .map_err(|_| ActorError::MalformedProposal)?))
    };
    if word(8)? != request { return Err(ActorError::MalformedProposal); }
    let generation = word(16)?;
    let generation_revision = word(24)?;
    if generation == 0 {
        return if generation_revision == 0 { Ok(None) } else { Err(ActorError::MalformedProposal) };
    }
    let source = FileTextMessageRequest { request, generation, generation_revision,
        target: proposal.target, policy_epoch: proposal.expected_policy_epoch, deadline: proposal.deadline };
    source.check().map_err(|error| redact(error.into()))?;
    Ok(Some(source))
}

impl backend::Sealed for FileGeneratedTextActorPort {}
impl ActorRequestPort for FileGeneratedTextActorPort {
    type Ticket = FileActorTicket<FileOversight>;
    fn submit(&self, request: u64, proposal: &ActorProposal) -> Result<Self::Ticket, ActorError> {
        match decode(request, proposal)? {
            Some(source) => FileGeneratedTextActorPort::submit(self, &source),
            None => self.finish(request, proposal.target, proposal.expected_policy_epoch, proposal.deadline),
        }
    }
    fn poll(&self, ticket: &Self::Ticket) -> Knowledge<ActorOutcome> {
        FileGeneratedTextActorPort::poll(self, ticket)
    }
    fn cancel(&self, ticket: &Self::Ticket) -> Result<(), ActorError> {
        FileGeneratedTextActorPort::cancel(self, ticket)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::consequence::activation::monitor::decoder::sampled::generation::MAX_GENERATION_TOKENS;

    fn source() -> FileTextMessageRequest {
        FileTextMessageRequest { request: 0x0102_0304_0506_0708, generation: 0x1112_1314_1516_1718,
            generation_revision: 9, target: ResolvedTarget { adapter: 10, object: 11,
                contract_version: 1, expected_version: 2, generation: 3 },
            policy_epoch: 0, deadline: ElapsedTick(100) }
    }

    #[test]
    fn generated_actor_wire_matches_independent_golden_source_and_finish_bytes() {
        let source = source(); let encoded = FileGeneratedTextActorPort::encode_message(&source).unwrap();
        let golden = [70,65,71,82,69,70,0,1, 1,2,3,4,5,6,7,8,
            17,18,19,20,21,22,23,24, 0,0,0,0,0,0,0,9];
        assert_eq!(encoded.payload, golden); assert_eq!(encoded.units, 32);
        assert_eq!(decode(source.request, &encoded).unwrap(), Some(source.clone()));
        let finish = FileGeneratedTextActorPort::encode_finish(source.request, source.target,
            source.policy_epoch, source.deadline).unwrap();
        let mut expected = golden; expected[16..].fill(0);
        assert_eq!(finish.payload, expected); assert_eq!(decode(source.request, &finish).unwrap(), None);
        assert_eq!(finish.target, encoded.target); assert_eq!(finish.deadline, encoded.deadline);
        assert_eq!(finish.expected_policy_epoch, encoded.expected_policy_epoch);
    }

    #[test]
    fn generated_actor_wire_rejects_truncation_suffixes_mismatched_keys_and_noncanonical_finish() {
        let source = source(); let encoded = FileGeneratedTextActorPort::encode_message(&source).unwrap();
        for end in 0..32 {
            let mut bad = encoded.clone(); bad.payload.truncate(end);
            assert_eq!(decode(source.request, &bad), Err(ActorError::MalformedProposal));
        }
        for which in 0..5 {
            let mut bad = encoded.clone();
            match which { 0 => bad.payload.push(0), 1 => bad.payload[0] ^= 1,
                2 => bad.payload[8] ^= 1, 3 => bad.units += 1, _ => bad.payload[16..24].fill(0) }
            assert_eq!(decode(source.request, &bad), Err(ActorError::MalformedProposal));
        }
        assert_eq!(decode(0, &encoded), Err(ActorError::MalformedProposal));
        assert_eq!(decode(source.request + 1, &encoded), Err(ActorError::MalformedProposal));
        assert_eq!(decode(source.request, &encoded).unwrap(), Some(source));
    }

    #[test]
    fn generated_actor_wire_uses_original_generation_bounds_without_truncating_identifiers() {
        let mut source = source(); source.request = u64::MAX; source.generation = u64::MAX;
        source.generation_revision = MAX_GENERATION_TOKENS as u64;
        let encoded = FileGeneratedTextActorPort::encode_message(&source).unwrap();
        assert_eq!(decode(u64::MAX, &encoded).unwrap(), Some(source.clone()));
        source.generation_revision += 1;
        assert_eq!(FileGeneratedTextActorPort::encode_message(&source), Err(ActorError::Capacity));
        let mut bad = encoded; bad.payload[24..].copy_from_slice(&source.generation_revision.to_be_bytes());
        assert_eq!(decode(u64::MAX, &bad), Err(ActorError::Capacity));
        source.generation_revision = 0; source.generation = 0;
        assert_eq!(FileGeneratedTextActorPort::encode_message(&source), Err(ActorError::MalformedProposal));
        assert!(FileGeneratedTextActorPort::encode_finish(u64::MAX, source.target,
            source.policy_epoch, source.deadline).is_ok());
    }
}
