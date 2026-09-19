//! Exact, model-bound byte BPE for the explicit FA-BBPE/1 profile.
//!
//! Input is one byte sequence: no Unicode normalization, regex pretokenization,
//! chat template, special-token recognition, dropout or unknown-token fallback.
//! A caller must independently establish that these are its model's trained
//! tokenization semantics. This is NOT a Hugging Face tokenizer.json importer.
//! Original prompt bytes and token boundaries survive encoding; replay uses the
//! emitted IDs, never a decode/re-tokenize round trip. No authority is created.

mod wire;
#[cfg(test)]
mod tests;

use crate::action::consequence::activation::tensor::kv::decoder::{
    DecoderProfile, MAX_DECODER_VOCABULARY,
};
use crate::Error;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::fmt;
use std::ops::Range;
use std::rc::Rc;

pub const MAX_INPUT_BYTES: usize = 65_536;
pub const MAX_TOKEN_BYTES: usize = 4_096;
pub const MAX_VOCABULARY_BYTES: usize = 8 * 1_048_576;
pub const MAX_MERGES: usize = 131_072;
pub const MAX_CONTROL_TOKENS: usize = 256;
pub const MAX_PAIR_LOOKUPS: usize = 3 * MAX_INPUT_BYTES;
pub const MAX_HEAP_POPS: usize = 3 * MAX_INPUT_BYTES;
pub const MAX_DECODE_BYTES: usize = 4 * 1_048_576;

/// The vector index is the ORIGINAL model token ID. Controls have no inferred
/// text spelling. Ordinary bytes that resemble a control remain ordinary bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenBytes {
    Content(Vec<u8>),
    Control,
}

/// Rank is this rule's index in the supplied ordered vector, never token ID,
/// spelling length or lexical order. Equal-ranked occurrences merge leftmost.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Merge {
    pub left: u32,
    pub right: u32,
    pub result: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenizationBudget {
    pub input_bytes: usize,
    pub pair_lookups: usize,
    pub heap_pops: usize,
}
impl Default for TokenizationBudget {
    fn default() -> Self {
        Self { input_bytes: MAX_INPUT_BYTES, pair_lookups: MAX_PAIR_LOOKUPS,
            heap_pops: MAX_HEAP_POPS }
    }
}

/// Logical examinations by THIS encoder, not allocations, FLOPs or wall time.
/// Stale heap entries count. A refusal retains performed work but no partial IDs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TokenizationWork {
    pub input_bytes: usize,
    pub pair_lookups: usize,
    pub heap_pops: usize,
    pub merges: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenizationFailure {
    pub error: Error,
    pub work: TokenizationWork,
}
impl fmt::Display for TokenizationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for TokenizationFailure {}

struct Data {
    profile: DecoderProfile,
    vocabulary: Vec<TokenBytes>,
    merges: Vec<Merge>,
    ranks: BTreeMap<(u32, u32), (usize, u32)>,
    singletons: [u32; 256],
    controls: Vec<u32>,
    max_content_bytes: usize,
}

/// Immutable tokenization DATA bound to an independently supplied model profile.
/// Clones share vocabulary and ranks, not RNG, model state or effect authority.
#[derive(Clone)]
pub struct ByteBpe(Rc<Data>);
impl fmt::Debug for ByteBpe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ByteBpe").field("profile", self.profile())
            .field("merges", &self.0.merges.len()).finish_non_exhaustive()
    }
}

/// One complete encoding. Construction is private; there is no successful value
/// containing an encoded prefix in place of a truncated or over-budget prompt.
/// Bytes may be non-UTF-8, and neither Debug nor a lossy String conversion exposes
/// or rewrites them. A clone remains an observation, not an execution handle.
#[derive(Clone)]
pub struct TokenizedInput {
    tokenizer: ByteBpe,
    source: Vec<u8>,
    tokens: Vec<u32>,
    spans: Vec<Range<usize>>,
    work: TokenizationWork,
}
impl TokenizedInput {
    pub fn source(&self) -> &[u8] { &self.source }
    pub fn tokens(&self) -> &[u32] { &self.tokens }
    pub fn spans(&self) -> &[Range<usize>] { &self.spans }
    pub fn profile(&self) -> &DecoderProfile { self.tokenizer.profile() }
    pub fn tokenizer(&self) -> &ByteBpe { &self.tokenizer }
    pub fn work(&self) -> TokenizationWork { self.work }
}
impl fmt::Debug for TokenizedInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokenizedInput").field("source_bytes", &self.source.len())
            .field("tokens", &self.tokens.len()).field("work", &self.work)
            .finish_non_exhaustive()
    }
}

