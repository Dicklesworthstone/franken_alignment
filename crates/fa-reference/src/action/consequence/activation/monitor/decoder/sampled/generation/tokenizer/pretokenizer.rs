//! Finite recognizers for two named Unicode-17 pretokenization grammars.
//! These mark original byte boundaries; the original BPE owns every merge.

use super::{ByteBpe, DecoderProfile, Error, Merge, Node, TokenBytes, unicode17};
use std::collections::BTreeMap;
use std::rc::Rc;

pub(super) const GPT2_PATTERN: &str = r"'s|'t|'re|'ve|'m|'ll|'d| ?\p{L}+| ?\p{N}+| ?[^\s\p{L}\p{N}]+|\s+(?!\S)|\s+";
pub(super) const LLAMA3_PATTERN: &str = r"(?i:'s|'t|'re|'ve|'m|'ll|'d)|[^\r\n\p{L}\p{N}]?\p{L}+|\p{N}{1,3}| ?[^\s\p{L}\p{N}]+[\r\n]*|\s*[\r\n]+|\s+(?!\S)|\s+";

/// Immutable input interpretation, not a model identity or training claim.
/// Unicode modes accept UTF-8 and use frozen General_Category L/N, White_Space
/// and default simple case folding from Unicode 17.0.0. They insert no prefix,
/// normalize no text and recognize no arbitrary regex. RawBytes admits all bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreTokenization {
    RawBytes,
    Gpt2Unicode17,
    Llama3Unicode17,
}

impl PreTokenization {
    /// Exact named grammar. This is descriptive immutable data, not an API for
    /// compiling a caller-supplied pattern or changing Unicode interpretation.
    pub fn pattern(self) -> Option<&'static str> {
        match self {
            Self::RawBytes => None,
            Self::Gpt2Unicode17 => Some(GPT2_PATTERN),
            Self::Llama3Unicode17 => Some(LLAMA3_PATTERN),
        }
    }

    pub fn unicode_version(self) -> Option<(u8, u8, u8)> {
        match self {
            Self::RawBytes => None,
            Self::Gpt2Unicode17 | Self::Llama3Unicode17 => Some(unicode17::VERSION),
        }
    }

    pub(super) fn bind_nodes(self, source: &str, nodes: &mut [Node], vocabulary: &[TokenBytes])
        -> Result<(), Error>
    {
        if source.len() != nodes.len() { return Err(Error::Binding); }
        // Special recognition already selected complete literal control spans.
        // Each intervening ordinary span is its own regex input, so neither a
        // lookahead nor a contraction can see through a control boundary.
        let control = |node: &Node| matches!(vocabulary.get(node.token as usize), Some(TokenBytes::Control));
        let mut start = 0;
        while start < nodes.len() {
            if control(&nodes[start]) { start = nodes[start].end; continue; }
            let mut end = start;
            while end < nodes.len() && !control(&nodes[end]) { end = nodes[end].end; }
            let ordinary = source.get(start..end).ok_or(Error::Binding)?;
            let mut at = 0;
            while at < ordinary.len() {
                nodes[start + at].boundary_before = true;
                let length = self.piece_length(&ordinary[at..]);
                if length == 0 { return Err(Error::Binding); }
                at += length;
            }
            start = end;
        }
        Ok(())
    }

    fn piece_length(self, source: &str) -> usize {
        if self == Self::RawBytes { return source.len(); }
        let contraction = contraction(source, self == Self::Llama3Unicode17);
        if contraction != 0 { return contraction; }
        let letters = match self {
            Self::Gpt2Unicode17 => optional_space_run(source, unicode17::is_letter),
            Self::Llama3Unicode17 => llama_letters(source),
            Self::RawBytes => unreachable!("raw input returned above"),
        };
        if letters != 0 { return letters; }
        let numbers = if self == Self::Gpt2Unicode17 {
            optional_space_run(source, unicode17::is_number)
        } else {
            source.chars().take(3).take_while(|c| unicode17::is_number(*c))
                .map(char::len_utf8).sum()
        };
        if numbers != 0 { return numbers; }
        let punctuation = optional_space_run(source, is_punctuation);
        if punctuation != 0 {
            return punctuation + if self == Self::Llama3Unicode17 {
                source[punctuation..].chars().take_while(|c| matches!(c, '\r' | '\n'))
                    .map(char::len_utf8).sum()
            } else { 0 };
        }
        // Greedy \s*[\r\n]+ ends at the LAST newline in the whitespace run;
        // whitespace after it belongs to a later piece. This never rescans a
        // growing prefix or implements unbounded regex backtracking.
        if self == Self::Llama3Unicode17 {
            let mut last_newline = 0;
            for (at, c) in source.char_indices().take_while(|(_, c)| unicode17::is_white_space(*c)) {
                if matches!(c, '\r' | '\n') { last_newline = at + c.len_utf8(); }
            }
            if last_newline != 0 { return last_newline; }
        }
        trailing_whitespace(source)
    }
}

