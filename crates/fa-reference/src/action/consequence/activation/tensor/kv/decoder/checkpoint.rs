//! Restart this decoder's actual numerical state, without importing authority.
//! Foreign/unverified KV images cannot construct this checkpoint type.

use super::{DecoderBudget, DecoderModel, DecoderSession, DecoderStep, DecoderWork};
use super::super::model::{LayerRestore, ModelKvDescriptor, ModelKvImage, MAX_MODEL_KV_VALUES};
use super::super::restore::{HostTensorMut, KvDestination, KvRestoreWindow};
use super::super::KvAppend;
use super::super::super::{BufferIdentity, ByteOrder, HostTensor, ScalarEncoding, TensorLayout};
use crate::Error;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

/// Parameter object, original tokens, complete KV prefix and next-token logits.
/// This profile has no stochastic sampler, pending token, scheduler or RNG state.
/// Cloning retains immutable numerical data, never mutable caches or permissions.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderCheckpoint;
/// use fa_reference::action::consequence::gate::containment::ActorState;
/// fn install_live(checkpoint: DecoderCheckpoint) -> ActorState { checkpoint }
/// ```
#[derive(Clone)]
pub struct DecoderCheckpoint {
    model: DecoderModel,
    stream: u64,
    tokens: Rc<[u32]>,
    cache: ModelKvImage,
    logits: Option<Rc<[f32]>>,
}
impl fmt::Debug for DecoderCheckpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecoderCheckpoint").field("profile", self.model.profile())
            .field("position", &self.tokens.len()).finish_non_exhaustive()
    }
}
impl DecoderCheckpoint {
    pub fn model(&self) -> &DecoderModel { &self.model }
    pub fn stream(&self) -> u64 { self.stream }
    pub fn tokens(&self) -> &[u32] { &self.tokens }
    pub fn cache(&self) -> &ModelKvImage { &self.cache }
    pub fn logits(&self) -> Option<&[f32]> { self.logits.as_deref() }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecoderRestoreBudget { pub cache_values: usize }

/// Captured restore work is separate from new inference-product counts. The
/// original source descriptor is retained; new captures have a new stream and
/// a bulk-capture revision, not fabricated original per-token capture receipts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecoderRestoreReceipt {
    pub source: ModelKvDescriptor,
    pub resumed_stream: u64,
    pub position: u64,
    pub values_restored: usize,
    pub bytes_written: usize,
    pub bytes_recaptured: usize,
    pub staged_write_bytes: usize,
}

impl DecoderSession {
    pub fn checkpoint(&self) -> Result<DecoderCheckpoint, Error> {
        let cache = self.cache_image()?;
        let tokens = self.tokens.clone().into();
        Ok(DecoderCheckpoint { model: self.model.clone(), stream: self.stream,
            tokens, cache, logits: self.logits.clone() })
    }

    /// Choose and consume exactly one token from the previous complete logits.
    /// Refusal cannot leave a selected-but-half-applied token. This intentionally
    /// has no EOS interpretation, stochastic state or automatic generation loop.
    pub fn advance_greedy(&mut self, expected_position: u64, budget: DecoderBudget) -> Result<DecoderStep, Error> {
        if expected_position != self.position() { return Err(Error::Stale); }
        let token = self.greedy_token()?;
        self.advance(expected_position, token, budget)
    }
}

