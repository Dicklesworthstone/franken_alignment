//! Lossless CPU write-back of captured values into explicitly borrowed buffers.
//!
//! Preparation validates every scalar and offset before writing either K or V.
//! Commit owns the exclusive destination borrows and contains no fallible work.
//! This is call-level data atomicity, not crash atomicity or inference restart.

use super::{KvCapture, KvContract, MAX_KV_POSITIONS, MAX_KV_VALUES};
use super::super::{
    BufferIdentity, ByteOrder, HostTensor, ScalarEncoding, TensorContract, TensorLayout,
    TokenSelection, decode_scalar,
};
use crate::action::consequence::activation::{FrameIdentity, SourceFrame};
use crate::Error;

#[derive(Debug)]
pub struct HostTensorMut<'a> {
    pub identity: BufferIdentity,
    pub layout: &'a TensorLayout,
    pub bytes: &'a mut [u8],
}

impl HostTensorMut<'_> {
    fn view(&self) -> HostTensor<'_> {
        HostTensor { identity: self.identity, layout: self.layout, bytes: self.bytes }
    }
}

/// Shared storage uses one exclusive slice with disjoint enclosing K/V spans.
/// Separate storage requires distinct declared object IDs as well as exclusive
/// Rust borrows. Neither form authenticates the supplied storage identity.
#[derive(Debug)]
pub enum KvDestination<'a> {
    Separate { keys: HostTensorMut<'a>, values: HostTensorMut<'a> },
    Shared {
        identity: BufferIdentity,
        keys: &'a TensorLayout,
        values: &'a TensorLayout,
        bytes: &'a mut [u8],
    },
}

impl KvDestination<'_> {
    fn views(&self) -> Result<(HostTensor<'_>, HostTensor<'_>), Error> {
        match self {
            Self::Separate { keys, values } => {
                if keys.identity.object == values.identity.object { return Err(Error::Binding); }
                Ok((keys.view(), values.view()))
            }
            Self::Shared { identity, keys, values, bytes } => {
                let k = keys.byte_range();
                let v = values.byte_range();
                if k.start < v.end && v.start < k.end { return Err(Error::Binding); }
                Ok((HostTensor { identity: *identity, layout: keys, bytes },
                    HostTensor { identity: *identity, layout: values, bytes }))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KvRestoreWindow {
    pub first_position: u64,
    pub token_count: usize,
    pub batch: usize,
    pub first_token: usize,
    pub buffer_first_position: u64,
}

/// Historical write metadata, never a backend-completion or authority token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TensorRestoreReceipt {
    pub source: FrameIdentity,
    pub destination: BufferIdentity,
    pub layout: TensorLayout,
    pub selection: TokenSelection,
    pub values_written: usize,
    pub bytes_written: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KvRestoreReceipt {
    pub window: KvRestoreWindow,
    pub first_sequence: u64,
    pub stream: u64,
    pub keys: BufferIdentity,
    pub values: BufferIdentity,
    pub key_layout: TensorLayout,
    pub value_layout: TensorLayout,
    pub normalized_values: usize,
    pub bytes_written: usize,
}

#[derive(Debug)]
struct Write { offset: usize, bytes: [u8; 4] }

#[derive(Debug)]
struct Staged { writes: Vec<Write>, width: usize }

impl Staged {
    fn publish(self, destination: &mut [u8]) {
        for write in self.writes {
            destination[write.offset..write.offset + self.width]
                .copy_from_slice(&write.bytes[..self.width]);
        }
    }
    fn bytes(&self) -> usize { self.writes.len() * std::mem::size_of::<Write>() }
}

/// Dropping an uncommitted plan leaves its destination unchanged. The mutable
/// borrow prevents safe Rust code from reusing the buffer before commit/drop.
/// This operation confers no access to memory outside the supplied slice.
#[derive(Debug)]
#[must_use = "restore plans do not write until commit consumes them"]
pub struct PreparedTensorRestore<'a> {
    destination: HostTensorMut<'a>,
    staged: Staged,
    receipt: TensorRestoreReceipt,
}

impl PreparedTensorRestore<'_> {
    pub fn staged_bytes(&self) -> usize { self.staged.bytes() }
    pub fn commit(self) -> TensorRestoreReceipt {
        self.staged.publish(self.destination.bytes);
        self.receipt
    }
}

#[derive(Debug)]
#[must_use = "restore plans do not write until commit consumes them"]
pub struct PreparedKvRestore<'a> {
    destination: KvDestination<'a>,
    keys: Staged,
    values: Staged,
    receipt: KvRestoreReceipt,
}

