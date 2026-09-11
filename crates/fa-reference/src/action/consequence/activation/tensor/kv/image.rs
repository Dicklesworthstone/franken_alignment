//! Immutable KV value images and a bounded, exact portable representation.
//!
//! Images retain no live controller, mutable capture store or host-buffer receipt.
//! A separately supplied descriptor binds metadata, not byte authenticity. The
//! caller must authenticate stored bytes under its own admitted storage profile.

use super::{KvCapture, KvContract, MAX_KV_POSITIONS, MAX_KV_VALUES};
use super::restore::{KvDestination, KvRestoreWindow, PreparedKvRestore, encode_exact, prepare_pair, source_range};
use super::super::{ByteOrder, ScalarEncoding, TensorContract, decode_scalar};
use crate::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
use crate::Error;
use std::rc::Rc;

pub const IMAGE_HEADER_BYTES: usize = 156;
pub const MAX_IMAGE_BYTES: usize = IMAGE_HEADER_BYTES + 4 * MAX_KV_VALUES;
const DOMAIN: &[u8; 8] = b"FAKVIMG\x01";

/// Keep independently when checking a received image's intended context. This
/// is data, not an integrity commitment. Matching it does not detect all edits
/// to finite tensor values; no crypto or external completeness is implied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KvImageDescriptor {
    pub contract: KvContract,
    pub stream: u64,
    pub source_batch: usize,
    pub first_position: u64,
    pub first_sequence: u64,
    pub source_revision: u64,
    pub token_count: usize,
}

impl KvImageDescriptor {
    pub fn normalized_values(&self) -> Result<usize, Error> {
        self.contract.keys().dimensions().checked_add(self.contract.values().dimensions())
            .and_then(|n| n.checked_mul(self.token_count)).ok_or(Error::Overflow)
    }

    pub fn encoded_len(&self) -> Result<usize, Error> {
        if self.stream == 0 || self.first_sequence == 0 { return Err(Error::InvalidInput); }
        if self.token_count > MAX_KV_POSITIONS || self.normalized_values()? > MAX_KV_VALUES {
            return Err(Error::Limit);
        }
        let count = u64::try_from(self.token_count).map_err(|_| Error::Overflow)?;
        if (count == 0 && self.source_revision != 0)
            || (count > 0 && (self.source_revision == 0 || self.source_revision > count))
        { return Err(Error::InvalidInput); }
        self.first_position.checked_add(count).ok_or(Error::Overflow)?;
        self.first_sequence.checked_add(count).ok_or(Error::Overflow)?;
        let per_token = self.contract.keys().dimensions() * self.contract.keys().encoding().bytes()
            + self.contract.values().dimensions() * self.contract.values().encoding().bytes();
        IMAGE_HEADER_BYTES.checked_add(per_token.checked_mul(self.token_count).ok_or(Error::Overflow)?)
            .ok_or(Error::Overflow)
    }

    /// The descriptor alone is a fixed-size manifest, independently retainable.
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        self.encoded_len()?;
        let mut bytes = Vec::with_capacity(IMAGE_HEADER_BYTES);
        bytes.extend_from_slice(DOMAIN);
        put32(&mut bytes, self.token_count)?;
        for value in [u64::try_from(self.source_batch).map_err(|_| Error::Overflow)?, self.stream,
            self.first_position, self.first_sequence, self.source_revision]
        { bytes.extend_from_slice(&value.to_be_bytes()); }
        put_contract(&mut bytes, self.contract.keys())?;
        put_contract(&mut bytes, self.contract.values())?;
        put32(&mut bytes, self.contract.query_heads())?;
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != IMAGE_HEADER_BYTES { return Err(Error::InvalidInput); }
        let mut reader = Reader { bytes, cursor: 0 };
        if reader.take(8)? != DOMAIN { return Err(Error::InvalidInput); }
        let token_count = reader.usize32()?;
        let source_batch = usize::try_from(reader.u64()?).map_err(|_| Error::Limit)?;
        let stream = reader.u64()?;
        let first_position = reader.u64()?;
        let first_sequence = reader.u64()?;
        let source_revision = reader.u64()?;
        let keys = read_contract(&mut reader)?;
        let values = read_contract(&mut reader)?;
        let contract = KvContract::new(keys, values, reader.usize32()?)?;
        let descriptor = Self { contract, stream, source_batch, first_position,
            first_sequence, source_revision, token_count };
        descriptor.encoded_len()?;
        if reader.cursor != bytes.len() { return Err(Error::InvalidInput); }
        Ok(descriptor)
    }
}

/// Canonical logical values; an imported token never claims an old host-buffer
/// generation or successful capture receipt it cannot establish.
#[derive(Clone, Debug)]
pub struct KvImageToken { key: SourceFrame, value: SourceFrame }

impl KvImageToken {
    pub fn key(&self) -> &SourceFrame { &self.key }
    pub fn value(&self) -> &SourceFrame { &self.value }
}