impl DecoderModel {
    /// Retain exact immutable parameters, not merely matching shape/numeric IDs.
    /// Restore through the existing all-layer CPU writer and checked capture,
    /// then compute subsequent tokens with the SAME forward implementation.
    /// This cannot restore an arbitrary external-host or edited experiment image.
    pub fn restore_checkpoint(
        &self, checkpoint: &DecoderCheckpoint, resumed_stream: u64, budget: DecoderRestoreBudget,
    ) -> Result<(DecoderSession, DecoderRestoreReceipt), Error> {
        if !Rc::ptr_eq(&self.data, &checkpoint.model.data) { return Err(Error::Binding); }
        if resumed_stream == 0 || resumed_stream == checkpoint.stream { return Err(Error::InvalidInput); }
        let count = checkpoint.tokens.len();
        let values = checkpoint.cache.normalized_values();
        if budget.cache_values > MAX_MODEL_KV_VALUES || values > budget.cache_values { return Err(Error::Limit); }
        if count != checkpoint.cache.len() || count > self.profile().shape().context
            || checkpoint.cache.profile() != self.cache_profile()
            || checkpoint.logits.is_some() != (count > 0)
        { return Err(Error::Binding); }
        let mut session = self.session(resumed_stream)?;
        let mut receipt = DecoderRestoreReceipt {
            source: checkpoint.cache.descriptor(), resumed_stream, position: count as u64,
            values_restored: values, bytes_written: 0, bytes_recaptured: 0, staged_write_bytes: 0,
        };
        if count == 0 { return Ok((session, receipt)); }
        let d = self.profile().head_width();
        let heads = self.profile().shape().cache_heads;
        let per_row = heads * d * 4;
        let layout = TensorLayout::new([1, count, heads, d], [0, per_row, d * 4, 4], 0,
            ScalarEncoding::Binary32, ByteOrder::Little)?;
        let length = count * per_row;
        let mut buffers = Vec::new();
        buffers.try_reserve_exact(self.profile().shape().layers).map_err(|_| Error::Limit)?;
        for id in self.cache_profile().layers().keys() {
            buffers.push((*id, zeroes(length)?, zeroes(length)?));
        }
        let targets = buffers.iter_mut().map(|(id, keys, values)| {
            let contract = &self.cache_profile().layers()[&*id];
            (*id, LayerRestore {
                destination: KvDestination::Separate {
                    keys: HostTensorMut { identity: BufferIdentity { object: contract.keys().profile().tap, generation: 1 },
                        layout: &layout, bytes: keys },
                    values: HostTensorMut { identity: BufferIdentity { object: contract.values().profile().tap, generation: 1 },
                        layout: &layout, bytes: values },
                },
                window: KvRestoreWindow { first_position: 0, token_count: count, batch: 0,
                    first_token: 0, buffer_first_position: 0 },
            })
        }).collect::<BTreeMap<_, _>>();
        let plan = checkpoint.cache.prepare_restore(targets)?;
        receipt.staged_write_bytes = plan.staged_bytes();
        let restored = plan.commit();
        receipt.bytes_written = restored.bytes_written;
        // These are owned derived replay buffers. Their new capture identity
        // never masquerades as the original computation's source generation.
        let requests = buffers.iter().map(|(id, keys, values)| {
            let contract = &self.cache_profile().layers()[id];
            (*id, KvAppend {
                keys: HostTensor { identity: BufferIdentity { object: contract.keys().profile().tap, generation: 1 },
                    layout: &layout, bytes: keys },
                values: HostTensor { identity: BufferIdentity { object: contract.values().profile().tap, generation: 1 },
                    layout: &layout, bytes: values },
                first_token: 0, token_count: count, buffer_first_position: 0, first_sequence: 1,
            })
        }).collect();
        let captured = session.cache.append(0, requests)?;
        receipt.bytes_recaptured = captured.source_bytes_read;
        session.tokens = checkpoint.tokens.to_vec();
        session.logits = checkpoint.logits.clone();
        // Restoring does not pretend to have re-executed historical inference.
        session.work = DecoderWork::default();
        Ok((session, receipt))
    }

    /// Independent algorithmic route: discard KV state, run the ORIGINAL tokens,
    /// and compare every retained cache scalar plus the complete next-logit vector.
    /// A new source stream/revision is different provenance, not a value mismatch.
    pub fn recompute_checkpoint(
        &self, checkpoint: &DecoderCheckpoint, replay_stream: u64, budget: DecoderBudget,
    ) -> Result<DecoderSession, Error> {
        if !Rc::ptr_eq(&self.data, &checkpoint.model.data) { return Err(Error::Binding); }
        if replay_stream == 0 || replay_stream == checkpoint.stream { return Err(Error::InvalidInput); }
        let session = self.recompute(replay_stream, &checkpoint.tokens, budget)?;
        let actual = session.cache_image()?;
        if !same_values(&actual, &checkpoint.cache)? || !same_logits(session.logits.as_deref(), checkpoint.logits()) {
            return Err(Error::Binding);
        }
        Ok(session)
    }
}

pub(super) fn same_values(left: &ModelKvImage, right: &ModelKvImage) -> Result<bool, Error> {
    if left.profile() != right.profile() || left.len() != right.len() { return Ok(false); }
    for id in left.profile().layers().keys() {
        let a = left.layer(*id)?; let b = right.layer(*id)?;
        for position in 0..left.len() as u64 {
            let a = a.token(position)?; let b = b.token(position)?;
            if a.key().words.as_ref() != b.key().words.as_ref()
                || a.value().words.as_ref() != b.value().words.as_ref() { return Ok(false); }
        }
    }
    Ok(true)
}
fn same_logits(left: Option<&[f32]>, right: Option<&[f32]>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(a), Some(b)) => a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.to_bits() == b.to_bits()),
        _ => false,
    }
}
fn zeroes(length: usize) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|_| Error::Limit)?;
    bytes.resize(length, 0);
    Ok(bytes)
}
