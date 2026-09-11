//! Paired key/value capture of contiguous new cache positions.
//!
//! This owns observation bytes only. It neither restores an inference backend
//! nor clones actor authority. Supplied buffer identities and temporal coherence
//! remain host assumptions. A failed append never publishes half of a K/V pair.

use super::{HostTensor, TensorCapture, TensorContract, TokenSelection};
use crate::Error;
use std::collections::BTreeMap;

pub const MAX_KV_POSITIONS: usize = 4096;
pub const MAX_KV_VALUES: usize = 1_048_576;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KvContract {
    keys: TensorContract,
    values: TensorContract,
    query_heads: usize,
}

impl KvContract {
    /// Query heads are not storage heads. This profile uses contiguous groups
    /// of query heads per cache head, including MQA (one cache head) and GQA.
    pub fn new(keys: TensorContract, values: TensorContract, query_heads: usize) -> Result<Self, Error> {
        let key = keys.profile();
        let value = values.profile();
        if key.tenant != value.tenant || key.model != value.model
            || key.model_generation != value.model_generation || key.tap == value.tap
            || keys.heads() != values.heads()
        { return Err(Error::Binding); }
        if query_heads == 0 || query_heads % keys.heads() != 0 { return Err(Error::InvalidInput); }
        if query_heads > super::MAX_VALUES { return Err(Error::Limit); }
        Ok(Self { keys, values, query_heads })
    }

