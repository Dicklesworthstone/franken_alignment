//! Explicit named controls, matched before the original byte-BPE merge pass.
//! No special spelling is guessed, normalized, stripped or treated as authority.

use super::{ByteBpe, DecoderProfile, Error, Merge, Node, TokenBytes,
    MAX_CONTROL_TOKENS, MAX_INPUT_BYTES, MAX_TOKEN_BYTES};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

/// Aggregate UTF-8 spelling bytes. The sparse recognizer has at most this many
/// edges and one more state; it never allocates a states-by-256 dense table.
pub const MAX_SPECIAL_TOKEN_BYTES: usize = 65_536;

#[derive(Clone, Copy)]
struct Match { token: u32, len: usize }
#[derive(Default)]
struct State {
    edges: BTreeMap<u8, usize>,
    failure: usize,
    longest: Option<Match>,
}
#[derive(Default)]
pub(super) struct SpecialTokens {
    names: BTreeMap<u32, Vec<u8>>,
    states: Vec<State>,
}

impl ByteBpe {
    /// Freeze exact UTF-8 spellings for explicitly named Control IDs. Earliest
    /// start wins; at that start the longest spelling wins, independent of ID.
    /// A control splits BPE segments, so merges can never cross its boundary.
    /// Names may be prefixes of one another but duplicate names are refused.
    ///
    /// This is an explicit tokenizer profile, not special-token inference or a
    /// post-hoc mutation of an existing tokenizer. Unnamed controls remain usable
    /// only as explicit token IDs. Empty names preserve the original FA-BBPE/1
    /// semantics and encoding. Named profiles serialize as FA-BBPE/2.
    ///
    /// Matching is literal: no regex, word boundaries, normalization, added
    /// whitespace or stripping. Decoding a Control still refuses; generation may
    /// suppress it only AFTER its original monitor admits a configured stop ID.
    pub fn new_with_special_tokens(profile: DecoderProfile, vocabulary: Vec<TokenBytes>,
        merges: Vec<Merge>, names: BTreeMap<u32, Vec<u8>>) -> Result<Self, Error>
    {
        // Check names before constructing any recognizer, and retain the original
        // vocabulary/merge validation rather than weakening it for special IDs.
        let special = SpecialTokens::new(&vocabulary, names)?;
        let mut tokenizer = Self::new(profile, vocabulary, merges)?;
        Rc::get_mut(&mut tokenizer.0).expect("fresh unshared tokenizer").special = special;
        Ok(tokenizer)
    }

    /// Immutable tokenizer data. Presence is not a permission to emit a control
    /// or to bypass a helper's independently registered stop-token policy.
    pub fn special_tokens(&self) -> &BTreeMap<u32, Vec<u8>> { &self.0.special.names }
}

impl SpecialTokens {
    fn new(vocabulary: &[TokenBytes], names: BTreeMap<u32, Vec<u8>>) -> Result<Self, Error> {
        if names.len() > MAX_CONTROL_TOKENS { return Err(Error::Limit); }
        let mut bytes = 0_usize;
        for (id, name) in &names {
            if !matches!(vocabulary.get(*id as usize), Some(TokenBytes::Control)) {
                return Err(Error::Binding);
            }
            if name.is_empty() { return Err(Error::InvalidInput); }
            if name.len() > MAX_TOKEN_BYTES { return Err(Error::Limit); }
            bytes = bytes.checked_add(name.len()).ok_or(Error::Limit)?;
            if bytes > MAX_SPECIAL_TOKEN_BYTES { return Err(Error::Limit); }
        }
        let mut unique = BTreeSet::new();
        for name in names.values() {
            std::str::from_utf8(name).map_err(|_| Error::InvalidInput)?;
            if !unique.insert(name.as_slice()) { return Err(Error::Duplicate); }
        }
        drop(unique);
        if names.is_empty() { return Ok(Self::default()); }

        // Reverse patterns and scan input backwards. The longest terminal suffix
        // then gives the longest control STARTING at each original byte offset.
        // Forward greedy selection produces leftmost-longest nonoverlap without
        // enumerating every nested match or rescanning a long shared prefix.
        let mut states = Vec::new();
        states.try_reserve_exact(bytes + 1).map_err(|_| Error::Limit)?;
        states.push(State::default());
        for (id, name) in &names {
            let mut state = 0;
            for byte in name.iter().rev() {
                state = match states[state].edges.get(byte).copied() {
                    Some(next) => next,
                    None => {
                        let next = states.len();
                        states.push(State::default());
                        states[state].edges.insert(*byte, next);
                        next
                    }
                };
            }
            states[state].longest = Some(Match { token: *id, len: name.len() });
        }
        let mut queue = Vec::new();
        queue.try_reserve_exact(states.len()).map_err(|_| Error::Limit)?;
        queue.extend(states[0].edges.iter().map(|(byte, child)| (0, *byte, *child)));
        let mut at = 0;
        while at < queue.len() {
            let (parent, byte, child) = queue[at];
            at += 1;
            let failure = if parent == 0 { 0 } else {
                let mut fallback = states[parent].failure;
                while fallback != 0 && !states[fallback].edges.contains_key(&byte) {
                    fallback = states[fallback].failure;
                }
                states[fallback].edges.get(&byte).copied().unwrap_or(0)
            };
            states[child].failure = failure;
            let inherited = states[failure].longest;
            if inherited.is_some_and(|candidate| states[child].longest
                .is_none_or(|own| candidate.len > own.len)) {
                states[child].longest = inherited;
            }
            // Each trie edge is queued once. No per-node temporary allocation
            // and no enumeration of every nested terminal match is required.
            queue.extend(states[child].edges.iter().map(|(b, s)| (child, *b, *s)));
        }
        Ok(Self { names, states })
    }

    pub(super) fn bind_nodes(&self, source: &[u8], nodes: &mut [Node]) -> Result<(), Error> {
        if source.len() > MAX_INPUT_BYTES || nodes.len() != source.len() { return Err(Error::Limit); }
        if self.names.is_empty() { return Ok(()); }
        let mut matches = Vec::new();
        matches.try_reserve_exact(source.len()).map_err(|_| Error::Limit)?;
        matches.resize(source.len(), None);
        let mut state = 0;
        for (start, byte) in source.iter().enumerate().rev() {
            loop {
                if let Some(next) = self.states[state].edges.get(byte) {
                    state = *next;
                    break;
                }
                if state == 0 { break; }
                state = self.states[state].failure;
            }
            matches[start] = self.states[state].longest;
        }
        let mut start = 0;
        while start < source.len() {
            let Some(found) = matches[start] else { start += 1; continue; };
            let end = start.checked_add(found.len).ok_or(Error::Limit)?;
            if end > nodes.len() { return Err(Error::Binding); }
            nodes[start].token = found.token;
            nodes[start].end = end;
            nodes[start].next = (end < nodes.len()).then_some(end);
            for node in &mut nodes[start + 1..end] { node.live = false; }
            if end < nodes.len() { nodes[end].previous = Some(start); }
            start = end;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