impl PreparedKvRestore<'_> {
    /// Logical staging storage (offset plus scalar bytes), excluding vector
    /// capacity/allocator metadata and the caller's destination buffers.
    pub fn staged_bytes(&self) -> usize { self.keys.bytes() + self.values.bytes() }

    pub fn commit(self) -> KvRestoreReceipt {
        match self.destination {
            KvDestination::Separate { keys, values } => {
                self.keys.publish(keys.bytes);
                self.values.publish(values.bytes);
            }
            KvDestination::Shared { bytes, .. } => {
                self.keys.publish(bytes);
                self.values.publish(bytes);
            }
        }
        self.receipt
    }
}

impl TensorContract {
    /// Exact inverse of the registered scalar representation, not a quantizer.
    /// A finite source that is not exactly representable refuses, without writes.
    pub fn prepare_restore<'a>(
        &self, source: &SourceFrame, destination: HostTensorMut<'a>, selected: TokenSelection,
    ) -> Result<PreparedTensorRestore<'a>, Error> {
        let staged = stage_rows(self, destination.view(), &[source], selected)?;
        let receipt = TensorRestoreReceipt {
            source: source.identity(), destination: destination.identity,
            layout: destination.layout.clone(), selection: selected,
            values_written: self.dimensions(), bytes_written: self.dimensions() * self.encoding().bytes(),
        };
        Ok(PreparedTensorRestore { destination, staged, receipt })
    }
}

impl KvCapture {
    /// Restore a retained contiguous window, including a window that starts on
    /// a later captured page. The source capture and its frontier never change.
    pub fn prepare_restore<'a>(
        &self, expected_revision: u64, destination: KvDestination<'a>, window: KvRestoreWindow,
    ) -> Result<PreparedKvRestore<'a>, Error> {
        if expected_revision != self.revision { return Err(Error::Stale); }
        let range = source_range(self.first_position, self.tokens.len(), window)?;
        let keys: Vec<_> = self.tokens[range.clone()].iter().map(|t| t.key().source()).collect();
        let values: Vec<_> = self.tokens[range].iter().map(|t| t.value().source()).collect();
        prepare_pair(&self.contract, &keys, &values, destination, window)
    }
}

pub(super) fn source_range(
    first: u64, count: usize, window: KvRestoreWindow,
) -> Result<std::ops::Range<usize>, Error> {
    if window.token_count == 0 { return Err(Error::InvalidInput); }
    if window.token_count > MAX_KV_POSITIONS { return Err(Error::Limit); }
    let start = window.first_position.checked_sub(first).ok_or(Error::Missing)?;
    let start = usize::try_from(start).map_err(|_| Error::Missing)?;
    let end = start.checked_add(window.token_count).ok_or(Error::Overflow)?;
    if end > count { return Err(Error::Missing); }
    let token = u64::try_from(window.first_token).map_err(|_| Error::Overflow)?;
    if window.buffer_first_position.checked_add(token).ok_or(Error::Overflow)? != window.first_position {
        return Err(Error::Binding);
    }
    Ok(start..end)
}

pub(super) fn prepare_pair<'a>(
    contract: &KvContract, keys: &[&SourceFrame], values: &[&SourceFrame],
    destination: KvDestination<'a>, window: KvRestoreWindow,
) -> Result<PreparedKvRestore<'a>, Error> {
    if keys.is_empty() || keys.len() != values.len() || keys.len() != window.token_count {
        return Err(Error::Binding);
    }
    if keys.len() > MAX_KV_POSITIONS { return Err(Error::Limit); }
    let normalized_values = contract.keys().dimensions().checked_add(contract.values().dimensions())
        .and_then(|n| n.checked_mul(keys.len())).ok_or(Error::Overflow)?;
    if normalized_values > MAX_KV_VALUES { return Err(Error::Limit); }
    let (key_target, value_target) = destination.views()?;
    if key_target.layout.shape()[..2] != value_target.layout.shape()[..2] { return Err(Error::Binding); }
    let first = keys[0].identity();
    if first.position != window.first_position { return Err(Error::Binding); }
    for (k, v) in keys.iter().zip(values) {
        let k = k.identity();
        let v = v.identity();
        if k.stream != v.stream || k.sequence != v.sequence || k.position != v.position {
            return Err(Error::Binding);
        }
    }
    let selected = TokenSelection {
        batch: window.batch, token: window.first_token, first_position: window.buffer_first_position,
        stream: first.stream, sequence: first.sequence,
    };
    let staged_keys = stage_rows(contract.keys(), key_target, keys, selected)?;
    let staged_values = stage_rows(contract.values(), value_target, values, selected)?;
    let receipt = KvRestoreReceipt {
        window, first_sequence: first.sequence, stream: first.stream,
        keys: key_target.identity, values: value_target.identity,
        key_layout: key_target.layout.clone(), value_layout: value_target.layout.clone(),
        normalized_values,
        bytes_written: staged_keys.writes.len() * staged_keys.width
            + staged_values.writes.len() * staged_values.width,
    };
    Ok(PreparedKvRestore { destination, keys: staged_keys, values: staged_values, receipt })
}