    pub fn keys(&self) -> &TensorContract { &self.keys }
    pub fn values(&self) -> &TensorContract { &self.values }
    pub fn query_heads(&self) -> usize { self.query_heads }
    pub fn cache_head_for(&self, query_head: usize) -> Result<usize, Error> {
        if query_head >= self.query_heads { return Err(Error::InvalidInput); }
        Ok(query_head / (self.query_heads / self.keys.heads()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KvBudget { pub positions: usize, pub normalized_values: usize }

#[derive(Clone, Copy, Debug)]
pub struct KvAppend<'a> {
    pub keys: HostTensor<'a>,
    pub values: HostTensor<'a>,
    pub first_token: usize,
    pub token_count: usize,
    pub buffer_first_position: u64,
    pub first_sequence: u64,
}

#[derive(Clone, Debug)]
pub struct KvToken { key: TensorCapture, value: TensorCapture }

impl KvToken {
    pub fn key(&self) -> &TensorCapture { &self.key }
    pub fn value(&self) -> &TensorCapture { &self.value }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KvAppendReceipt {
    pub revision: u64,
    pub first_position: u64,
    pub next_position: u64,
    pub first_sequence: u64,
    pub next_sequence: u64,
    pub token_count: usize,
    pub source_bytes_read: usize,
    pub normalized_values: usize,
}

/// A bounded, owned, immutable-prefix observation store. No overwrite, skip,
/// truncation, gap filling, resumption or conversion into restart authority.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::tensor::kv::KvCapture;
/// use fa_reference::action::Permit;
/// fn promote(capture: KvCapture) -> Permit { capture }
/// ```
#[derive(Debug)]
pub struct KvCapture {
    contract: KvContract,
    stream: u64,
    batch: usize,
    first_position: u64,
    next_position: u64,
    next_sequence: u64,
    revision: u64,
    budget: KvBudget,
    tokens: Vec<KvToken>,
    receipts: Vec<KvAppendReceipt>,
    normalized_values: usize,
    source_bytes_read: usize,
    generations: BTreeMap<u64, u64>,
}

impl KvCapture {
    pub fn new(
        contract: KvContract, stream: u64, batch: usize, first_position: u64,
        first_sequence: u64, budget: KvBudget,
    ) -> Result<Self, Error> {
        if stream == 0 || first_sequence == 0 || budget.positions == 0 || budget.normalized_values == 0 {
            return Err(Error::InvalidInput);
        }
        if budget.positions > MAX_KV_POSITIONS || budget.normalized_values > MAX_KV_VALUES {
            return Err(Error::Limit);
        }
        Ok(Self { contract, stream, batch, first_position, next_position: first_position,
            next_sequence: first_sequence, revision: 0, budget, tokens: Vec::new(),
            receipts: Vec::new(), normalized_values: 0, source_bytes_read: 0, generations: BTreeMap::new() })
    }

    pub fn contract(&self) -> &KvContract { &self.contract }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn next_position(&self) -> u64 { self.next_position }
    pub fn next_sequence(&self) -> u64 { self.next_sequence }
    pub fn len(&self) -> usize { self.tokens.len() }
    pub fn is_empty(&self) -> bool { self.tokens.is_empty() }
    pub fn normalized_values(&self) -> usize { self.normalized_values }
    /// Successful committed reads only; failed appends may also perform reads.
    pub fn source_bytes_read(&self) -> usize { self.source_bytes_read }
    pub fn receipts(&self) -> &[KvAppendReceipt] { &self.receipts }

    pub fn token(&self, position: u64) -> Result<&KvToken, Error> {
        let offset = position.checked_sub(self.first_position).ok_or(Error::Missing)?;
        let index = usize::try_from(offset).map_err(|_| Error::Missing)?;
        self.tokens.get(index).ok_or(Error::Missing)
    }

    pub fn append(&mut self, expected_revision: u64, request: KvAppend<'_>) -> Result<KvAppendReceipt, Error> {
        if expected_revision != self.revision || request.first_sequence != self.next_sequence { return Err(Error::Stale); }
        if request.token_count == 0 { return Err(Error::InvalidInput); }
        self.contract.keys.validate_tensor(request.keys)?;
        self.contract.values.validate_tensor(request.values)?;
        let key_shape = request.keys.layout.shape();
        let value_shape = request.values.layout.shape();
        if key_shape[..2] != value_shape[..2] { return Err(Error::Binding); }
        let token_end = request.first_token.checked_add(request.token_count).ok_or(Error::Overflow)?;
        if self.batch >= key_shape[0] || token_end > key_shape[1] { return Err(Error::InvalidInput); }
        for tensor in [request.keys, request.values] {
            if self.generations.get(&tensor.identity.object).is_some_and(|floor| tensor.identity.generation < *floor) {
                return Err(Error::Stale);
            }
        }
        // A shared declared storage object must use one identical base slice,
        // one incarnation and disjoint enclosing K/V spans. Other interleaved
        // shared-buffer layouts need a separately registered alias contract.
        if request.keys.identity.object == request.values.identity.object {
            if request.keys.identity.generation != request.values.identity.generation
                || !std::ptr::eq(request.keys.bytes, request.values.bytes)
            { return Err(Error::Binding); }
            let keys = request.keys.layout.byte_range();
            let values = request.values.layout.byte_range();
            if keys.start < values.end && values.start < keys.end { return Err(Error::Binding); }
        }
        let first_token = u64::try_from(request.first_token).map_err(|_| Error::Overflow)?;
        let first_position = request.buffer_first_position.checked_add(first_token).ok_or(Error::Overflow)?;
        if first_position != self.next_position { return Err(Error::Stale); }
        let count = u64::try_from(request.token_count).map_err(|_| Error::Overflow)?;
        let next_position = self.next_position.checked_add(count).ok_or(Error::Overflow)?;
        let next_sequence = self.next_sequence.checked_add(count).ok_or(Error::Overflow)?;
        let revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        let positions = self.tokens.len().checked_add(request.token_count).ok_or(Error::Overflow)?;
        let per_token = self.contract.keys.dimensions().checked_add(self.contract.values.dimensions()).ok_or(Error::Overflow)?;
        let new_values = per_token.checked_mul(request.token_count).ok_or(Error::Overflow)?;
        let normalized_values = self.normalized_values.checked_add(new_values).ok_or(Error::Overflow)?;
        let per_token_bytes = self.contract.keys.dimensions() * self.contract.keys.encoding().bytes()
            + self.contract.values.dimensions() * self.contract.values.encoding().bytes();
        let read_bytes = per_token_bytes.checked_mul(request.token_count).ok_or(Error::Overflow)?;
        let source_bytes_read = self.source_bytes_read.checked_add(read_bytes).ok_or(Error::Overflow)?;
        if positions > self.budget.positions || normalized_values > self.budget.normalized_values {
            return Err(Error::Limit);
        }
        let mut captured = Vec::with_capacity(request.token_count);
        for index in 0..request.token_count {
            let selected = TokenSelection {
                batch: self.batch, token: request.first_token + index,
                first_position: request.buffer_first_position, stream: self.stream,
                sequence: request.first_sequence + index as u64,
            };
            let key = self.contract.keys.capture(request.keys, selected)?;
            let value = self.contract.values.capture(request.values, selected)?;
            captured.push(KvToken { key, value });
        }
        let receipt = KvAppendReceipt {
            revision, first_position, next_position, first_sequence: request.first_sequence,
            next_sequence, token_count: request.token_count, source_bytes_read: read_bytes,
            normalized_values: new_values,
        };
        // Finish fallible reservations before publishing values or frontier.
        self.tokens.try_reserve(request.token_count).map_err(|_| Error::Limit)?;
        self.receipts.try_reserve(1).map_err(|_| Error::Limit)?;
        self.tokens.extend(captured);
        self.receipts.push(receipt.clone());
        self.revision = revision;
        self.next_position = next_position;
        self.next_sequence = next_sequence;
        self.normalized_values = normalized_values;
        self.source_bytes_read = source_bytes_read;
        for tensor in [request.keys, request.values] {
            self.generations.insert(tensor.identity.object, tensor.identity.generation);
        }
        Ok(receipt)
    }
}
