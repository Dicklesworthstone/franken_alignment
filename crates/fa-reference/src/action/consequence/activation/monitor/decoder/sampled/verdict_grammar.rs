//! Fixed native-helper output grammar, compiled from the actual decoder bytes.
//! It restricts numerical token choices; it does not judge their truth or grant
//! authority. No public installer or checkpoint import/export is provided.

use super::generation::tokenizer::ByteBpe;
use super::generation::MAX_STOP_TOKENS;
use crate::Error;

const WORDS: [&[u8]; 4] = [b"allow", b"hold", b"deny", b"abstain"];
const MAX_WORD_BYTES: usize = 7;
// Empty prefix, every word byte prefix, and the consumed-terminal state.
const MAX_STATES: usize = 1 + 5 + 4 + 4 + 7 + 1;
// Unique nonempty token spellings allow at most one token for each remaining
// word-prefix length. Controls are separately bounded by the native policy.
const MAX_CHOICES: usize = WORDS.len() * MAX_WORD_BYTES + MAX_STOP_TOKENS;

#[derive(Debug)]
struct State {
    allowed: Vec<u32>,
    next: Vec<usize>,
}

#[derive(Debug)]
pub(super) struct VerdictGrammar {
    states: Vec<State>,
    current: usize,
}

impl VerdictGrammar {
    pub(super) fn new(tokenizer: &ByteBpe, stops: &[u32]) -> Result<Self, Error> {
        if stops.is_empty() { return Err(Error::Incomplete); }
        if stops.len() > MAX_STOP_TOKENS { return Err(Error::Limit); }
        for (index, token) in stops.iter().enumerate() {
            if !tokenizer.is_control(*token)? { return Err(Error::Binding); }
            if stops[..index].contains(token) { return Err(Error::Duplicate); }
        }
        if tokenizer.control_tokens().iter().any(|token| !stops.contains(token)) {
            return Err(Error::Incomplete);
        }

        let mut prefixes: Vec<&[u8]> = Vec::new();
        prefixes.try_reserve_exact(MAX_STATES - 1).map_err(|_| Error::Limit)?;
        prefixes.push(b"");
        for word in WORDS {
            for end in 1..=word.len() {
                let prefix = &word[..end];
                if !prefixes.contains(&prefix) { prefixes.push(prefix); }
            }
        }
        let terminal = prefixes.len();
        if terminal >= MAX_STATES { return Err(Error::Limit); }
        let mut states = Vec::new();
        states.try_reserve_exact(terminal + 1).map_err(|_| Error::Limit)?;
        for prefix in &prefixes {
            let complete = WORDS.contains(prefix);
            let mut state = State { allowed: Vec::new(), next: Vec::new() };
            state.allowed.try_reserve_exact(MAX_CHOICES).map_err(|_| Error::Limit)?;
            state.next.try_reserve_exact(MAX_CHOICES).map_err(|_| Error::Limit)?;
            for token in 0..tokenizer.profile().shape().vocabulary {
                let token = u32::try_from(token).map_err(|_| Error::Limit)?;
                let next = if tokenizer.is_control(token)? {
                    if complete && stops.contains(&token) { Some(terminal) } else { None }
                } else {
                    let bytes = tokenizer.content_bytes(token)?;
                    // No UTF-8 decoding, normalization, tokenizer re-encoding,
                    // empty-token loops or control-spelling substitution.
                    if bytes.is_empty() || bytes.len() > MAX_WORD_BYTES { continue; }
                    WORDS.iter().find_map(|word| {
                        let suffix = word.strip_prefix(*prefix)?;
                        if !suffix.starts_with(bytes) { return None; }
                        let length = prefix.len() + bytes.len();
                        prefixes.iter().position(|candidate| *candidate == &word[..length])
                    })
                };
                if let Some(next) = next {
                    if state.allowed.len() == MAX_CHOICES { return Err(Error::Limit); }
                    state.allowed.push(token); state.next.push(next);
                }
            }
            // ByteBpe contains all 256 byte singletons; each word therefore has
            // a real continuation. Never invent a default verdict on a gap.
            if state.allowed.is_empty() { return Err(Error::Incomplete); }
            states.push(state);
        }
        states.push(State { allowed: Vec::new(), next: Vec::new() });
        Ok(Self { states, current: 0 })
    }

    pub(super) fn allowed(&self) -> Result<&[u32], Error> {
        let tokens = &self.states[self.current].allowed;
        if tokens.is_empty() { return Err(Error::WrongState); }
        Ok(tokens)
    }

    pub(super) fn next(&self, token: u32) -> Result<usize, Error> {
        let state = &self.states[self.current];
        let index = state.allowed.binary_search(&token).map_err(|_| Error::Binding)?;
        Ok(state.next[index])
    }

    // Only a previously checked transition, committed with the original token
    // and PRNG immediately after numerical computation, before fallible review.
    pub(super) fn commit(&mut self, next: usize) { self.current = next; }
}

#[cfg(test)]
mod tests;
