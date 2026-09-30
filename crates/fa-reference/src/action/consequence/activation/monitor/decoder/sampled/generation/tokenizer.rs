//! Exact, model-bound byte BPE with explicit optional named controls.
//!
//! Input is one byte sequence: no Unicode normalization, regex pretokenization,
//! chat template, dropout or unknown-token fallback. Named-control recognition
//! is enabled only by an explicit immutable registration (FA-BBPE/2).
//! A caller must independently establish that these are its model's trained
//! tokenization semantics. External import accepts only its documented subset.
//! Original prompt bytes and token boundaries survive encoding; replay uses the
//! emitted IDs, never a decode/re-tokenize round trip. No authority is created.

mod wire;
mod special;
pub use special::MAX_SPECIAL_TOKEN_BYTES;
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
/// text spelling. Only new_with_special_tokens registers literal spellings.
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

/// Logical byte/BPE examinations, not allocations, FLOPs or wall time.
/// Optional named-control matching is separately bounded by input/name limits;
/// pair_lookups and heap_pops count only the original BPE pass.
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
    special: special::SpecialTokens,
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

/// A cooperative encoding never exposes unfinished token IDs or spans.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenizationProgress {
    Pending,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EncodingPhase {
    Offering(usize),
    Merging,
    Collecting(Option<usize>),
    Complete,
    Failed(Error),
}

/// Owns one immutable prompt and its original tokenizer through bounded BPE
/// steps. Admission still copies the bounded prompt, allocates scratch and runs
/// the linear named-control pass; this is NOT a constant-time admission claim.
/// Each advance thereafter offers one initial pair, pops at most one candidate
/// (including stale candidates), or retains one final token. A successful merge
/// offers at most its two neighbors. No new buffer capacity is requested while
/// advancing. The caller may service cancellation/deadlines between advances.
///
/// Cancellation consumes this owner and returns performed work, never partial
/// IDs. Finishing a pending or failed owner cannot manufacture a complete prompt.
/// This is tokenizer data processing, not an inference or authority handle.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::TokenizationCursor;
/// fn duplicate(cursor: TokenizationCursor) { let _copy = cursor.clone(); }
/// ```
pub struct TokenizationCursor {
    tokenizer: ByteBpe,
    source: Vec<u8>,
    nodes: Vec<Node>,
    heap: BinaryHeap<Reverse<Candidate>>,
    tokens: Vec<u32>,
    spans: Vec<Range<usize>>,
    budget: TokenizationBudget,
    work: TokenizationWork,
    phase: EncodingPhase,
}

impl fmt::Debug for TokenizationCursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokenizationCursor")
            .field("source_bytes", &self.source.len())
            .field("phase", &self.phase)
            .field("work", &self.work)
            .finish_non_exhaustive()
    }
}

impl TokenizationCursor {
    pub fn work(&self) -> TokenizationWork { self.work }

    /// Repeated terminal calls are inert. The first failure and all performed
    /// work survive retries; neither a retry nor finish restarts the merge pass.
    pub fn advance(&mut self) -> Result<TokenizationProgress, TokenizationFailure> {
        match self.advance_inner() {
            Ok(progress) => Ok(progress),
            Err(error) => {
                self.phase = EncodingPhase::Failed(error);
                Err(TokenizationFailure { error, work: self.work })
            }
        }
    }

