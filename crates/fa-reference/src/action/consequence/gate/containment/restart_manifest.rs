//! Versioned lossless serialization for the reference actor restart state.
//!
//! The manifest contains actor data only: restart-profile identity, original
//! tokens, opaque cache bytes, opaque sampler bytes and next position. It never
//! contains permits, rights, reservations, policy state or external-effect
//! outcomes. Parsing establishes framing and internal consistency, not origin
//! authenticity, durability or successful restoration into a serving backend.

use super::{ActorState, RestartGrade, RestartProfile, MAX_CACHE_BYTES, MAX_SAMPLER_BYTES, MAX_TOKENS};
use crate::Error;

const DOMAIN: &[u8; 8] = b"FARSTRT\x01";
pub const RESTART_MANIFEST_HEADER_BYTES: usize = 84;
pub const MAX_RESTART_MANIFEST_BYTES: usize = RESTART_MANIFEST_HEADER_BYTES
    + MAX_TOKENS * 4 + MAX_CACHE_BYTES + MAX_SAMPLER_BYTES;
const COMPONENT_TOKENS: u8 = 1;
const COMPONENT_CACHE: u8 = 2;
const COMPONENT_SAMPLER: u8 = 4;
const COMPONENT_POSITION: u8 = 8;
const REQUIRED_COMPONENTS: u8 = COMPONENT_TOKENS | COMPONENT_CACHE | COMPONENT_SAMPLER | COMPONENT_POSITION;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestartManifest {
    actor: ActorState,
}

impl RestartManifest {
    pub fn capture(actor: &ActorState) -> Result<Self, Error> {
        // Re-run the public constructor so a future ActorState extension cannot
        // silently enter this format without updating its completeness contract.
        let actor = ActorState::new(
            actor.profile(), actor.tokens().to_vec(), actor.cache().to_vec(),
            actor.sampler().to_vec(), actor.next_position(),
        )?;
        Ok(Self { actor })
    }

    pub fn actor(&self) -> &ActorState { &self.actor }

    pub fn encoded_len(&self) -> Result<usize, Error> {
        RESTART_MANIFEST_HEADER_BYTES
            .checked_add(self.actor.tokens().len().checked_mul(4).ok_or(Error::Overflow)?)
            .and_then(|n| n.checked_add(self.actor.cache().len()))
            .and_then(|n| n.checked_add(self.actor.sampler().len()))
            .filter(|n| *n <= MAX_RESTART_MANIFEST_BYTES)
            .ok_or(Error::Limit)
    }

    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        let length = self.encoded_len()?;
        let mut bytes = Vec::with_capacity(length);
        bytes.extend_from_slice(DOMAIN);
        bytes.push(REQUIRED_COMPONENTS);
        bytes.extend_from_slice(&[0; 3]);
        let profile = self.actor.profile();
        for value in [profile.id, profile.generation, profile.host_generation,
            profile.model_generation, profile.tokenizer_generation, profile.state_schema_generation,
            self.actor.next_position()]
        { bytes.extend_from_slice(&value.to_be_bytes()); }
        bytes.push(match profile.grade {
            RestartGrade::AuditOnly => 0,
            RestartGrade::FunctionalRestart => 1,
            RestartGrade::ExactRestart => 2,
        });
        bytes.extend_from_slice(&[0; 3]);
        put_len(&mut bytes, self.actor.tokens().len())?;
        put_len(&mut bytes, self.actor.cache().len())?;
        put_len(&mut bytes, self.actor.sampler().len())?;
        for token in self.actor.tokens() { bytes.extend_from_slice(&token.to_be_bytes()); }
        bytes.extend_from_slice(self.actor.cache());
        bytes.extend_from_slice(self.actor.sampler());
        if bytes.len() != length { return Err(Error::Binding); }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_RESTART_MANIFEST_BYTES { return Err(Error::Limit); }
        if bytes.len() < RESTART_MANIFEST_HEADER_BYTES { return Err(Error::Incomplete); }
        let mut reader = Reader { bytes, cursor: 0 };
        if reader.take(8)? != DOMAIN { return Err(Error::InvalidInput); }
        if reader.byte()? != REQUIRED_COMPONENTS { return Err(Error::Incomplete); }
        if reader.take(3)? != [0, 0, 0] { return Err(Error::InvalidInput); }
        let profile = RestartProfile {
            id: reader.u64()?, generation: reader.u64()?, host_generation: reader.u64()?,
            model_generation: reader.u64()?, tokenizer_generation: reader.u64()?,
            state_schema_generation: reader.u64()?, grade: RestartGrade::AuditOnly,
        };
        let next_position = reader.u64()?;
        let grade = match reader.byte()? {
            0 => RestartGrade::AuditOnly,
            1 => RestartGrade::FunctionalRestart,
            2 => RestartGrade::ExactRestart,
            _ => return Err(Error::InvalidInput),
        };
        if reader.take(3)? != [0, 0, 0] { return Err(Error::InvalidInput); }
        let token_count = reader.len32()?;
        let cache_len = reader.len32()?;
        let sampler_len = reader.len32()?;
        if token_count > MAX_TOKENS || cache_len > MAX_CACHE_BYTES || sampler_len > MAX_SAMPLER_BYTES {
            return Err(Error::Limit);
        }
        let payload = token_count.checked_mul(4).and_then(|n| n.checked_add(cache_len))
            .and_then(|n| n.checked_add(sampler_len)).ok_or(Error::Overflow)?;
        if reader.cursor.checked_add(payload).ok_or(Error::Overflow)? != bytes.len() {
            return Err(Error::InvalidInput);
        }
        let mut tokens = Vec::new();
        tokens.try_reserve_exact(token_count).map_err(|_| Error::Limit)?;
        for _ in 0..token_count { tokens.push(reader.u32()?); }
        let cache = reader.take(cache_len)?.to_vec();
        let sampler = reader.take(sampler_len)?.to_vec();
        if reader.cursor != bytes.len() { return Err(Error::InvalidInput); }
        let actor = ActorState::new(RestartProfile { grade, ..profile }, tokens, cache, sampler, next_position)?;
        Ok(Self { actor })
    }

    /// Decode and require the exact registered restart profile. Equal dimensions
    /// never substitute a different model/host/tokenizer/schema generation.
    pub fn decode_for(bytes: &[u8], expected: RestartProfile) -> Result<Self, Error> {
        let manifest = Self::decode(bytes)?;
        if manifest.actor.profile() != expected { return Err(Error::Binding); }
        Ok(manifest)
    }
}