impl ByteBpe {
    /// Construct one explicit input profile around the ORIGINAL complete
    /// vocabulary/merge/control admission. Existing constructors stay RawBytes.
    /// Frozen data may be cloned, but no mutable tokenizer interpretation exists.
    pub fn new_with_pre_tokenization(profile: DecoderProfile, vocabulary: Vec<TokenBytes>,
        merges: Vec<Merge>, names: BTreeMap<u32, Vec<u8>>, pre_tokenization: PreTokenization)
        -> Result<Self, Error>
    {
        let mut tokenizer = Self::new_with_special_tokens(profile, vocabulary, merges, names)?;
        Rc::get_mut(&mut tokenizer.0).expect("fresh unshared tokenizer").pre_tokenization = pre_tokenization;
        Ok(tokenizer)
    }
}

fn contraction(source: &str, insensitive: bool) -> usize {
    let Some(rest) = source.strip_prefix('\'') else { return 0; };
    for ending in ["s", "t", "re", "ve", "m", "ll", "d"] {
        let mut actual = rest.chars();
        let mut length = 1;
        let matched = ending.chars().all(|expected| {
            let Some(c) = actual.next() else { return false; };
            length += c.len_utf8();
            if insensitive { unicode17::fold_simple(c) == expected } else { c == expected }
        });
        if matched { return length; }
    }
    0
}

fn optional_space_run(source: &str, class: fn(char) -> bool) -> usize {
    let (prefix, rest) = source.strip_prefix(' ').map_or((0, source), |rest| (1, rest));
    let bytes: usize = rest.chars().take_while(|c| class(*c)).map(char::len_utf8).sum();
    if bytes == 0 { 0 } else { prefix + bytes }
}

fn llama_letters(source: &str) -> usize {
    let mut chars = source.chars();
    let Some(first) = chars.next() else { return 0; };
    if unicode17::is_letter(first) {
        return first.len_utf8() + chars.take_while(|c| unicode17::is_letter(*c)).map(char::len_utf8).sum::<usize>();
    }
    if matches!(first, '\r' | '\n') || unicode17::is_number(first) { return 0; }
    let letters: usize = chars.take_while(|c| unicode17::is_letter(*c)).map(char::len_utf8).sum();
    if letters == 0 { 0 } else { first.len_utf8() + letters }
}

fn is_punctuation(c: char) -> bool {
    !unicode17::is_white_space(c) && !unicode17::is_letter(c) && !unicode17::is_number(c)
}

fn trailing_whitespace(source: &str) -> usize {
    let mut end = 0;
    let mut last_start = 0;
    for (at, c) in source.char_indices().take_while(|(_, c)| unicode17::is_white_space(*c)) {
        last_start = at;
        end = at + c.len_utf8();
    }
    // \s+(?!\S) consumes all whitespace at EOF, otherwise all but the last
    // scalar when possible. The final \s+ alternative handles a lone scalar.
    if end == source.len() || last_start == 0 { end } else { last_start }
}

#[cfg(test)]
mod tests;