    fn advance_inner(&mut self) -> Result<TokenizationProgress, Error> {
        match self.phase {
            EncodingPhase::Failed(error) => return Err(error),
            EncodingPhase::Complete => return Ok(TokenizationProgress::Complete),
            EncodingPhase::Offering(index) => {
                if index < self.nodes.len() {
                    self.tokenizer.offer(index, &self.nodes, &mut self.heap,
                        self.budget, &mut self.work)?;
                    self.phase = EncodingPhase::Offering(index + 1);
                } else {
                    self.phase = EncodingPhase::Merging;
                }
            }
            EncodingPhase::Merging => {
                if self.heap.is_empty() {
                    self.phase = EncodingPhase::Collecting(
                        (!self.nodes.is_empty()).then_some(0));
                } else {
                    if self.work.heap_pops == self.budget.heap_pops { return Err(Error::Limit); }
                    self.work.heap_pops += 1;
                    let Reverse(candidate) = self.heap.pop().expect("nonempty candidate heap");
                    let Candidate { left, right, left_token, right_token, result, .. } = candidate;
                    if self.nodes[left].live && self.nodes[right].live
                        && self.nodes[left].next == Some(right)
                        && self.nodes[left].token == left_token
                        && self.nodes[right].token == right_token
                    {
                        let next = self.nodes[right].next;
                        self.nodes[left].token = result;
                        self.nodes[left].end = self.nodes[right].end;
                        self.nodes[left].next = next;
                        self.nodes[right].live = false;
                        if let Some(next) = next { self.nodes[next].previous = Some(left); }
                        self.work.merges += 1;
                        if let Some(previous) = self.nodes[left].previous {
                            self.tokenizer.offer(previous, &self.nodes, &mut self.heap,
                                self.budget, &mut self.work)?;
                        }
                        self.tokenizer.offer(left, &self.nodes, &mut self.heap,
                            self.budget, &mut self.work)?;
                    }
                }
            }
            EncodingPhase::Collecting(Some(current)) => {
                self.tokens.push(self.nodes[current].token);
                self.spans.push(current..self.nodes[current].end);
                self.phase = EncodingPhase::Collecting(self.nodes[current].next);
            }
            EncodingPhase::Collecting(None) => {
                self.phase = EncodingPhase::Complete;
                return Ok(TokenizationProgress::Complete);
            }
        }
        Ok(TokenizationProgress::Pending)
    }

    pub fn finish(self) -> Result<TokenizedInput, TokenizationFailure> {
        let error = match self.phase {
            EncodingPhase::Complete => None,
            EncodingPhase::Failed(error) => Some(error),
            _ => Some(Error::Incomplete),
        };
        if let Some(error) = error {
            return Err(TokenizationFailure { error, work: self.work });
        }
        Ok(TokenizedInput { tokenizer: self.tokenizer, source: self.source,
            tokens: self.tokens, spans: self.spans, work: self.work })
    }

    /// Drop scratch and unfinished output without refunding observed work.
    pub fn cancel(self) -> TokenizationWork { self.work }
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
            controls, max_content_bytes, special: special::SpecialTokens::default() })))
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

    /// Run the same cooperative cursor to completion. No second BPE engine or
    /// alternate rank, special-token, span or work-accounting policy exists.
    pub fn encode(&self, source: &[u8], budget: TokenizationBudget)
        -> Result<TokenizedInput, TokenizationFailure>
    {
        let mut cursor = self.begin_encode(source, budget)?;
        while cursor.advance()? != TokenizationProgress::Complete {}
        cursor.finish()
    }

    /// Admit owned input and scratch, then yield before initial pair lookups.
    /// Copying and named-control recognition remain a bounded synchronous
    /// admission pass; the candidate and final-output passes are cooperative.
    pub fn begin_encode(&self, source: &[u8], budget: TokenizationBudget)
        -> Result<TokenizationCursor, TokenizationFailure>
    {
        let mut work = TokenizationWork::default();
        self.begin_encode_inner(source, budget, &mut work)
            .map_err(|error| TokenizationFailure { error, work })
    }

    fn begin_encode_inner(&self, source: &[u8], budget: TokenizationBudget,
        work: &mut TokenizationWork) -> Result<TokenizationCursor, Error>
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
        self.0.special.bind_nodes(source, &mut nodes)?;
        Ok(TokenizationCursor { tokenizer: self.clone(), source: original,
            nodes, heap, tokens, spans, budget, work: *work,
            phase: EncodingPhase::Offering(0) })
    }

    fn offer(&self, left: usize, nodes: &[Node], heap: &mut BinaryHeap<Reverse<Candidate>>,
        budget: TokenizationBudget, work: &mut TokenizationWork) -> Result<(), Error>
    {
        if !nodes[left].live { return Ok(()); }
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
            bytes = bytes.checked_add(tokenizer_content(self, *token)?.len()).ok_or(Error::Limit)?;
            if bytes > max_bytes { return Err(Error::Limit); }
        }
        let mut output = Vec::new();
        output.try_reserve_exact(bytes).map_err(|_| Error::Limit)?;
        for token in tokens { output.extend_from_slice(self.content_bytes(*token)?); }
        Ok(output)
    }
}