impl ByteBpe {
    /// Require all 256 unique single-byte tokens, unique nonempty spellings, and
    /// an exact bounded model vocabulary. Each merge concatenates existing
    /// content, uses only earlier-defined operands and has a unique input pair.
    /// Every multi-byte token must be reachable through at least one merge.
    /// Controls cannot participate in merges or be recognized from input text.
    pub fn new(profile: DecoderProfile, vocabulary: Vec<TokenBytes>, merges: Vec<Merge>)
        -> Result<Self, Error>
    {
        if vocabulary.len() != profile.shape().vocabulary { return Err(Error::Binding); }
        if vocabulary.len() < 256 { return Err(Error::Incomplete); }
        if vocabulary.len() > MAX_DECODER_VOCABULARY || merges.len() > MAX_MERGES {
            return Err(Error::Limit);
        }
        // All length limits precede byte comparisons and retention of indexes.
        let mut bytes = 0_usize;
        let mut controls = Vec::new();
        let mut max_content_bytes = 0;
        for (id, token) in vocabulary.iter().enumerate() {
            match token {
                TokenBytes::Control => {
                    if controls.len() == MAX_CONTROL_TOKENS { return Err(Error::Limit); }
                    controls.push(id as u32);
                }
                TokenBytes::Content(value) => {
                    if value.is_empty() { return Err(Error::InvalidInput); }
                    if value.len() > MAX_TOKEN_BYTES { return Err(Error::Limit); }
                    bytes = bytes.checked_add(value.len()).ok_or(Error::Limit)?;
                    if bytes > MAX_VOCABULARY_BYTES { return Err(Error::Limit); }
                    max_content_bytes = max_content_bytes.max(value.len());
                }
            }
        }
        let mut seen = BTreeSet::new();
        let mut singletons = [None; 256];
        let mut available = BTreeSet::new();
        for (id, token) in vocabulary.iter().enumerate() {
            if let TokenBytes::Content(value) = token {
                if !seen.insert(value.as_slice()) { return Err(Error::Duplicate); }
                if value.len() == 1 {
                    singletons[usize::from(value[0])] = Some(id as u32);
                    available.insert(id as u32);
                }
            }
        }
        if singletons.iter().any(Option::is_none) { return Err(Error::Incomplete); }
        let mut ranks = BTreeMap::new();
        for (rank, rule) in merges.iter().enumerate() {
            let left = content(&vocabulary, rule.left)?;
            let right = content(&vocabulary, rule.right)?;
            let result = content(&vocabulary, rule.result)?;
            if !available.contains(&rule.left) || !available.contains(&rule.right) {
                return Err(Error::Binding);
            }
            if result.len() != left.len() + right.len()
                || !result.starts_with(left) || &result[left.len()..] != right
            { return Err(Error::Binding); }
            if ranks.insert((rule.left, rule.right), (rank, rule.result)).is_some() {
                return Err(Error::Duplicate);
            }
            available.insert(rule.result);
        }
        if vocabulary.iter().enumerate().any(|(id, token)| {
            matches!(token, TokenBytes::Content(_)) && !available.contains(&(id as u32))
        }) { return Err(Error::Incomplete); }
        drop(seen);
        Ok(Self(Rc::new(Data { profile, vocabulary, merges, ranks,
            singletons: singletons.map(|id| id.expect("complete singleton inventory")),
            controls, max_content_bytes })))
    }

    pub fn profile(&self) -> &DecoderProfile { &self.0.profile }
    pub fn control_tokens(&self) -> &[u32] { &self.0.controls }
    pub fn max_content_bytes(&self) -> usize { self.0.max_content_bytes }
    pub fn content_bytes(&self, token: u32) -> Result<&[u8], Error> {
        content(&self.0.vocabulary, token)
    }
    pub fn is_control(&self, token: u32) -> Result<bool, Error> {
        Ok(matches!(self.0.vocabulary.get(token as usize).ok_or(Error::Missing)?, TokenBytes::Control))
    }
    pub fn binds(&self, profile: &DecoderProfile) -> bool { self.profile() == profile }

    /// Heap candidates use fixed input positions and original token IDs. A merge
    /// only creates candidates at its two neighbors, avoiding repeated whole-
    /// prompt scans. Dead/stale candidates are checked and charged before reuse.
    pub fn encode(&self, source: &[u8], budget: TokenizationBudget)
        -> Result<TokenizedInput, TokenizationFailure>
    {
        let mut work = TokenizationWork::default();
        self.encode_inner(source, budget, &mut work)
            .map_err(|error| TokenizationFailure { error, work })
    }

