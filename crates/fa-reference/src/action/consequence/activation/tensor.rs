//! Checked borrowed host tensors and owned token captures.
//!
//! Logical axes are always [batch, token, cache_head, channel], independently
//! of physical strides. This CPU byte-slice boundary neither synchronizes a GPU
//! nor authenticates host metadata. Capturing owns only the selected token.

pub mod kv;

use super::{CaptureProfile, FrameIdentity, MAX_VALUES, SourceFrame};
use crate::Error;

pub const MAX_HOST_BYTES: usize = 1_073_741_824;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScalarEncoding { Binary32, Binary16, BFloat16 }

impl ScalarEncoding {
    pub fn bytes(self) -> usize {
        match self { Self::Binary32 => 4, Self::Binary16 | Self::BFloat16 => 2 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ByteOrder { Little, Big }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferIdentity { pub object: u64, pub generation: u64 }

/// A sufficient non-overlap certificate, not a general strided-alias solver.
/// Permuted dense axes, padding and singleton zero strides are supported.
/// Broadcast/non-singleton zero strides and uncertifiable interleaving refuse.
/// All offsets/strides are BYTES; unaligned byte-slice origins are supported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TensorLayout {
    shape: [usize; 4],
    strides: [usize; 4],
    offset: usize,
    encoding: ScalarEncoding,
    order: ByteOrder,
    end: usize,
}

impl TensorLayout {
    pub fn new(
        shape: [usize; 4], strides: [usize; 4], offset: usize,
        encoding: ScalarEncoding, order: ByteOrder,
    ) -> Result<Self, Error> {
        if shape.contains(&0) { return Err(Error::InvalidInput); }
        shape.iter().try_fold(1_usize, |n, size| n.checked_mul(*size)).ok_or(Error::Overflow)?;
        let mut axes: Vec<_> = (0..4).filter(|axis| shape[*axis] > 1).collect();
        axes.sort_unstable_by_key(|axis| strides[*axis]);
        let mut span = encoding.bytes();
        for axis in axes {
            // Translates enclosing disjoint byte spans, including element width.
            if strides[axis] < span { return Err(Error::InvalidInput); }
            span = strides[axis].checked_mul(shape[axis] - 1)
                .and_then(|delta| span.checked_add(delta)).ok_or(Error::Overflow)?;
        }
        let end = offset.checked_add(span).ok_or(Error::Overflow)?;
        if end > MAX_HOST_BYTES { return Err(Error::Limit); }
        Ok(Self { shape, strides, offset, encoding, order, end })
    }

    pub fn shape(&self) -> [usize; 4] { self.shape }
    pub fn byte_strides(&self) -> [usize; 4] { self.strides }
    pub fn byte_range(&self) -> std::ops::Range<usize> { self.offset..self.end }
    pub fn encoding(&self) -> ScalarEncoding { self.encoding }
    pub fn byte_order(&self) -> ByteOrder { self.order }

    fn offset_of(&self, coordinates: [usize; 4]) -> Result<usize, Error> {
        if coordinates.iter().zip(self.shape).any(|(index, size)| *index >= size) {
            return Err(Error::InvalidInput);
        }
        coordinates.iter().zip(self.strides).try_fold(self.offset, |offset, (index, stride)| {
            index.checked_mul(stride).and_then(|n| offset.checked_add(n)).ok_or(Error::Overflow)
        })
    }
}

/// Untrusted borrowed arguments. Constructors of downstream captures validate
/// these fields before copying. The lifetime prevents ordinary safe-Rust buffer
/// mutation while borrowed; it is not an external-device completion fence.
#[derive(Clone, Copy, Debug)]
pub struct HostTensor<'a> {
    pub identity: BufferIdentity,
    pub layout: &'a TensorLayout,
    pub bytes: &'a [u8],
}

impl HostTensor<'_> {
    fn validate(&self) -> Result<(), Error> {
        if self.identity.object == 0 || self.identity.generation == 0 { return Err(Error::InvalidInput); }
        if self.bytes.len() > MAX_HOST_BYTES { return Err(Error::Limit); }
        if self.layout.end > self.bytes.len() { return Err(Error::Incomplete); }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenSelection {
    pub batch: usize,
    pub token: usize,
    pub first_position: u64,
    pub stream: u64,
    pub sequence: u64,
}

/// Freeze encoding, byte order and the head/channel flattening semantics with
/// the capture profile. The token/batch extents may grow; their axes never move.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TensorContract {
    profile: CaptureProfile,
    encoding: ScalarEncoding,
    order: ByteOrder,
    heads: usize,
    channels: usize,
}

impl TensorContract {
    pub fn new(
        profile: CaptureProfile, encoding: ScalarEncoding, order: ByteOrder,
        heads: usize, channels: usize,
    ) -> Result<Self, Error> {
        if [profile.tenant, profile.model, profile.model_generation, profile.tap,
            profile.layout_generation].contains(&0) || heads == 0 || channels == 0
        { return Err(Error::InvalidInput); }
        let dimensions = heads.checked_mul(channels).ok_or(Error::Overflow)?;
        if dimensions > MAX_VALUES { return Err(Error::Limit); }
        Ok(Self { profile, encoding, order, heads, channels })
    }