fn put_len(bytes: &mut Vec<u8>, value: usize) -> Result<(), Error> {
    bytes.extend_from_slice(&u32::try_from(value).map_err(|_| Error::Limit)?.to_be_bytes());
    Ok(())
}

struct Reader<'a> { bytes: &'a [u8], cursor: usize }
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let end = self.cursor.checked_add(count).ok_or(Error::Overflow)?;
        let value = self.bytes.get(self.cursor..end).ok_or(Error::Incomplete)?;
        self.cursor = end;
        Ok(value)
    }
    fn byte(&mut self) -> Result<u8, Error> { Ok(self.take(1)?[0]) }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().map_err(|_| Error::Incomplete)?))
    }
    fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().map_err(|_| Error::Incomplete)?))
    }
    fn len32(&mut self) -> Result<usize, Error> {
        usize::try_from(self.u32()?).map_err(|_| Error::Limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> RestartProfile {
        RestartProfile { id: 1, generation: 2, host_generation: 3, model_generation: 4,
            tokenizer_generation: 5, state_schema_generation: 6, grade: RestartGrade::ExactRestart }
    }
    fn actor() -> ActorState {
        ActorState::new(profile(), vec![0, 1, u32::MAX], vec![9, 8, 7], vec![6, 5], 3).unwrap()
    }

    #[test]
    fn exact_round_trip_and_profile_binding() {
        let manifest = RestartManifest::capture(&actor()).unwrap();
        let bytes = manifest.encode().unwrap();
        assert_eq!(bytes.len(), manifest.encoded_len().unwrap());
        assert_eq!(RestartManifest::decode_for(&bytes, profile()).unwrap().actor(), &actor());
        let mut other = profile(); other.model_generation += 1;
        assert_eq!(RestartManifest::decode_for(&bytes, other), Err(Error::Binding));
    }

    #[test]
    fn every_truncation_and_trailing_byte_refuses() {
        let bytes = RestartManifest::capture(&actor()).unwrap().encode().unwrap();
        for end in 0..bytes.len() { assert!(RestartManifest::decode(&bytes[..end]).is_err()); }
        let mut extra = bytes; extra.push(0);
        assert_eq!(RestartManifest::decode(&extra), Err(Error::InvalidInput));
    }

    #[test]
    fn missing_component_reserved_bits_and_unknown_grade_refuse() {
        let bytes = RestartManifest::capture(&actor()).unwrap().encode().unwrap();
        for component in [COMPONENT_TOKENS, COMPONENT_CACHE, COMPONENT_SAMPLER, COMPONENT_POSITION] {
            let mut changed = bytes.clone(); changed[8] &= !component;
            assert_eq!(RestartManifest::decode(&changed), Err(Error::Incomplete));
        }
        let mut reserved = bytes.clone(); reserved[9] = 1;
        assert_eq!(RestartManifest::decode(&reserved), Err(Error::InvalidInput));
        let mut grade = bytes; grade[68] = 3;
        assert_eq!(RestartManifest::decode(&grade), Err(Error::InvalidInput));
    }

    #[test]
    fn token_position_and_declared_lengths_are_revalidated() {
        let bytes = RestartManifest::capture(&actor()).unwrap().encode().unwrap();
        let mut position = bytes.clone(); position[60..68].copy_from_slice(&4_u64.to_be_bytes());
        assert_eq!(RestartManifest::decode(&position), Err(Error::Binding));
        let mut cache = bytes.clone(); cache[76..80].copy_from_slice(&4_u32.to_be_bytes());
        assert_eq!(RestartManifest::decode(&cache), Err(Error::InvalidInput));
        let mut tokens = bytes; tokens[72..76].copy_from_slice(&(MAX_TOKENS as u32 + 1).to_be_bytes());
        assert_eq!(RestartManifest::decode(&tokens), Err(Error::Limit));
    }
}
