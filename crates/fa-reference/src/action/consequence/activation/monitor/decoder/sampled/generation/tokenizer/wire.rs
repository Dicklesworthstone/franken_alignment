//! Strict FA-BBPE/1 and named-control FA-BBPE/2 interchange. Static scaled model
//! profiles use /3 and /4 with an exact numerical extension; legacy bytes stay.
//! Explicit Unicode-17 input grammars use /5 and bind their whole interpretation.
//! A file cannot choose its model identity,
//! dimensions or numerical profile: compare the independently supplied header
//! before allocating its vocabulary. This encoding provides no authentication.
#[path = "huggingface.rs"]
mod huggingface;

use super::*;

const DOMAIN: &[u8; 8] = b"FABBPE01";
const NAMED_DOMAIN: &[u8; 8] = b"FABBPE02";
const SCALED_DOMAIN: &[u8; 8] = b"FABBPE03";
const SCALED_NAMED_DOMAIN: &[u8; 8] = b"FABBPE04";
const UNICODE_DOMAIN: &[u8; 8] = b"FABBPE05";
const HEADER_BYTES: usize = 120;
const UNICODE_BINDING_BYTES: usize = 6;
pub(super) const MAX_FILE_BYTES: usize = HEADER_BYTES + 5 * MAX_DECODER_VOCABULARY
    + MAX_VOCABULARY_BYTES + 4 + 12 * MAX_MERGES
    + 4 + 8 * MAX_CONTROL_TOKENS + MAX_SPECIAL_TOKEN_BYTES + UNICODE_BINDING_BYTES;

impl ByteBpe {
    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let scaling = self.profile().rotary_scaling().binding_bytes();
        let unicode = self.pre_tokenization() != PreTokenization::RawBytes;
        let mut count = HEADER_BYTES + scaling.as_ref().map_or(0, |bytes| bytes.len())
            + if unicode { UNICODE_BINDING_BYTES } else { 0 }
            + 4 + 12 * self.0.merges.len();
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
        encoded_header[..8].copy_from_slice(domain(named, scaling.is_some(), unicode));
        output.extend_from_slice(&encoded_header);
        if unicode {
            output.extend_from_slice(&unicode_binding(self.pre_tokenization(), named, scaling.is_some())?);
        }
        if let Some(binding) = scaling { output.extend_from_slice(&binding); }
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
        let unicode = &supplied[..8] == UNICODE_DOMAIN;
        let (named, scaled, pre_tokenization) = match &supplied[..8] {
            domain if domain == DOMAIN => (false, false, PreTokenization::RawBytes),
            domain if domain == NAMED_DOMAIN => (true, false, PreTokenization::RawBytes),
            domain if domain == SCALED_DOMAIN => (false, true, PreTokenization::RawBytes),
            domain if domain == SCALED_NAMED_DOMAIN => (true, true, PreTokenization::RawBytes),
            domain if domain == UNICODE_DOMAIN => read_unicode_binding(reader.take(UNICODE_BINDING_BYTES)?)?,
            _ => return Err(Error::Binding),
        };
        let scaling = expected.rotary_scaling().binding_bytes();
        if scaled != scaling.is_some() { return Err(Error::Binding); }
        let mut expected_header = header(expected);
        expected_header[..8].copy_from_slice(domain(named, scaled, unicode));
        if supplied != expected_header.as_slice() { return Err(Error::Binding); }
        if let Some(binding) = scaling {
            if reader.take(binding.len())? != binding { return Err(Error::Binding); }
        }
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
        if !named {
            return Self::new_with_pre_tokenization(expected.clone(), vocabulary, merges,
                BTreeMap::new(), pre_tokenization);
        }
        let count = reader.u32()? as usize;
        // Empty tables use the canonical profile without a named extension.
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
        Self::new_with_pre_tokenization(expected.clone(), vocabulary, merges, names, pre_tokenization)
    }
}

fn domain(named: bool, scaled: bool, unicode: bool) -> &'static [u8; 8] {
    if unicode { return UNICODE_DOMAIN; }
    match (named, scaled) {
        (false, false) => DOMAIN,
        (true, false) => NAMED_DOMAIN,
        (false, true) => SCALED_DOMAIN,
        (true, true) => SCALED_NAMED_DOMAIN,
    }
}

fn unicode_binding(mode: PreTokenization, named: bool, scaled: bool)
    -> Result<[u8; UNICODE_BINDING_BYTES], Error>
{
    let algorithm = match mode {
        PreTokenization::RawBytes => return Err(Error::Binding),
        PreTokenization::Gpt2Unicode17 => 1,
        PreTokenization::Llama3Unicode17 => 2,
    };
    let (major, minor, patch) = mode.unicode_version().ok_or(Error::Binding)?;
    // Prefix insertion is explicitly false. The two low flags retain the
    // original named-control and static numerical extensions; no other bits exist.
    Ok([algorithm, major, minor, patch, u8::from(named) | (u8::from(scaled) << 1), 0])
}

fn read_unicode_binding(bytes: &[u8]) -> Result<(bool, bool, PreTokenization), Error> {
    let mode = match bytes[0] {
        1 => PreTokenization::Gpt2Unicode17,
        2 => PreTokenization::Llama3Unicode17,
        _ => return Err(Error::Binding),
    };
    let named = bytes[4] & 1 != 0;
    let scaled = bytes[4] & 2 != 0;
    if bytes != unicode_binding(mode, named, scaled)? { return Err(Error::Binding); }
    Ok((named, scaled, mode))
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