    pub fn profile(&self) -> CaptureProfile { self.profile }
    pub fn dimensions(&self) -> usize { self.heads * self.channels }
    pub fn heads(&self) -> usize { self.heads }
    pub fn channels(&self) -> usize { self.channels }
    pub fn encoding(&self) -> ScalarEncoding { self.encoding }
    pub fn byte_order(&self) -> ByteOrder { self.order }

    pub fn frame_identity(&self, selected: TokenSelection) -> Result<FrameIdentity, Error> {
        if selected.stream == 0 || selected.sequence == 0 { return Err(Error::InvalidInput); }
        let token = u64::try_from(selected.token).map_err(|_| Error::Overflow)?;
        let position = selected.first_position.checked_add(token).ok_or(Error::Overflow)?;
        Ok(FrameIdentity { profile: self.profile, stream: selected.stream,
            sequence: selected.sequence, position })
    }

    fn validate_tensor(&self, tensor: HostTensor<'_>) -> Result<(), Error> {
        tensor.validate()?;
        if tensor.layout.encoding != self.encoding || tensor.layout.order != self.order
            || tensor.layout.shape[2] != self.heads || tensor.layout.shape[3] != self.channels
        { return Err(Error::Binding); }
        Ok(())
    }

    pub fn capture(&self, tensor: HostTensor<'_>, selected: TokenSelection) -> Result<TensorCapture, Error> {
        self.validate_tensor(tensor)?;
        let identity = self.frame_identity(selected)?;
        tensor.layout.offset_of([selected.batch, selected.token, 0, 0])?;
        let mut values = Vec::with_capacity(self.dimensions());
        for head in 0..self.heads {
            for channel in 0..self.channels {
                let offset = tensor.layout.offset_of([selected.batch, selected.token, head, channel])?;
                let end = offset.checked_add(self.encoding.bytes()).ok_or(Error::Overflow)?;
                let bytes = tensor.bytes.get(offset..end).ok_or(Error::Incomplete)?;
                values.push(decode_scalar(bytes, self.encoding, self.order)?);
            }
        }
        let source = SourceFrame::capture(identity, &values)?;
        let receipt = TensorCaptureReceipt {
            contract: self.clone(), buffer: tensor.identity, layout: tensor.layout.clone(), selected,
            source_bytes_read: self.dimensions() * self.encoding.bytes(),
            normalized_bytes: source.raw_bytes(),
        };
        Ok(TensorCapture { source, receipt })
    }
}

/// Metadata about an actual owned copy, not authentication or a restart grade.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TensorCaptureReceipt {
    contract: TensorContract,
    buffer: BufferIdentity,
    layout: TensorLayout,
    selected: TokenSelection,
    source_bytes_read: usize,
    normalized_bytes: usize,
}

impl TensorCaptureReceipt {
    pub fn contract(&self) -> &TensorContract { &self.contract }
    pub fn buffer(&self) -> BufferIdentity { self.buffer }
    pub fn layout(&self) -> &TensorLayout { &self.layout }
    pub fn selection(&self) -> TokenSelection { self.selected }
    pub fn source_bytes_read(&self) -> usize { self.source_bytes_read }
    pub fn normalized_bytes(&self) -> usize { self.normalized_bytes }
}

