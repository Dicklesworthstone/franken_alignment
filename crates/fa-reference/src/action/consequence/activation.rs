//! Progressive binary32 observations, not restart checkpoints or authority.
//!
//! The capture owner checks every block against immutable source bits. A decoder
//! accepts only those checked blocks, from the same capture instance and in order.
//! This is a process-local source check, NOT transport authentication. Missing
//! mantissa bits denote an interval; they are never claimed to be observed zeroes.

use crate::Error;
use std::rc::Rc;

pub const MAX_VALUES: usize = 65_536;
pub const HEADER_BYTES: usize = 78;
pub const MAX_BLOCK_BYTES: usize = HEADER_BYTES + 4 * MAX_VALUES;
const DOMAIN: &[u8; 8] = b"FAPF32\0\x01";
const FRACTION_BITS: u8 = 23;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureProfile {
    pub tenant: u64,
    pub model: u64,
    pub model_generation: u64,
    pub tap: u64,
    pub layout_generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameIdentity {
    pub profile: CaptureProfile,
    pub stream: u64,
    pub sequence: u64,
    pub position: u64,
}

impl FrameIdentity {
    fn fields(self) -> [u64; 8] {
        [self.profile.tenant, self.profile.model, self.profile.model_generation,
            self.profile.tap, self.profile.layout_generation, self.stream,
            self.sequence, self.position]
    }
}

/// Immutable supplied activation values. Cloning shares evidence, never rights.
#[derive(Clone, Debug)]
pub struct SourceFrame {
    identity: FrameIdentity,
    words: Rc<[u32]>,
    binding: Rc<()>,
}

impl SourceFrame {
    pub fn capture(identity: FrameIdentity, values: &[f32]) -> Result<Self, Error> {
        if identity.fields()[..7].contains(&0) || values.is_empty() {
            return Err(Error::InvalidInput);
        }
        if values.len() > MAX_VALUES { return Err(Error::Limit); }
        if values.iter().any(|v| !v.is_finite()) { return Err(Error::InvalidInput); }
        Ok(Self {
            identity, words: values.iter().map(|v| v.to_bits()).collect::<Vec<_>>().into(),
            binding: Rc::new(()),
        })
    }

    pub fn identity(&self) -> FrameIdentity { self.identity }
    pub fn dimensions(&self) -> usize { self.words.len() }
    pub fn raw_bytes(&self) -> usize { self.words.len() * 4 }

    /// Initial blocks contain sign, exponent, then the selected mantissa prefix.
    /// Refinements contain only newly disclosed mantissa bits, per coordinate.
    /// Header and final-byte padding are included in this exact byte count.
    pub fn encoded_len(&self, from: Option<u8>, to: u8) -> Result<usize, Error> {
        Ok(HEADER_BYTES + (self.words.len() * usize::from(width(from, to)?)).div_ceil(8))
    }

    pub fn encode_initial(&self, bits: u8) -> Result<Vec<u8>, Error> {
        self.encode(None, bits)
    }

    pub fn encode_refinement(&self, from: u8, to: u8) -> Result<Vec<u8>, Error> {
        self.encode(Some(from), to)
    }

    fn encode(&self, from: Option<u8>, to: u8) -> Result<Vec<u8>, Error> {
        let width = width(from, to)?;
        let mut bytes = Vec::with_capacity(self.encoded_len(from, to)?);
        bytes.extend_from_slice(DOMAIN);
        for value in self.identity.fields() { bytes.extend_from_slice(&value.to_be_bytes()); }
        bytes.extend_from_slice(&(self.words.len() as u32).to_be_bytes());
        bytes.push(from.unwrap_or(u8::MAX));
        bytes.push(to);
        bytes.resize(self.encoded_len(from, to)?, 0);
        let mask = if width == 32 { u32::MAX } else { (1_u32 << width) - 1 };
        let mut offset = HEADER_BYTES * 8;
        for word in self.words.iter() {
            let value = (*word >> (FRACTION_BITS - to)) & mask;
            for bit in (0..width).rev() {
                bytes[offset / 8] |= (((value >> bit) & 1) as u8) << (7 - offset % 8);
                offset += 1;
            }
        }
        Ok(bytes)
    }

    /// A content/shape comparison against this actual supplied source, including
    /// every header field and padding bit. No caller-provided error bound is used.
    pub fn verify_block(&self, bytes: &[u8]) -> Result<VerifiedBlock, Error> {
        if bytes.len() > MAX_BLOCK_BYTES { return Err(Error::Limit); }
        if bytes.len() < HEADER_BYTES || &bytes[..8] != DOMAIN { return Err(Error::InvalidInput); }
        let from = match bytes[76] { u8::MAX => None, value => Some(value) };
        let to = bytes[77];
        let expected = self.encode(from, to)?;
        if bytes != expected.as_slice() { return Err(Error::Binding); }
        Ok(VerifiedBlock {
            identity: self.identity, dimensions: self.words.len(), binding: Rc::clone(&self.binding),
            from, to, bytes: expected,
        })
    }
}

/// No public constructor and no unchecked-byte decoder entrypoint. Checked
/// blocks can be shared as observations; they cannot become effect permits.
#[derive(Clone, Debug)]
pub struct VerifiedBlock {
    identity: FrameIdentity,
    dimensions: usize,
    binding: Rc<()>,
    from: Option<u8>,
    to: u8,
    bytes: Vec<u8>,
}

impl VerifiedBlock {
    pub fn bytes(&self) -> &[u8] { &self.bytes }
    pub fn identity(&self) -> FrameIdentity { self.identity }
    pub fn mantissa_bits(&self) -> u8 { self.to }
}

/// A source-bound partial observation. Prefixes contain zeros in the missing
/// bit positions, but only interval()/exact_values() export numerical values.
#[derive(Clone, Debug)]
pub struct ProgressiveFrame {
    identity: FrameIdentity,
    binding: Rc<()>,
    prefixes: Vec<u32>,
    bits: u8,
    wire_bytes: usize,
}

impl ProgressiveFrame {
    pub fn from_initial(block: &VerifiedBlock) -> Result<Self, Error> {
        if block.from.is_some() { return Err(Error::WrongState); }
        Ok(Self {
            identity: block.identity, binding: Rc::clone(&block.binding),
            prefixes: decode_words(block), bits: block.to, wire_bytes: block.bytes.len(),
        })
    }

    pub fn refine(&mut self, block: &VerifiedBlock) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.binding, &block.binding) || self.identity != block.identity
            || self.prefixes.len() != block.dimensions
        {
            return Err(Error::Binding);
        }
        if block.from != Some(self.bits) || block.to <= self.bits { return Err(Error::Stale); }
        let wire_bytes = self.wire_bytes.checked_add(block.bytes.len()).ok_or(Error::Overflow)?;
        let delta = decode_words(block);
        // Verified shape and ordering make publication below infallible.
        for (prefix, extra) in self.prefixes.iter_mut().zip(delta) { *prefix |= extra; }
        self.bits = block.to;
        self.wire_bytes = wire_bytes;
        Ok(())
    }

    pub fn identity(&self) -> FrameIdentity { self.identity }
    pub fn dimensions(&self) -> usize { self.prefixes.len() }
    pub fn mantissa_bits(&self) -> u8 { self.bits }
    pub fn wire_bytes(&self) -> usize { self.wire_bytes }
    pub fn is_exact(&self) -> bool { self.bits == FRACTION_BITS }

    /// Inclusive source-containing binary32 endpoints. Negative values reverse
    /// bit order; subnormals and both zero encodings are preserved without math.
    pub fn interval(&self, index: usize) -> Result<[f32; 2], Error> {
        let prefix = *self.prefixes.get(index).ok_or(Error::Missing)?;
        let mask = (1_u32 << (FRACTION_BITS - self.bits)) - 1;
        let near_zero = f32::from_bits(prefix);
        let far_zero = f32::from_bits(prefix | mask);
        Ok(if prefix >> 31 == 0 { [near_zero, far_zero] } else { [far_zero, near_zero] })
    }

    pub fn exact_values(&self) -> Result<Vec<f32>, Error> {
        if !self.is_exact() { return Err(Error::Incomplete); }
        Ok(self.prefixes.iter().map(|word| f32::from_bits(*word)).collect())
    }
}