/// Snapshot cloning shares all arrays and token metadata. Initial freezing
/// copies O(tokens) metadata but no activation arrays. No mutable branch or
/// conversion into a live capture frontier or actor authority is exposed.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::tensor::kv::image::KvImage;
/// use fa_reference::action::Permit;
/// fn grant(image: KvImage) -> Permit { image }
/// ```
#[derive(Clone, Debug)]
pub struct KvImage {
    descriptor: KvImageDescriptor,
    tokens: Rc<[KvImageToken]>,
}

impl KvCapture {
    pub fn snapshot(&self, expected_revision: u64) -> Result<KvImage, Error> {
        if self.revision != expected_revision { return Err(Error::Stale); }
        let first_sequence = self.next_sequence.checked_sub(self.tokens.len() as u64).ok_or(Error::Binding)?;
        let descriptor = KvImageDescriptor {
            contract: self.contract.clone(), stream: self.stream, source_batch: self.batch,
            first_position: self.first_position, first_sequence, source_revision: self.revision,
            token_count: self.tokens.len(),
        };
        descriptor.encoded_len()?;
        let mut tokens = Vec::new();
        tokens.try_reserve_exact(self.tokens.len()).map_err(|_| Error::Limit)?;
        for token in &self.tokens {
            tokens.push(KvImageToken { key: token.key().source().clone(), value: token.value().source().clone() });
        }
        Ok(KvImage { descriptor, tokens: tokens.into() })
    }
}

impl KvImage {
    pub fn descriptor(&self) -> &KvImageDescriptor { &self.descriptor }
    pub fn len(&self) -> usize { self.tokens.len() }
    pub fn is_empty(&self) -> bool { self.tokens.is_empty() }

    pub fn token(&self, position: u64) -> Result<&KvImageToken, Error> {
        let index = position.checked_sub(self.descriptor.first_position).ok_or(Error::Missing)?;
        self.tokens.get(usize::try_from(index).map_err(|_| Error::Missing)?).ok_or(Error::Missing)
    }

    /// Canonical scalar bytes use the registered original encodings in fixed
    /// key-then-value, token-major order. Original physical padding is not data.
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        let mut bytes = self.descriptor.encode()?;
        let length = self.descriptor.encoded_len()?;
        bytes.try_reserve_exact(length - bytes.len()).map_err(|_| Error::Limit)?;
        for token in self.tokens.iter() {
            for (source, contract) in [(&token.key, self.descriptor.contract.keys()),
                (&token.value, self.descriptor.contract.values())]
            {
                for bits in source.words.iter().copied() {
                    let scalar = encode_exact(bits, contract.encoding(), contract.byte_order())?;
                    bytes.extend_from_slice(&scalar[..contract.encoding().bytes()]);
                }
            }
        }
        if bytes.len() != length { return Err(Error::Binding); }
        Ok(bytes)
    }

    /// Parsing verifies bounds, exact framing, finite scalars and expected
    /// metadata. It does not authenticate data, grant a restart grade, or prove
    /// that these values were produced by the declared model.
    pub fn decode(bytes: &[u8], expected: &KvImageDescriptor) -> Result<Self, Error> {
        expected.encoded_len()?;
        if bytes.len() > MAX_IMAGE_BYTES { return Err(Error::Limit); }
        let header = bytes.get(..IMAGE_HEADER_BYTES).ok_or(Error::Incomplete)?;
        let descriptor = KvImageDescriptor::decode(header)?;
        if &descriptor != expected { return Err(Error::Binding); }
        if bytes.len() != descriptor.encoded_len()? { return Err(Error::InvalidInput); }
        let mut reader = Reader { bytes, cursor: IMAGE_HEADER_BYTES };
        let mut tokens = Vec::new();
        tokens.try_reserve_exact(descriptor.token_count).map_err(|_| Error::Limit)?;
        for index in 0..descriptor.token_count {
            let key = read_source(&mut reader, &descriptor, descriptor.contract.keys(), index)?;
            let value = read_source(&mut reader, &descriptor, descriptor.contract.values(), index)?;
            tokens.push(KvImageToken { key, value });
        }
        if reader.cursor != bytes.len() { return Err(Error::InvalidInput); }
        Ok(Self { descriptor, tokens: tokens.into() })
    }

    pub fn prepare_restore<'a>(
        &self, destination: KvDestination<'a>, window: KvRestoreWindow,
    ) -> Result<PreparedKvRestore<'a>, Error> {
        let range = source_range(self.descriptor.first_position, self.tokens.len(), window)?;
        let keys: Vec<_> = self.tokens[range.clone()].iter().map(KvImageToken::key).collect();
        let values: Vec<_> = self.tokens[range].iter().map(KvImageToken::value).collect();
        prepare_pair(&self.descriptor.contract, &keys, &values, destination, window)
    }
}

