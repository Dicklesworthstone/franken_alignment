//! Strict FA-BBPE/1 and named-control FA-BBPE/2 interchange. A file cannot choose its model identity,
//! dimensions or numerical profile: compare the independently supplied header
//! before allocating its vocabulary. This encoding provides no authentication.
#[path = "huggingface.rs"]
mod huggingface;

use super::*;

const DOMAIN: &[u8; 8] = b"FABBPE01";
const NAMED_DOMAIN: &[u8; 8] = b"FABBPE02";
const HEADER_BYTES: usize = 120;
pub(super) const MAX_FILE_BYTES: usize = HEADER_BYTES + 5 * MAX_DECODER_VOCABULARY
    + MAX_VOCABULARY_BYTES + 4 + 12 * MAX_MERGES
    + 4 + 8 * MAX_CONTROL_TOKENS + MAX_SPECIAL_TOKEN_BYTES;

impl ByteBpe {
    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut count = HEADER_BYTES + 4 + 12 * self.0.merges.len();
        for token in &self.0.vocabulary {
            count = count.checked_add(match token {
                TokenBytes::Control => 1,
                TokenBytes::Content(bytes) => 5 + bytes.len(),
            }).ok_or(Error::Limit)?;
        }
        let named = !self.special_tokens().is_empty();
        if named {
            count = count.checked_add(4).ok_or(Error::Limit)?;
            for name in self.special_tokens().values() {
                count = count.checked_add(8 + name.len()).ok_or(Error::Limit)?;
            }
        }
        if count > MAX_FILE_BYTES { return Err(Error::Limit); }
        let mut output = Vec::new();
        output.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        let mut encoded_header = header(self.profile());
        if named { encoded_header[..8].copy_from_slice(NAMED_DOMAIN); }
        output.extend_from_slice(&encoded_header);
        for token in &self.0.vocabulary {
            match token {
                TokenBytes::Control => output.push(0),
                TokenBytes::Content(bytes) => {
                    output.push(1);
                    output.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
                    output.extend_from_slice(bytes);
                }
            }
        }
        output.extend_from_slice(&(self.0.merges.len() as u32).to_be_bytes());
        for rule in &self.0.merges {
            for token in [rule.left, rule.right, rule.result] {
                output.extend_from_slice(&token.to_be_bytes());
            }
        }
        if named {
            output.extend_from_slice(&(self.special_tokens().len() as u32).to_be_bytes());
            for (id, name) in self.special_tokens() {
                output.extend_from_slice(&id.to_be_bytes());
                output.extend_from_slice(&(name.len() as u32).to_be_bytes());
                output.extend_from_slice(name);
            }
        }
        Ok(output)
    }

    /// Canonical big-endian lengths/IDs with exact coverage, no unknown tags,
    /// trailing bytes, padding, profile guessing or permissive legacy fallback.
    /// The full original inventory validation also applies to imported data.
    pub fn from_bytes(expected: &DecoderProfile, bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_FILE_BYTES { return Err(Error::Limit); }
        let mut reader = Reader { bytes, at: 0 };
        let supplied = reader.take(HEADER_BYTES)?;
        let named = match &supplied[..8] {
            domain if domain == DOMAIN => false,
            domain if domain == NAMED_DOMAIN => true,
            _ => return Err(Error::Binding),
        };
        let mut expected_header = header(expected);
        if named { expected_header[..8].copy_from_slice(NAMED_DOMAIN); }
        if supplied != expected_header.as_slice() { return Err(Error::Binding); }
        let count = expected.shape().vocabulary;
        if count < 256 { return Err(Error::Incomplete); }
        let mut vocabulary = Vec::new();
        vocabulary.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        let mut retained = 0_usize;
        for _ in 0..count {
            vocabulary.push(match reader.take(1)?[0] {
                0 => TokenBytes::Control,
                1 => {
                    let length = reader.u32()? as usize;
                    if length == 0 { return Err(Error::InvalidInput); }
                    if length > MAX_TOKEN_BYTES { return Err(Error::Limit); }
                    retained = retained.checked_add(length).ok_or(Error::Limit)?;
                    if retained > MAX_VOCABULARY_BYTES { return Err(Error::Limit); }
                    let source = reader.take(length)?;
                    let mut owned = Vec::new();
                    owned.try_reserve_exact(length).map_err(|_| Error::Limit)?;
                    owned.extend_from_slice(source);
                    TokenBytes::Content(owned)
                }
                _ => return Err(Error::InvalidInput),
            });
        }
        let count = reader.u32()? as usize;
        if count > MAX_MERGES { return Err(Error::Limit); }
        // Admit merge storage only when its complete declared bytes are present.
        // Legacy archives still require exact EOF immediately after the merges.
        let remaining = reader.bytes.len() - reader.at;
        if (!named && remaining != count * 12)
            || (named && remaining < count * 12 + 4)
        { return Err(Error::InvalidInput); }
        let mut merges = Vec::new();
        merges.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        for _ in 0..count {
            merges.push(Merge { left: reader.u32()?, right: reader.u32()?, result: reader.u32()? });
        }
        if !named { return Self::new(expected.clone(), vocabulary, merges); }
        let count = reader.u32()? as usize;
        // Empty tables have exactly one canonical representation: version one.
        if count == 0 { return Err(Error::InvalidInput); }
        if count > MAX_CONTROL_TOKENS { return Err(Error::Limit); }
        let mut names = BTreeMap::new();
        let mut previous = None;
        let mut retained = 0_usize;
        for _ in 0..count {
            let id = reader.u32()?;
            if previous.is_some_and(|value| id <= value) { return Err(Error::Binding); }
            previous = Some(id);
            let length = reader.u32()? as usize;
            if length == 0 { return Err(Error::InvalidInput); }
            if length > MAX_TOKEN_BYTES { return Err(Error::Limit); }
            retained = retained.checked_add(length).ok_or(Error::Limit)?;
            if retained > MAX_SPECIAL_TOKEN_BYTES { return Err(Error::Limit); }
            if !matches!(vocabulary.get(id as usize), Some(TokenBytes::Control)) {
                return Err(Error::Binding);
            }
            let source = reader.take(length)?;
            let mut name = Vec::new();
            name.try_reserve_exact(length).map_err(|_| Error::Limit)?;
            name.extend_from_slice(source);
            names.insert(id, name);
        }
        if reader.at != bytes.len() { return Err(Error::InvalidInput); }
        Self::new_with_special_tokens(expected.clone(), vocabulary, merges, names)
    }
}

fn header(profile: &DecoderProfile) -> [u8; HEADER_BYTES] {
    let id = profile.identity();
    let shape = profile.shape();
    let fields = [id.tenant, id.model, id.model_generation, id.tokenizer_generation, id.profile_generation,
        shape.vocabulary as u64, shape.hidden as u64, shape.intermediate as u64, shape.layers as u64,
        shape.query_heads as u64, shape.cache_heads as u64, shape.context as u64,
        profile.epsilon().to_bits(), profile.theta().to_bits()];
    let mut bytes = [0_u8; HEADER_BYTES];
    bytes[..8].copy_from_slice(DOMAIN);
    for (index, value) in fields.iter().enumerate() {
        bytes[8 + index * 8..16 + index * 8].copy_from_slice(&value.to_be_bytes());
    }
    bytes
}
struct Reader<'a> { bytes: &'a [u8], at: usize }
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let end = self.at.checked_add(count).ok_or(Error::Limit)?;
        let bytes = self.bytes.get(self.at..end).ok_or(Error::Incomplete)?;
        self.at = end;
        Ok(bytes)
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().map_err(|_| Error::Incomplete)?))
    }
}