#[derive(Clone, Debug)]
pub struct TensorCapture { source: SourceFrame, receipt: TensorCaptureReceipt }

impl TensorCapture {
    pub fn source(&self) -> &SourceFrame { &self.source }
    pub fn receipt(&self) -> &TensorCaptureReceipt { &self.receipt }
}

fn decode_scalar(bytes: &[u8], encoding: ScalarEncoding, order: ByteOrder) -> Result<f32, Error> {
    let bits = match encoding {
        ScalarEncoding::Binary32 => {
            let word: [u8; 4] = bytes.try_into().map_err(|_| Error::Incomplete)?;
            match order { ByteOrder::Little => u32::from_le_bytes(word), ByteOrder::Big => u32::from_be_bytes(word) }
        }
        ScalarEncoding::BFloat16 | ScalarEncoding::Binary16 => {
            let word: [u8; 2] = bytes.try_into().map_err(|_| Error::Incomplete)?;
            let word = match order { ByteOrder::Little => u16::from_le_bytes(word), ByteOrder::Big => u16::from_be_bytes(word) };
            if encoding == ScalarEncoding::BFloat16 { u32::from(word) << 16 }
            else {
                let sign = u32::from(word & 0x8000) << 16;
                let exponent = u32::from((word >> 10) & 31);
                let mut fraction = u32::from(word & 1023);
                match exponent {
                    31 => return Err(Error::InvalidInput),
                    0 if fraction == 0 => sign,
                    0 => {
                        let mut shifts = 0_u32;
                        while fraction & 1024 == 0 { fraction <<= 1; shifts += 1; }
                        sign | ((113 - shifts) << 23) | ((fraction & 1023) << 13)
                    }
                    _ => sign | ((exponent + 112) << 23) | (fraction << 13),
                }
            }
        }
    };
    let value = f32::from_bits(bits);
    if !value.is_finite() { return Err(Error::InvalidInput); }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_binary16_word_matches_independent_power_of_two_arithmetic() {
        for word in 0_u16..=u16::MAX {
            let exponent = i32::from((word >> 10) & 31);
            let fraction = u32::from(word & 1023);
            for order in [ByteOrder::Little, ByteOrder::Big] {
                let bytes = match order { ByteOrder::Little => word.to_le_bytes(), ByteOrder::Big => word.to_be_bytes() };
                let result = decode_scalar(&bytes, ScalarEncoding::Binary16, order);
                if exponent == 31 { assert_eq!(result, Err(Error::InvalidInput)); continue; }
                let magnitude = if exponent == 0 { f64::from(fraction) * 2_f64.powi(-24) }
                    else { f64::from(1024 + fraction) * 2_f64.powi(exponent - 25) };
                let expected = if word & 0x8000 != 0 { -magnitude } else { magnitude } as f32;
                assert_eq!(result.unwrap().to_bits(), expected.to_bits(), "word={word:04x}");
            }
        }
    }

    #[test]
    fn every_bfloat16_word_preserves_the_finite_value_and_rejects_nonfinite() {
        for word in 0_u16..=u16::MAX {
            let expected = f32::from_bits(u32::from(word) << 16);
            let actual = decode_scalar(&word.to_be_bytes(), ScalarEncoding::BFloat16, ByteOrder::Big);
            if expected.is_finite() { assert_eq!(actual.unwrap().to_bits(), expected.to_bits()); }
            else { assert_eq!(actual, Err(Error::InvalidInput)); }
        }
    }

    #[test]
    fn checked_span_certificate_never_accepts_overlapping_elements_in_small_domain() {
        for first in 0..=24 {
            for second in 0..=24 {
                for rows in 1..=4 {
                    for columns in 1..=4 {
                        let result = TensorLayout::new([1, 1, rows, columns], [0, 0, first, second],
                            3, ScalarEncoding::Binary32, ByteOrder::Little);
                        if let Ok(layout) = result {
                            let mut occupied = std::collections::BTreeSet::new();
                            for row in 0..rows {
                                for column in 0..columns {
                                    let offset = 3 + row * first + column * second;
                                    for byte in offset..offset + 4 { assert!(occupied.insert(byte)); }
                                }
                            }
                            assert_eq!(occupied.last().copied().unwrap() + 1, layout.end);
                        }
                    }
                }
            }
        }
    }
}