fn read_source(
    reader: &mut Reader<'_>, descriptor: &KvImageDescriptor, contract: &TensorContract, index: usize,
) -> Result<SourceFrame, Error> {
    let mut values = Vec::new();
    values.try_reserve_exact(contract.dimensions()).map_err(|_| Error::Limit)?;
    for _ in 0..contract.dimensions() {
        values.push(decode_scalar(reader.take(contract.encoding().bytes())?, contract.encoding(), contract.byte_order())?);
    }
    SourceFrame::capture(FrameIdentity {
        profile: contract.profile(), stream: descriptor.stream,
        sequence: descriptor.first_sequence + index as u64,
        position: descriptor.first_position + index as u64,
    }, &values)
}

fn put32(bytes: &mut Vec<u8>, value: usize) -> Result<(), Error> {
    bytes.extend_from_slice(&u32::try_from(value).map_err(|_| Error::Limit)?.to_be_bytes());
    Ok(())
}
fn put_contract(bytes: &mut Vec<u8>, contract: &TensorContract) -> Result<(), Error> {
    let p = contract.profile();
    for value in [p.tenant, p.model, p.model_generation, p.tap, p.layout_generation] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes.push(match contract.encoding() { ScalarEncoding::Binary32 => 0, ScalarEncoding::Binary16 => 1, ScalarEncoding::BFloat16 => 2 });
    bytes.push(match contract.byte_order() { ByteOrder::Little => 0, ByteOrder::Big => 1 });
    put32(bytes, contract.heads())?;
    put32(bytes, contract.channels())
}
fn read_contract(reader: &mut Reader<'_>) -> Result<TensorContract, Error> {
    let profile = CaptureProfile { tenant: reader.u64()?, model: reader.u64()?,
        model_generation: reader.u64()?, tap: reader.u64()?, layout_generation: reader.u64()? };
    let encoding = match reader.take(1)?[0] {
        0 => ScalarEncoding::Binary32, 1 => ScalarEncoding::Binary16, 2 => ScalarEncoding::BFloat16,
        _ => return Err(Error::InvalidInput),
    };
    let order = match reader.take(1)?[0] { 0 => ByteOrder::Little, 1 => ByteOrder::Big, _ => return Err(Error::InvalidInput) };
    TensorContract::new(profile, encoding, order, reader.usize32()?, reader.usize32()?)
}
struct Reader<'a> { bytes: &'a [u8], cursor: usize }
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let end = self.cursor.checked_add(count).ok_or(Error::Overflow)?;
        let bytes = self.bytes.get(self.cursor..end).ok_or(Error::Incomplete)?;
        self.cursor = end;
        Ok(bytes)
    }
    fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().map_err(|_| Error::Incomplete)?))
    }
    fn usize32(&mut self) -> Result<usize, Error> {
        usize::try_from(u32::from_be_bytes(self.take(4)?.try_into().map_err(|_| Error::Incomplete)?))
            .map_err(|_| Error::Limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::consequence::activation::tensor::{BufferIdentity, HostTensor, TensorLayout};
    use super::super::{KvAppend, KvBudget};

    #[test]
    fn freezing_shares_arrays_and_cloning_shares_the_entire_snapshot() {
        let p = CaptureProfile { tenant: 1, model: 2, model_generation: 3, tap: 4, layout_generation: 5 };
        let k = TensorContract::new(p, ScalarEncoding::Binary32, ByteOrder::Little, 1, 1).unwrap();
        let v = TensorContract::new(CaptureProfile { tap: 6, ..p }, ScalarEncoding::Binary32, ByteOrder::Little, 1, 1).unwrap();
        let mut capture = KvCapture::new(KvContract::new(k, v, 1).unwrap(), 7, 0, 0, 1,
            KvBudget { positions: 1, normalized_values: 2 }).unwrap();
        let layout = TensorLayout::new([1; 4], [0; 4], 0, ScalarEncoding::Binary32, ByteOrder::Little).unwrap();
        let kb = 1_f32.to_le_bytes();
        let vb = 2_f32.to_le_bytes();
        capture.append(0, KvAppend {
            keys: HostTensor { identity: BufferIdentity { object: 1, generation: 1 }, layout: &layout, bytes: &kb },
            values: HostTensor { identity: BufferIdentity { object: 2, generation: 1 }, layout: &layout, bytes: &vb },
            first_token: 0, token_count: 1, buffer_first_position: 0, first_sequence: 1,
        }).unwrap();
        let image = capture.snapshot(1).unwrap();
        assert!(Rc::ptr_eq(&image.tokens[0].key.words, &capture.tokens[0].key().source().words));
        assert!(Rc::ptr_eq(&image.tokens[0].value.words, &capture.tokens[0].value().source().words));
        assert!(Rc::ptr_eq(&image.tokens, &image.clone().tokens));
    }
}