    fn encode_inner(&self, source: &[u8], budget: TokenizationBudget,
        work: &mut TokenizationWork) -> Result<TokenizedInput, Error>
    {
        if budget.input_bytes > MAX_INPUT_BYTES || budget.pair_lookups > MAX_PAIR_LOOKUPS
            || budget.heap_pops > MAX_HEAP_POPS || source.len() > budget.input_bytes
        { return Err(Error::Limit); }
        let count = source.len();
        let mut nodes = Vec::new();
        nodes.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        let mut heap = BinaryHeap::new();
        heap.try_reserve(count.saturating_mul(3)).map_err(|_| Error::Limit)?;
        let mut tokens = Vec::new();
        tokens.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        let mut spans = Vec::new();
        spans.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        let mut original = Vec::new();
        original.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        original.extend_from_slice(source);
        for (index, byte) in source.iter().enumerate() {
            nodes.push(Node { token: self.0.singletons[usize::from(*byte)],
                end: index + 1, previous: index.checked_sub(1),
                next: (index + 1 < count).then_some(index + 1), live: true });
        }
        work.input_bytes = count;
        for index in 0..count {
            self.offer(index, &nodes, &mut heap, budget, work)?;
        }
        while !heap.is_empty() {
            if work.heap_pops == budget.heap_pops { return Err(Error::Limit); }
            work.heap_pops += 1;
            let Reverse(candidate) = heap.pop().expect("nonempty candidate heap");
            let Candidate { left, right, left_token, right_token, result, .. } = candidate;
            if !nodes[left].live || !nodes[right].live || nodes[left].next != Some(right)
                || nodes[left].token != left_token || nodes[right].token != right_token
            { continue; }
            let next = nodes[right].next;
            nodes[left].token = result;
            nodes[left].end = nodes[right].end;
            nodes[left].next = next;
            nodes[right].live = false;
            if let Some(next) = next { nodes[next].previous = Some(left); }
            work.merges += 1;
            if let Some(previous) = nodes[left].previous {
                self.offer(previous, &nodes, &mut heap, budget, work)?;
            }
            self.offer(left, &nodes, &mut heap, budget, work)?;
        }
        // Linked intervals partition the ORIGINAL input, including invalid UTF-8.
        let mut index = (count != 0).then_some(0);
        while let Some(current) = index {
            tokens.push(nodes[current].token);
            spans.push(current..nodes[current].end);
            index = nodes[current].next;
        }
        Ok(TokenizedInput { tokenizer: self.clone(), source: original, tokens, spans, work: *work })
    }

    fn offer(&self, left: usize, nodes: &[Node], heap: &mut BinaryHeap<Reverse<Candidate>>,
        budget: TokenizationBudget, work: &mut TokenizationWork) -> Result<(), Error>
    {
        let Some(right) = nodes[left].next else { return Ok(()); };
        if work.pair_lookups == budget.pair_lookups { return Err(Error::Limit); }
        work.pair_lookups += 1;
        let left_token = nodes[left].token;
        let right_token = nodes[right].token;
        if let Some(&(rank, result)) = self.0.ranks.get(&(left_token, right_token)) {
            heap.push(Reverse(Candidate { rank, left, right, left_token, right_token, result }));
        }
        Ok(())
    }

    /// Decode original content IDs, without normalization, special-token
    /// stripping, replacement characters or pretending arbitrary bytes are UTF-8.
    /// The entire length and token inventory are admitted before allocation.
    pub fn decode(&self, tokens: &[u32], max_bytes: usize) -> Result<Vec<u8>, Error> {
        if tokens.len() > MAX_INPUT_BYTES || max_bytes > MAX_DECODE_BYTES { return Err(Error::Limit); }
        let mut bytes = 0_usize;
        for token in tokens {
            bytes = bytes.checked_add(self.content_bytes(*token)?.len()).ok_or(Error::Limit)?;
            if bytes > max_bytes { return Err(Error::Limit); }
        }
        let mut output = Vec::new();
        output.try_reserve_exact(bytes).map_err(|_| Error::Limit)?;
        for token in tokens { output.extend_from_slice(self.content_bytes(*token)?); }
        Ok(output)
    }
}

fn content(vocabulary: &[TokenBytes], token: u32) -> Result<&[u8], Error> {
    match vocabulary.get(token as usize).ok_or(Error::Missing)? {
        TokenBytes::Content(bytes) => Ok(bytes),
        TokenBytes::Control => Err(Error::Binding),
    }
}
struct Node {
    token: u32,
    end: usize,
    previous: Option<usize>,
    next: Option<usize>,
    live: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Candidate {
    rank: usize,
    left: usize,
    right: usize,
    left_token: u32,
    right_token: u32,
    result: u32,
}