fn stage_rows(
    contract: &TensorContract, target: HostTensor<'_>, sources: &[&SourceFrame], selected: TokenSelection,
) -> Result<Staged, Error> {
    contract.validate_tensor(target)?;
    let count = contract.dimensions().checked_mul(sources.len()).ok_or(Error::Overflow)?;
    if sources.is_empty() || count > MAX_KV_VALUES { return Err(Error::Limit); }
    let end = selected.token.checked_add(sources.len()).ok_or(Error::Overflow)?;
    if selected.batch >= target.layout.shape()[0] || end > target.layout.shape()[1] {
        return Err(Error::InvalidInput);
    }
    let mut writes = Vec::new();
    writes.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    for (row, source) in sources.iter().enumerate() {
        let selection = TokenSelection {
            token: selected.token + row,
            sequence: selected.sequence.checked_add(row as u64).ok_or(Error::Overflow)?,
            ..selected
        };
        if source.identity() != contract.frame_identity(selection)? || source.dimensions() != contract.dimensions() {
            return Err(Error::Binding);
        }
        for (index, word) in source.words.iter().copied().enumerate() {
            let offset = target.layout.offset_of([
                selected.batch, selection.token, index / contract.channels(), index % contract.channels(),
            ])?;
            let end = offset.checked_add(contract.encoding().bytes()).ok_or(Error::Overflow)?;
            target.bytes.get(offset..end).ok_or(Error::Incomplete)?;
            writes.push(Write { offset, bytes: encode_exact(word, contract.encoding(), contract.byte_order())? });
        }
    }
    Ok(Staged { writes, width: contract.encoding().bytes() })
}

/// Require bit-exact representability, including the sign of zero. A final
/// independent decode check also guards the exponent/subnormal boundary cases.
pub(super) fn encode_exact(bits: u32, encoding: ScalarEncoding, order: ByteOrder) -> Result<[u8; 4], Error> {
    if !f32::from_bits(bits).is_finite() { return Err(Error::InvalidInput); }
    let mut output = [0; 4];
    if encoding == ScalarEncoding::Binary32 {
        output = match order { ByteOrder::Little => bits.to_le_bytes(), ByteOrder::Big => bits.to_be_bytes() };
    } else {
        let word = if encoding == ScalarEncoding::BFloat16 {
            if bits & 0xffff != 0 { return Err(Error::Binding); }
            (bits >> 16) as u16
        } else {
            let sign = ((bits >> 16) & 0x8000) as u16;
            let exponent = (bits >> 23) & 255;
            let fraction = bits & 0x7f_ffff;
            if bits & 0x7fff_ffff == 0 { sign }
            else if (113..=142).contains(&exponent) {
                if fraction & 0x1fff != 0 { return Err(Error::Binding); }
                sign | (((exponent - 112) << 10) | (fraction >> 13)) as u16
            } else if (103..=112).contains(&exponent) {
                let shift = 126 - exponent;
                let significand = 0x80_0000 | fraction;
                if significand & ((1_u32 << shift) - 1) != 0 { return Err(Error::Binding); }
                sign | (significand >> shift) as u16
            } else { return Err(Error::Binding); }
        };
        let bytes = match order { ByteOrder::Little => word.to_le_bytes(), ByteOrder::Big => word.to_be_bytes() };
        output[..2].copy_from_slice(&bytes);
    }
    if decode_scalar(&output[..encoding.bytes()], encoding, order)?.to_bits() != bits { return Err(Error::Binding); }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_finite_half_word_round_trips_without_rounding() {
        for encoding in [ScalarEncoding::Binary16, ScalarEncoding::BFloat16] {
            for order in [ByteOrder::Little, ByteOrder::Big] {
                for word in 0..=u16::MAX {
                    let original = match order { ByteOrder::Little => word.to_le_bytes(), ByteOrder::Big => word.to_be_bytes() };
                    if let Ok(value) = decode_scalar(&original, encoding, order) {
                        assert_eq!(&encode_exact(value.to_bits(), encoding, order).unwrap()[..2], &original);
                    }
                }
            }
        }
    }

    #[test]
    fn nonrepresentable_and_nonfinite_values_never_round_into_a_restore() {
        for encoding in [ScalarEncoding::Binary16, ScalarEncoding::BFloat16] {
            for bits in [1_u32, 0x3f80_0001, 0x8000_0001] {
                assert_eq!(encode_exact(bits, encoding, ByteOrder::Little), Err(Error::Binding));
            }
            for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                assert_eq!(encode_exact(value.to_bits(), encoding, ByteOrder::Big), Err(Error::InvalidInput));
            }
        }
        assert_eq!(encode_exact(f32::MAX.to_bits(), ScalarEncoding::Binary16, ByteOrder::Little), Err(Error::Binding));
    }
}