fn width(from: Option<u8>, to: u8) -> Result<u8, Error> {
    if to > FRACTION_BITS || from.is_some_and(|v| v >= to) { return Err(Error::InvalidInput); }
    Ok(from.map_or(9 + to, |v| to - v))
}

fn decode_words(block: &VerifiedBlock) -> Vec<u32> {
    let width = block.from.map_or(9 + block.to, |v| block.to - v);
    let mut offset = HEADER_BYTES * 8;
    (0..block.dimensions).map(|_| {
        let mut value = 0_u32;
        for _ in 0..width {
            value = (value << 1) | u32::from((block.bytes[offset / 8] >> (7 - offset % 8)) & 1);
            offset += 1;
        }
        value << (FRACTION_BITS - block.to)
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> FrameIdentity {
        FrameIdentity { profile: CaptureProfile { tenant: 1, model: 2, model_generation: 3,
            tap: 4, layout_generation: 5 }, stream: 6, sequence: 7, position: 0 }
    }

    #[test]
    fn manual_golden_zero_mantissa_prefix_and_raw_words() {
        let source = SourceFrame::capture(identity(), &[1.0, -2.0, 0.0, -0.0]).unwrap();
        let coarse = source.encode_initial(0).unwrap();
        // Four independent 9-bit sign/exponent words: 07f, 180, 000, 100.
        assert_eq!(&coarse[HEADER_BYTES..], &[0x3f, 0xe0, 0x00, 0x10, 0x00]);
        let exact = source.encode_initial(23).unwrap();
        assert_eq!(&exact[HEADER_BYTES..], &[
            0x3f, 0x80, 0, 0, 0xc0, 0, 0, 0, 0, 0, 0, 0, 0x80, 0, 0, 0,
        ]);
        assert_eq!(coarse.len(), HEADER_BYTES + 5);
    }

    #[test]
    fn every_rung_contains_source_and_exact_rung_preserves_all_bits() {
        let values = [0.0, -0.0, f32::from_bits(1), -f32::from_bits(1),
            f32::MIN_POSITIVE, f32::from_bits(0x007f_ffff), 1.234567, -9.876543,
            f32::MAX, f32::MIN];
        let source = SourceFrame::capture(identity(), &values).unwrap();
        let initial = source.verify_block(&source.encode_initial(0).unwrap()).unwrap();
        let mut view = ProgressiveFrame::from_initial(&initial).unwrap();
        assert_eq!(view.exact_values(), Err(Error::Incomplete));
        for bits in 0..=23 {
            for (index, original) in values.iter().enumerate() {
                let [lo, hi] = view.interval(index).unwrap();
                assert!(lo <= *original && *original <= hi);
            }
            if bits < 23 {
                let before: Vec<_> = (0..values.len()).map(|i| view.interval(i).unwrap()).collect();
                let block = source.verify_block(&source.encode_refinement(bits, bits + 1).unwrap()).unwrap();
                view.refine(&block).unwrap();
                for (index, [lo, hi]) in before.into_iter().enumerate() {
                    let [next_lo, next_hi] = view.interval(index).unwrap();
                    assert!(lo <= next_lo && next_hi <= hi);
                }
            }
        }
        let actual: Vec<_> = view.exact_values().unwrap().iter().map(|v| v.to_bits()).collect();
        assert_eq!(actual, values.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
    }

    #[test]
    fn all_truncations_single_bit_mutations_and_extra_bytes_refuse() {
        let source = SourceFrame::capture(identity(), &[1.25, -2.75, 3.0]).unwrap();
        let valid = source.encode_initial(5).unwrap();
        source.verify_block(&valid).unwrap();
        for end in 0..valid.len() { assert!(source.verify_block(&valid[..end]).is_err()); }
        for bit in 0..valid.len() * 8 {
            let mut changed = valid.clone(); changed[bit / 8] ^= 1 << (bit % 8);
            assert!(source.verify_block(&changed).is_err());
        }
        let mut extra = valid; extra.push(0);
        assert!(source.verify_block(&extra).is_err());
    }

    #[test]
    fn foreign_same_id_source_and_out_of_order_refinement_are_atomic() {
        let source = SourceFrame::capture(identity(), &[1.0]).unwrap();
        let other = SourceFrame::capture(identity(), &[1.0]).unwrap();
        let mut view = ProgressiveFrame::from_initial(&source.verify_block(&source.encode_initial(0).unwrap()).unwrap()).unwrap();
        let original = (view.interval(0).unwrap(), view.wire_bytes());
        let foreign = other.verify_block(&other.encode_refinement(0, 5).unwrap()).unwrap();
        assert_eq!(view.refine(&foreign), Err(Error::Binding));
        let late = source.verify_block(&source.encode_refinement(5, 10).unwrap()).unwrap();
        assert_eq!(view.refine(&late), Err(Error::Stale));
        assert_eq!((view.interval(0).unwrap(), view.wire_bytes()), original);
        let next = source.verify_block(&source.encode_refinement(0, 5).unwrap()).unwrap();
        view.refine(&next).unwrap();
        assert_eq!(view.refine(&next), Err(Error::Stale));
    }

    #[test]
    fn finite_capture_dimensions_and_bit_ranges_are_bounded() {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(SourceFrame::capture(identity(), &[bad]).is_err());
        }
        assert!(SourceFrame::capture(identity(), &[]).is_err());
        let source = SourceFrame::capture(identity(), &vec![0.0; MAX_VALUES]).unwrap();
        assert_eq!(source.encode_initial(23).unwrap().len(), MAX_BLOCK_BYTES);
        assert!(SourceFrame::capture(identity(), &vec![0.0; MAX_VALUES + 1]).is_err());
        assert!(source.encode_initial(24).is_err());
        for (from, to) in [(0, 0), (23, 23), (24, 23), (255, 1), (0, 24)] {
            assert!(source.encode_refinement(from, to).is_err());
        }
    }
}