fn tokenizer_content(tokenizer: &ByteBpe, token: u32) -> Result<&[u8], Error> {
    tokenizer.content_bytes(token)
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

#[cfg(test)]
mod cooperative_tests {
    use super::*;
    use crate::action::consequence::activation::tensor::kv::decoder::{DecoderIdentity, DecoderShape};

    fn tokenizer(named: bool) -> ByteBpe {
        let mut vocabulary: Vec<_> = (0..=255)
            .map(|byte| TokenBytes::Content(vec![byte])).collect();
        vocabulary.extend([TokenBytes::Content(b"aa".to_vec()),
            TokenBytes::Content(b"ab".to_vec()), TokenBytes::Content(b"aab".to_vec())]);
        if named { vocabulary.extend([TokenBytes::Control, TokenBytes::Control]); }
        let profile = DecoderProfile::new(DecoderIdentity {
            tenant: 1, model: 2, model_generation: 3, tokenizer_generation: 4,
            profile_generation: 5,
        }, DecoderShape { vocabulary: vocabulary.len(), hidden: 2, intermediate: 2,
            layers: 1, query_heads: 1, cache_heads: 1, context: 64 }, 1e-5, 10000.0).unwrap();
        let names = if named {
            BTreeMap::from([(259, b"<q>".to_vec()), (260, b"<q>x".to_vec())])
        } else { BTreeMap::new() };
        ByteBpe::new_with_special_tokens(profile, vocabulary, vec![
            Merge { left: 97, right: 97, result: 256 },
            Merge { left: 97, right: 98, result: 257 },
            Merge { left: 256, right: 98, result: 258 },
        ], names).unwrap()
    }

    // Independent rank-ordered vector rewrite; no production heap or node links.
    fn oracle(source: &[u8]) -> Vec<u32> {
        let mut tokens: Vec<_> = source.iter().map(|byte| u32::from(*byte)).collect();
        loop {
            let mut chosen = None;
            for (left, right, result) in [(97, 97, 256), (97, 98, 257), (256, 98, 258)] {
                if let Some(index) = tokens.windows(2).position(|pair| pair == [left, right]) {
                    chosen = Some((index, result));
                    break;
                }
            }
            let Some((index, result)) = chosen else { return tokens; };
            tokens[index] = result;
            tokens.remove(index + 1);
        }
    }

    fn drain(mut cursor: TokenizationCursor) -> TokenizedInput {
        let capacities = (cursor.nodes.capacity(), cursor.heap.capacity(),
            cursor.tokens.capacity(), cursor.spans.capacity(), cursor.source.capacity());
        loop {
            let before = cursor.work();
            let progress = cursor.advance().unwrap();
            let after = cursor.work();
            assert!(after.heap_pops - before.heap_pops <= 1);
            assert!(after.pair_lookups - before.pair_lookups <= 2);
            assert!(after.merges - before.merges <= 1);
            assert_eq!(capacities, (cursor.nodes.capacity(), cursor.heap.capacity(),
                cursor.tokens.capacity(), cursor.spans.capacity(), cursor.source.capacity()));
            if progress == TokenizationProgress::Complete { break; }
        }
        let work = cursor.work();
        assert_eq!(cursor.advance().unwrap(), TokenizationProgress::Complete);
        assert_eq!(cursor.work(), work);
        cursor.finish().unwrap()
    }

    #[test]
    fn cooperative_exhaustive_ids_spans_and_one_shot_work_match() {
        let tokenizer = tokenizer(false);
        for length in 0..=8 {
            for bits in 0..(1_usize << length) {
                let source: Vec<_> = (0..length)
                    .map(|index| if bits & (1 << index) == 0 { b'a' } else { b'b' }).collect();
                let encoded = drain(tokenizer.begin_encode(&source, TokenizationBudget::default()).unwrap());
                assert_eq!(encoded.tokens(), oracle(&source));
                assert_eq!(encoded.source(), source);
                let mut through = 0;
                for (&token, span) in encoded.tokens().iter().zip(encoded.spans()) {
                    assert_eq!(span.start, through);
                    assert_eq!(tokenizer.content_bytes(token).unwrap(), &source[span.clone()]);
                    through = span.end;
                }
                assert_eq!(through, source.len());
                let synchronous = tokenizer.encode(&source, TokenizationBudget::default()).unwrap();
                assert_eq!(encoded.work(), synchronous.work());
                assert_eq!(encoded.spans(), synchronous.spans());
            }
        }
    }

    #[test]
    fn named_controls_keep_leftmost_longest_original_boundaries() {
        let tokenizer = tokenizer(true);
        let encoded = drain(tokenizer.begin_encode(b"a<q>bc<q>x", TokenizationBudget::default()).unwrap());
        assert_eq!(encoded.tokens(), &[97, 259, 98, 99, 260]);
        assert_eq!(encoded.spans(), &[0..1, 1..4, 4..5, 5..6, 6..10]);
        assert_eq!(encoded.source(), b"a<q>bc<q>x");
    }

    #[test]
    fn owned_input_survives_mutation_and_interleaved_cursors() {
        let tokenizer = tokenizer(false);
        let mut source = b"aab\xff\0".to_vec();
        let mut first = tokenizer.begin_encode(&source, TokenizationBudget::default()).unwrap();
        let mut second = tokenizer.begin_encode(b"abab", TokenizationBudget::default()).unwrap();
        source.fill(b'z');
        for _ in 0..3 { first.advance().unwrap(); second.advance().unwrap(); }
        let first = drain(first);
        assert_eq!(first.source(), b"aab\xff\0");
        assert_eq!(first.tokens(), &[258, 255, 0]);
        assert_eq!(drain(second).tokens(), &[257, 257]);
    }

    #[test]
    fn cancellation_and_pending_finish_never_expose_partial_success() {
        let tokenizer = tokenizer(false);
        let mut cursor = tokenizer.begin_encode(b"aaab", TokenizationBudget::default()).unwrap();
        for _ in 0..3 { assert_eq!(cursor.advance().unwrap(), TokenizationProgress::Pending); }
        let work = cursor.work();
        assert_eq!(work.input_bytes, 4);
        assert_eq!(work.pair_lookups, 3);
        assert_eq!(work.heap_pops, 0);
        assert_eq!(cursor.cancel(), work);
        let cursor = tokenizer.begin_encode(b"aaab", TokenizationBudget::default()).unwrap();
        let error = cursor.finish().unwrap_err();
        assert_eq!(error.error, Error::Incomplete);
        assert_eq!(error.work.input_bytes, 4);
    }

    #[test]
    fn merge_budget_failure_is_sticky_and_retains_completed_work() {
        let tokenizer = tokenizer(false);
        let budget = TokenizationBudget { input_bytes: 3, pair_lookups: 2, heap_pops: 8 };
        let mut cursor = tokenizer.begin_encode(b"aaa", budget).unwrap();
        let failure = loop {
            match cursor.advance() {
                Ok(TokenizationProgress::Pending) => {}
                other => break other.unwrap_err(),
            }
        };
        assert_eq!(failure.error, Error::Limit);
        assert_eq!(failure.work, TokenizationWork {
            input_bytes: 3, pair_lookups: 2, heap_pops: 1, merges: 1,
        });
        assert_eq!(cursor.advance(), Err(failure));
        assert_eq!(cursor.finish().unwrap_err(), failure);
        assert_eq!(tokenizer.encode(b"aaa", budget).unwrap_err(), failure);
    }

    #[test]
    fn input_and_heap_limits_refuse_without_unaccounted_retry() {
        let tokenizer = tokenizer(false);
        let failure = tokenizer.begin_encode(b"a", TokenizationBudget {
            input_bytes: 0, ..TokenizationBudget::default()
        }).unwrap_err();
        assert_eq!(failure, TokenizationFailure { error: Error::Limit,
            work: TokenizationWork::default() });
        let mut cursor = tokenizer.begin_encode(b"aa", TokenizationBudget {
            heap_pops: 0, ..TokenizationBudget::default()
        }).unwrap();
        let failure = loop {
            match cursor.advance() {
                Ok(TokenizationProgress::Pending) => {}
                other => break other.unwrap_err(),
            }
        };
        assert_eq!(failure.work.heap_pops, 0);
        assert_eq!(failure.work.merges, 0);
        assert_eq!(failure.work.pair_lookups, 1);
        assert_eq!(cursor.cancel(), failure.work);
        assert!(drain(tokenizer.begin_encode(&[], TokenizationBudget::default()).unwrap())
            .tokens().is_empty());
    }
}
